use super::*;
use anyhow::Context;

/// This is occurrence identity, scoped to an immutable invocation snapshot. It is
/// never fact deduplication by call_id or content bytes. Validate before writing.
pub(in crate::orchestration_runtime_repository) fn occurrence_count(
    input: &AppendRuntimeEventInput,
) -> Result<i64> {
    let Some(manifest) = input.payload.get("_context_occurrences") else {
        return Ok(0);
    };
    anyhow::ensure!(
        input.event_type == "provider_semantic_step"
            && input.node_run_id.is_some()
            && input.payload["source"] == "ai_native"
            && input.payload["kind"] == "model_call",
        "context occurrence owner invalid"
    );
    anyhow::ensure!(
        manifest["version"] == 1,
        "context occurrence version invalid"
    );
    let entries = manifest["entries"]
        .as_array()
        .ok_or_else(|| anyhow!("context occurrence entries missing"))?;
    let body: Value = serde_json::from_str(
        input.payload["body"]
            .as_str()
            .ok_or_else(|| anyhow!("context occurrence snapshot missing"))?,
    )?;
    let mut identities = std::collections::HashSet::new();
    let mut keys = std::collections::HashSet::new();
    for entry in entries {
        let id = Uuid::parse_str(
            entry["event_id"]
                .as_str()
                .ok_or_else(|| anyhow!("context occurrence identity missing"))?,
        )?;
        let metadata = &entry["metadata"];
        let key = metadata["step_key"]
            .as_str()
            .ok_or_else(|| anyhow!("context occurrence key missing"))?;
        anyhow::ensure!(
            identities.insert(id) && keys.insert(key),
            "context occurrence identity repeated"
        );
        let reference = &metadata["body_ref"];
        let pointer = reference["pointer"]
            .as_str()
            .ok_or_else(|| anyhow!("context occurrence pointer missing"))?;
        anyhow::ensure!(
            metadata["kind"] == "tool_result"
                && reference["step_key"] == input.payload["step_key"]
                && body.pointer(pointer).is_some(),
            "context occurrence reference invalid"
        );
    }
    Ok(i64::try_from(entries.len())?)
}

// Keep lossless occurrence metadata as an encoded source string. PostgreSQL
// JSONB remains the same query/display projection used by the old physical child.
// The root JSON can then omit the private manifest without parsing a NUL token.
pub(super) fn project_metadata(payload: &mut Value) {
    if let Some(entries) = payload
        .get_mut("_context_occurrences")
        .and_then(|manifest| manifest.get_mut("entries"))
        .and_then(Value::as_array_mut)
    {
        for entry in entries {
            let packet = lossless_json_value(&entry["metadata"]);
            if let Some(original) = packet[1].as_str() {
                entry["metadata_original"] = Value::String(original.into());
                entry["metadata"] = packet[0].clone();
            }
        }
    }
}

impl PgControlPlaneStore {
    /// Expand the original journal layout before an old-binary rollback. Stop new
    /// writers first. IDs, cursor slots and exact original metadata are retained.
    pub async fn restore_native_context_occurrences(
        &self,
        application: Uuid,
        run: Uuid,
    ) -> Result<u64> {
        let mut tx = self.pool().begin().await?;
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select application_id from flow_runs where id=$1 for no key update",
        )
        .bind(run)
        .fetch_optional(&mut *tx)
        .await?;
        anyhow::ensure!(
            owner == Some(application),
            "native occurrence restoration scope mismatch"
        );
        let rows = sqlx::query("select e.*,runtime_original_json(e.payload,e.raw_json_payloads,'payload') original,s.snapshot_count from runtime_events e join provider_semantic_trajectory_steps s on s.event_id=e.id where e.flow_run_id=$1 and e.payload ? '_context_occurrences' order by e.sequence for update of e,s")
            .bind(run).fetch_all(&mut *tx).await?;
        let mut count = 0;
        for row in rows {
            anyhow::ensure!(
                row.get::<i64, _>("snapshot_count") == 1,
                "native occurrence snapshot replaced"
            );
            let mut original: Value = row.get("original");
            let manifest = original
                .as_object_mut()
                .context("native occurrence owner invalid")?
                .remove("_context_occurrences")
                .context("native occurrence manifest missing")?;
            anyhow::ensure!(
                manifest["version"] == 1,
                "unknown native occurrence version"
            );
            let entries = manifest["entries"]
                .as_array()
                .context("native occurrence entries missing")?;
            for (ordinal, entry) in entries.iter().enumerate() {
                let mut child = original.clone();
                let fields = child
                    .as_object_mut()
                    .context("native occurrence metadata invalid")?;
                fields.remove("body");
                fields.remove("body_format");
                fields.remove("_observation_body_ref");
                let metadata = if let Some(raw) = entry["metadata_original"].as_str() {
                    serde_json::from_str::<Value>(raw)?
                } else {
                    entry["metadata"].clone()
                };
                for (key, value) in metadata
                    .as_object()
                    .context("native occurrence metadata missing")?
                {
                    fields.insert(key.clone(), value.clone());
                }
                let (projection, originals) = lossless_json_columns("payload", &child);
                sqlx::query("insert into runtime_events(id,flow_run_id,node_run_id,span_id,parent_span_id,sequence,event_type,layer,source,trust_level,item_id,ledger_ref,payload,visibility,durability,raw_json_payloads,created_at) values($1,$2,$3,$4,$5,$6,'provider_semantic_step',$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)")
                    .bind(Uuid::parse_str(entry["event_id"].as_str().context("native occurrence identity missing")?)?)
                    .bind(run).bind(row.get::<Uuid,_>("node_run_id")).bind(row.get::<Option<Uuid>,_>("span_id"))
                    .bind(row.get::<Option<Uuid>,_>("parent_span_id")).bind(row.get::<i64,_>("sequence")+i64::try_from(ordinal)?+1)
                    .bind(row.get::<String,_>("layer")).bind(row.get::<String,_>("source")).bind(row.get::<String,_>("trust_level"))
                    .bind(row.get::<Option<Uuid>,_>("item_id")).bind(row.get::<Option<String>,_>("ledger_ref"))
                    .bind(projection).bind(row.get::<String,_>("visibility")).bind(row.get::<String,_>("durability"))
                    .bind(originals).bind(row.get::<OffsetDateTime,_>("created_at")).execute(&mut *tx).await?;
                count += 1;
            }
            let (projection, originals) = lossless_json_columns("payload", &original);
            sqlx::query("update runtime_events set payload=$2,raw_json_payloads=$3 where id=$1")
                .bind(row.get::<Uuid, _>("id"))
                .bind(projection)
                .bind(originals)
                .execute(&mut *tx)
                .await?;
            sqlx::query("update provider_semantic_trajectory_steps set metadata=metadata-'_context_occurrences' where event_id=$1")
                .bind(row.get::<Uuid,_>("id")).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(count)
    }
}
