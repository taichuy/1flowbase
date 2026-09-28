use super::*;
use base64::Engine;
use control_plane_contracts::ports::{ClientTrajectoryFrameKind, ClientTrajectoryTransport};

impl PgControlPlaneStore {
    pub(crate) async fn append_client_trajectory_archive_part(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> Result<ClientTrajectoryArchiveReceipt> {
        self.append_archive_values(input, None).await
    }
    async fn append_archive_values(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
        legacy: Option<&Value>,
    ) -> Result<ClientTrajectoryArchiveReceipt> {
        anyhow::ensure!(!input.frames.is_empty(), "client archive empty part");
        let transport = match input.transport {
            ClientTrajectoryTransport::Http => "http",
            ClientTrajectoryTransport::Websocket => "websocket",
        };
        let mut tx = self.pool().begin().await?;
        sqlx::query("insert into client_trajectory_archive_heads(request_id,transport,flow_run_id) values($1,$2,(select flow_run_id from client_trajectory_captures where request_id=$1)) on conflict do nothing")
            .bind(input.request_id).bind(transport).execute(&mut *tx).await?;
        let row = sqlx::query("select transport,persisted_through from client_trajectory_archive_heads where request_id=$1 for update")
            .bind(input.request_id).fetch_one(&mut *tx).await?;
        anyhow::ensure!(
            row.get::<String, _>("transport") == transport,
            "client archive transport mismatch"
        );
        let existing: Option<Uuid> = sqlx::query_scalar(
            "select request_id from client_trajectory_archive_parts where part_id=$1",
        )
        .bind(input.part_id)
        .fetch_optional(&mut *tx)
        .await?;
        let persisted: i64 = row.get("persisted_through");
        if let Some(owner) = existing {
            anyhow::ensure!(
                owner == input.request_id,
                "client archive part scope mismatch"
            );
            tx.commit().await?;
            return Ok(ClientTrajectoryArchiveReceipt {
                request_id: input.request_id,
                persisted_through: persisted,
            });
        }
        let mut bytes = Vec::new();
        let mut directory = Vec::new();
        for (index, frame) in input.frames.iter().enumerate() {
            let sequence = persisted
                .checked_add(index as i64 + 1)
                .ok_or_else(|| anyhow::anyhow!("client archive sequence exhausted"))?;
            // Preserve a frame boundary regardless of its size; storage batching never
            // turns one original transport frame into several public section items.
            let original = legacy.map(serde_json::to_vec).transpose()?;
            let body = original.as_deref().unwrap_or(&frame.bytes);
            directory.push(serde_json::json!({"sequence":sequence,"kind":frame.kind,"observed_at":frame.observed_at,"offset":bytes.len(),"length":body.len(),"format":if legacy.is_some() {"legacy_json"} else {"wire"}}));
            bytes.extend_from_slice(body);
        }
        let through = persisted + input.frames.len() as i64;
        sqlx::query("insert into client_trajectory_archive_parts(part_id,request_id,first_sequence,last_sequence,frames,bytes) values($1,$2,$3,$4,$5,$6)")
            .bind(input.part_id).bind(input.request_id).bind(persisted+1).bind(through).bind(Value::Array(directory)).bind(bytes)
            .execute(&mut *tx).await?;
        sqlx::query(
            "update client_trajectory_archive_heads set persisted_through=$2 where request_id=$1",
        )
        .bind(input.request_id)
        .bind(through)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ClientTrajectoryArchiveReceipt {
            request_id: input.request_id,
            persisted_through: through,
        })
    }

    pub(crate) async fn discard_unbound_client_trajectory_archive_parts(
        &self,
        request_id: Uuid,
    ) -> Result<()> {
        // Existing part FK cascades from the head. The extra capture guard
        // preserves a trajectory even if its binding transaction is still visible.
        sqlx::query("delete from client_trajectory_archive_heads h where h.request_id=$1 and h.flow_run_id is null and not exists(select 1 from client_trajectory_captures c where c.request_id=h.request_id)")
            .bind(request_id).execute(self.pool()).await?;
        Ok(())
    }

    pub(crate) async fn read_client_trajectory_archive_frames(
        &self,
        request_id: Uuid,
        cursor: i64,
        limit: i64,
    ) -> Result<Vec<ClientTrajectoryArchiveFrame>> {
        let rows = sqlx::query("select f.value as frame,substring(p.bytes from (f.value->>'offset')::integer+1 for (f.value->>'length')::integer) as bytes from client_trajectory_archive_parts p cross join lateral jsonb_array_elements(p.frames) f(value) where p.request_id=$1 and p.last_sequence>$2 and (f.value->>'sequence')::bigint>$2 order by (f.value->>'sequence')::bigint limit $3")
            .bind(request_id).bind(cursor).bind(limit.clamp(1,128)).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                let f: Value = row.get("frame");
                Ok(ClientTrajectoryArchiveFrame {
                    sequence: f["sequence"]
                        .as_i64()
                        .ok_or_else(|| anyhow::anyhow!("archive sequence missing"))?,
                    kind: serde_json::from_value(f["kind"].clone())?,
                    observed_at: f["observed_at"].as_str().unwrap_or_default().to_owned(),
                    bytes: archive_wire(&f, row.get("bytes"))?,
                })
            })
            .collect()
    }

    pub(super) async fn read_archived_client_raw(
        &self,
        _flow: Uuid,
        request: Uuid,
        step: Uuid,
        cursor: Option<i64>,
        limit: i64,
    ) -> Result<ClientTrajectorySection> {
        // Caller has already checked step, flow and many-to-many node ownership.
        let rows = sqlx::query("select f.value as frame,substring(p.bytes from (f.value->>'offset')::integer+1 for (f.value->>'length')::integer) as bytes from client_trajectory_archive_parts p cross join lateral jsonb_array_elements(p.frames) f(value) where p.request_id=$1 and p.last_sequence>$2 and (f.value->>'sequence')::bigint>$2 order by (f.value->>'sequence')::bigint limit $3")
            .bind(request).bind(cursor.unwrap_or(0)).bind(limit+1).fetch_all(self.pool()).await?;
        let more = rows.len() > limit as usize;
        let items:Vec<_>=rows.into_iter().take(limit as usize).map(|row| {
            let f:Value=row.get("frame");
            let bytes:Vec<u8>=row.get("bytes");
            let value=if is_legacy_json(&f) {archive_original_value(&f,&bytes)?} else {
                let (encoding,body)=match String::from_utf8(bytes.clone()) {Ok(s)=>("utf8",s),Err(_)=>("base64",base64::engine::general_purpose::STANDARD.encode(bytes))};
                serde_json::json!({"direction":if f["kind"]=="request" {"submitted"} else {"emitted"},"encoding":encoding,"body":body,"frame_kind":f["kind"]})
            };
            Ok(ClientTrajectorySectionItem {sequence:f["sequence"].as_i64().ok_or_else(||anyhow::anyhow!("stored archive cursor missing"))?,value})
        }).collect::<Result<Vec<_>>>()?;
        let next_cursor = if more {
            items.last().map(|i| i.sequence)
        } else {
            None
        };
        Ok(ClientTrajectorySection {
            step_id: step,
            request_id: request,
            evidence_scope: "capture".into(),
            section: "raw".into(),
            items,
            next_cursor,
        })
    }

    pub(super) async fn append_legacy_client_raw(
        &self,
        input: &AppendClientTrajectoryInput,
        value: &Value,
    ) -> Result<()> {
        let valid:bool=sqlx::query_scalar("select exists(select 1 from client_trajectory_captures where request_id=$1 and flow_run_id=$2 and node_run_id is not distinct from $3)")
            .bind(input.request_id).bind(input.flow_run_id).bind(input.node_run_id).fetch_one(self.pool()).await?;
        anyhow::ensure!(valid, "client trajectory capture scope mismatch");
        let bytes = if value["encoding"] == "base64" {
            base64::engine::general_purpose::STANDARD
                .decode(value["body"].as_str().unwrap_or_default())?
        } else {
            value["body"]
                .as_str()
                .unwrap_or_default()
                .as_bytes()
                .to_vec()
        };
        let transport: Option<String> = sqlx::query_scalar(
            "select transport from client_trajectory_archive_heads where request_id=$1",
        )
        .bind(input.request_id)
        .fetch_optional(self.pool())
        .await?;
        let transport = if transport.as_deref() == Some("websocket") {
            ClientTrajectoryTransport::Websocket
        } else {
            ClientTrajectoryTransport::Http
        };
        self.append_archive_values(
            &AppendClientTrajectoryArchiveInput {
                request_id: input.request_id,
                part_id: Uuid::now_v7(),
                transport,
                frames: vec![ClientTrajectoryArchiveFrame {
                    sequence: 0,
                    kind: value
                        .get("frame_kind")
                        .cloned()
                        .map(serde_json::from_value)
                        .transpose()?
                        .unwrap_or(ClientTrajectoryFrameKind::ResponseJson),
                    observed_at: input.observed_at.clone(),
                    bytes,
                }],
            },
            Some(value),
        )
        .await?;
        Ok(())
    }
}

fn archive_wire(frame: &Value, bytes: Vec<u8>) -> Result<Vec<u8>> {
    if !is_legacy_json(frame) {
        return Ok(bytes);
    }
    let value = archive_original_value(frame, &bytes)?;
    if value["encoding"] == "base64" {
        Ok(base64::engine::general_purpose::STANDARD
            .decode(value["body"].as_str().unwrap_or_default())?)
    } else {
        Ok(value["body"]
            .as_str()
            .unwrap_or_default()
            .as_bytes()
            .to_vec())
    }
}

fn is_legacy_json(frame: &Value) -> bool {
    matches!(
        frame["format"].as_str(),
        Some("legacy_json" | "legacy_payload_json")
    )
}
fn archive_original_value(frame: &Value, bytes: &[u8]) -> Result<Value> {
    let original: Value = serde_json::from_slice(bytes)?;
    if frame["format"] == "legacy_payload_json" {
        original
            .get("fact")
            .and_then(|f| f.get("value"))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("archived legacy payload value missing"))
    } else {
        Ok(original)
    }
}
