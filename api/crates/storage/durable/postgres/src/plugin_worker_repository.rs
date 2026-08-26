use anyhow::Result;
use async_trait::async_trait;
use control_plane::ports::{CreatePluginWorkerLeaseInput, PluginWorkerRepository};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

#[async_trait]
impl PluginWorkerRepository for PgControlPlaneStore {
    async fn create_worker_lease(
        &self,
        input: &CreatePluginWorkerLeaseInput,
    ) -> Result<domain::PluginWorkerLeaseRecord> {
        let row = sqlx::query(
            r#"
            insert into plugin_worker_leases (
                id,
                scope_id,
                installation_id,
                worker_key,
                status,
                runtime_scope
            ) values (
                $1, $2, $3, $4, $5, $6
            )
            returning
                id,
                installation_id,
                worker_key,
                status,
                runtime_scope,
                last_heartbeat_at,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(input.installation_id)
        .bind(&input.worker_key)
        .bind(input.status.as_str())
        .bind(serde_json::json!({}))
        .fetch_one(self.pool())
        .await?;

        Ok(domain::PluginWorkerLeaseRecord {
            id: row.get("id"),
            installation_id: row.get("installation_id"),
            worker_key: row.get("worker_key"),
            status: match row.get::<String, _>("status").as_str() {
                "unloaded" => domain::PluginWorkerStatus::Unloaded,
                "starting" => domain::PluginWorkerStatus::Starting,
                "idle" => domain::PluginWorkerStatus::Idle,
                "busy" => domain::PluginWorkerStatus::Busy,
                "recycled" => domain::PluginWorkerStatus::Recycled,
                "crashed" => domain::PluginWorkerStatus::Crashed,
                other => {
                    return Err(anyhow::anyhow!(
                        "invalid plugin worker status loaded from database: {other}"
                    ))
                }
            },
            runtime_scope: row.get("runtime_scope"),
            last_heartbeat_at: row.get("last_heartbeat_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }
}
