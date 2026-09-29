//! Explicit retained-data maintenance and reentry repair for physical sealing.
use super::*;

#[derive(Debug, Default, serde::Serialize)]
pub struct RuntimeStorageCompactionReceipt {
    pub requested_runs: usize,
    pub client_semantic_records_processed: u64,
    pub native_snapshot_events: u64,
    pub raw_archive_parts: u64,
    pub raw_archive_parts_sealed: u64,
    pub compact_directory_records: u64,
}

impl PgControlPlaneStore {
    /// Compact only the caller's explicit run allowlist. Each immutable part/value
    /// is verified and committed independently; rerunning resumes retained data.
    /// batch_size bounds maintenance transactions, never accepted client content.
    pub async fn compact_retained_runtime_storage(
        &self,
        flow_run_ids: &[Uuid],
        batch_size: i64,
    ) -> Result<RuntimeStorageCompactionReceipt> {
        anyhow::ensure!(batch_size > 0, "maintenance batch size must be positive");
        let mut receipt = RuntimeStorageCompactionReceipt {
            requested_runs: flow_run_ids.len(),
            ..Default::default()
        };
        for flow in flow_run_ids {
            let exists: bool =
                sqlx::query_scalar("select exists(select 1 from flow_runs where id=$1)")
                    .bind(flow)
                    .fetch_one(self.pool())
                    .await?;
            anyhow::ensure!(exists, "maintenance run unavailable: {flow}");
            loop {
                let moved = self
                    .migrate_client_trajectory_semantic_history(
                        std::slice::from_ref(flow),
                        batch_size,
                    )
                    .await?;
                receipt.client_semantic_records_processed += moved;
                if moved == 0 {
                    break;
                }
            }
            loop {
                let moved = self
                    .compact_client_trajectory_directories(std::slice::from_ref(flow), batch_size)
                    .await?;
                receipt.compact_directory_records += moved;
                if moved == 0 {
                    break;
                }
            }
            let mut cursor: Option<Uuid> = None;
            loop {
                let ids:Vec<Uuid>=sqlx::query_scalar("select id from runtime_events where flow_run_id=$1 and event_type='provider_semantic_step' and payload->>'source'='ai_native' and payload->>'kind'='model_call' and observation_body_manifest_id is null and ($2::uuid is null or id>$2) order by id limit $3")
                    .bind(flow).bind(cursor).bind(batch_size).fetch_all(self.pool()).await?;
                if ids.is_empty() {
                    break;
                }
                cursor = ids.last().copied();
                for id in ids {
                    receipt.native_snapshot_events +=
                        u64::from(self.migrate_native_snapshot_event(id, *flow).await?);
                }
            }
            let requests:Vec<Uuid>=sqlx::query_scalar("select h.request_id from client_trajectory_archive_heads h join client_trajectory_captures c on c.request_id=h.request_id and c.flow_run_id=h.flow_run_id where h.flow_run_id=$1 order by h.request_id")
                .bind(flow).fetch_all(self.pool()).await?;
            for request in requests {
                receipt.raw_archive_parts += self
                    .migrate_client_trajectory_archive_parts(*flow, request)
                    .await?;
                receipt.raw_archive_parts_sealed +=
                    self.seal_client_trajectory_archive_request(request).await?;
            }
        }
        Ok(receipt)
    }
}
