//! Native snapshots retain their exact producer string. RawValue supplies source
//! spans; immutable items share only identical bytes, never equivalent JSON.
use super::*;
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn spans<'a>(body: &'a str) -> Option<Vec<(usize, &'a str)>> {
    fn collect<'a>(
        root: &'a str,
        value: &'a RawValue,
        output: &mut Vec<(usize, &'a str)>,
    ) -> Option<()> {
        if !value.get().starts_with('{') {
            return Some(());
        }
        let fields: BTreeMap<String, &'a RawValue> = serde_json::from_str(value.get()).ok()?;
        for (key, value) in fields {
            if ["input", "messages", "tools", "system"].contains(&key.as_str())
                && value.get().starts_with('[')
            {
                let items: Vec<&'a RawValue> = serde_json::from_str(value.get()).ok()?;
                for item in items {
                    let source = item.get();
                    let start = (source.as_ptr() as usize).checked_sub(root.as_ptr() as usize)?;
                    if start.checked_add(source.len())? > root.len() {
                        return None;
                    }
                    output.push((start, source));
                }
            } else if ["native_request", "wire_body"].contains(&key.as_str()) {
                collect(root, value, output)?;
            }
        }
        Some(())
    }
    // A Native producer serializes JSON to this string: NULs inside user values
    // are escaped bytes. A literal NUL or unknown shape remains in the old codec.
    if body.contains('\0') {
        return None;
    }
    let value: &RawValue = serde_json::from_str(body).ok()?;
    let mut result = Vec::new();
    collect(body, value, &mut result)?;
    result.sort_by_key(|(offset, _)| *offset);
    let mut through = 0;
    for (offset, text) in &result {
        if *offset < through {
            return None;
        }
        through = offset + text.len();
    }
    (!result.is_empty()).then_some(result)
}

pub(super) async fn archive(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    scope: Uuid,
    application: Uuid,
    flow: Uuid,
    body: &str,
) -> Result<Option<Uuid>> {
    let Some(parts) = spans(body) else {
        return Ok(None);
    };
    let hash = digest(body.as_bytes());
    // One application owns sharing. This transaction lock also orders concurrent
    // multi-snapshot batches, without retaining an unbounded in-memory cache.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("native-snapshots:{application}"))
        .execute(&mut **tx)
        .await?;
    if let Some((id, original)) = sqlx::query_as::<_,(Uuid,Value)>(
        "select id,runtime_native_snapshot_body(id,$3) from runtime_native_snapshot_manifests where scope_id=$1 and application_id=$2 and content_hash=$4")
        .bind(scope).bind(application).bind(flow).bind(&hash).fetch_optional(&mut **tx).await? {
        anyhow::ensure!(original.as_str() == Some(body), "native snapshot hash collision");
        return Ok(Some(id));
    }
    let mut unique = BTreeMap::new();
    for (_, text) in &parts {
        let hash = digest(text.as_bytes());
        if let Some(previous) = unique.insert(hash, *text) {
            anyhow::ensure!(previous == *text, "native item hash collision");
        }
    }
    let hashes: Vec<_> = unique.keys().cloned().collect();
    let rows = sqlx::query("select id,content_hash,body,byte_size from runtime_native_snapshot_items where scope_id=$1 and application_id=$2 and content_hash=any($3)")
        .bind(scope).bind(application).bind(&hashes).fetch_all(&mut **tx).await?;
    let mut ids = BTreeMap::new();
    for row in rows {
        let hash: String = row.get("content_hash");
        let stored: String = row.get("body");
        let expected = unique[&hash];
        anyhow::ensure!(
            stored == expected && row.get::<i64, _>("byte_size") == i64::try_from(expected.len())?,
            "native item hash collision"
        );
        ids.insert(hash, row.get::<Uuid, _>("id"));
    }
    for (hash, text) in &unique {
        if ids.contains_key(hash) {
            continue;
        }
        let id = Uuid::now_v7();
        sqlx::query("insert into runtime_native_snapshot_items(id,scope_id,application_id,content_hash,body,byte_size) values($1,$2,$3,$4,$5,$6)")
            .bind(id).bind(scope).bind(application).bind(hash).bind(text).bind(i64::try_from(text.len())?).execute(&mut **tx).await?;
        ids.insert(hash.clone(), id);
    }
    let mut layout = Vec::new();
    let mut through = 0;
    let mut rebuilt = String::new();
    for (offset, text) in parts {
        let literal = &body[through..offset];
        if !literal.is_empty() {
            layout.push(json!(["literal", literal]));
            rebuilt.push_str(literal);
        }
        layout.push(json!(["item", ids[&digest(text.as_bytes())]]));
        rebuilt.push_str(text);
        through = offset + text.len();
    }
    if through < body.len() {
        layout.push(json!(["literal", &body[through..]]));
        rebuilt.push_str(&body[through..]);
    }
    anyhow::ensure!(
        rebuilt == body,
        "native snapshot span reconstruction mismatch"
    );
    let id = Uuid::now_v7();
    sqlx::query("insert into runtime_native_snapshot_manifests(id,scope_id,application_id,content_hash,byte_size,layout) values($1,$2,$3,$4,$5,$6)")
        .bind(id).bind(scope).bind(application).bind(hash).bind(i64::try_from(body.len())?).bind(Value::Array(layout)).execute(&mut **tx).await?;
    let item_ids: Vec<_> = ids.values().copied().collect();
    sqlx::query("insert into runtime_native_snapshot_references(manifest_id,item_id) select $1,unnest($2::uuid[])")
        .bind(id).bind(item_ids).execute(&mut **tx).await?;
    let restored: Value = sqlx::query_scalar("select runtime_native_snapshot_body($1,$2)")
        .bind(id)
        .bind(flow)
        .fetch_one(&mut **tx)
        .await?;
    anyhow::ensure!(
        restored.as_str() == Some(body),
        "native snapshot persisted reconstruction mismatch"
    );
    Ok(Some(id))
}

impl PgControlPlaneStore {
    /// Explicit maintenance: switch one retained event only after exact restoration.
    pub(crate) async fn migrate_native_snapshot_event(
        &self,
        event_id: Uuid,
        flow: Uuid,
    ) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) as original,observation_body_manifest_id from runtime_events where id=$1 and flow_run_id=$2 and event_type='provider_semantic_step' for update")
            .bind(event_id).bind(flow).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(false);
        };
        if row
            .get::<Option<Uuid>, _>("observation_body_manifest_id")
            .is_some()
        {
            return Ok(false);
        }
        let original: Value = row.get("original");
        if original["source"] != "ai_native" || original["kind"] != "model_call" {
            return Ok(false);
        }
        if original
            .as_object()
            .is_some_and(|fields| fields.keys().any(|key| key.contains('\0')))
        {
            return Ok(false);
        }
        let Some(body) = original["body"].as_str() else {
            return Ok(false);
        };
        let (scope, application) = sqlx::query_as::<_, (Uuid, Uuid)>(
            "select scope_id,application_id from flow_runs where id=$1",
        )
        .bind(flow)
        .fetch_one(&mut *tx)
        .await?;
        let Some(id) = archive(&mut tx, scope, application, flow, body).await? else {
            return Ok(false);
        };
        let mut projected = original.clone();
        let fields = projected
            .as_object_mut()
            .ok_or_else(|| anyhow!("native observation invalid"))?;
        fields.remove("body");
        fields.insert(
            "_observation_body_ref".into(),
            json!({"manifest_id":id,"application_id":application}),
        );
        sqlx::query("update runtime_events set payload=($3::jsonb->0),raw_json_payloads=jsonb_strip_nulls(jsonb_build_object('payload',($3::jsonb->1))),observation_body_content_id=null,observation_body_manifest_id=$4 where id=$1 and flow_run_id=$2")
            .bind(event_id).bind(flow).bind(lossless_json_parameter(&projected)).bind(id).execute(&mut *tx).await?;
        let restored:Value=sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
            .bind(event_id).fetch_one(&mut *tx).await?;
        anyhow::ensure!(
            restored == original,
            "native historical reconstruction mismatch"
        );
        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "native_snapshots/_tests.rs"]
mod tests;
