use super::*;

impl PgControlPlaneStore {
    /// Resolve a single immutable snapshot within the same run/node/invocation/attempt.
    /// No chains, cross-run lookup, client reconstruction or read-side persistence.
    pub(super) async fn native_trajectory_body(
        &self,
        flow: Uuid,
        node: Uuid,
        payload: &Value,
    ) -> Result<String> {
        let Some(reference) = payload.get("body_ref") else {
            return expand_snapshot(payload);
        };
        let key = reference["step_key"]
            .as_str()
            .ok_or_else(|| anyhow!("native snapshot key missing"))?;
        // New snapshots use opaque unique keys, never the legacy mutable request/reply keys.
        Uuid::parse_str(key).map_err(|_| anyhow!("native snapshot key invalid"))?;
        let pointer = reference["pointer"]
            .as_str()
            .ok_or_else(|| anyhow!("native snapshot pointer missing"))?;
        let invocation = payload["invocation_id"]
            .as_str()
            .ok_or_else(|| anyhow!("native invocation missing"))?;
        let attempt = payload["provider_attempt_index"]
            .as_i64()
            .ok_or_else(|| anyhow!("native attempt missing"))?;
        let source: Option<Value> = sqlx::query_scalar(
            "select runtime_original_json(e.payload,e.raw_json_payloads,'payload')
             from provider_semantic_trajectory_steps s join runtime_events e on e.id=s.body_event_id
             where s.flow_run_id=$1 and s.node_run_id=$2 and s.invocation_id=$3
               and s.provider_attempt_index=$4 and s.step_key=$5
               and s.metadata->>'source'='ai_native' and s.snapshot_count=1",
        )
        .bind(flow)
        .bind(node)
        .bind(invocation)
        .bind(attempt)
        .bind(key)
        .fetch_optional(self.pool())
        .await?;
        let source = source.ok_or_else(|| anyhow!("native snapshot unavailable"))?;
        anyhow::ensure!(
            source.get("body_ref").is_none(),
            "native snapshot chains forbidden"
        );
        let body: Value = serde_json::from_str(&expand_snapshot(&source)?)?;
        let detail = body
            .pointer(pointer)
            .ok_or_else(|| anyhow!("native snapshot pointer unavailable"))?;
        Ok(serde_json::to_string(detail)?)
    }
}

fn expand_snapshot(payload: &Value) -> Result<String> {
    let body = payload["body"]
        .as_str()
        .ok_or_else(|| anyhow!("native trajectory body missing"))?;
    if payload["body_format"] != "native_reply_v2" {
        return Ok(body.to_owned());
    }
    let mut value: Value = serde_json::from_str(body)?;
    let content = value
        .get("final_content")
        .cloned()
        .ok_or_else(|| anyhow!("native reply content missing"))?;
    let result = value["result"]
        .as_object_mut()
        .ok_or_else(|| anyhow!("native reply result missing"))?;
    anyhow::ensure!(
        !result.contains_key("final_content"),
        "native reply content conflict"
    );
    result.insert("final_content".into(), content);
    Ok(serde_json::to_string(&value)?)
}
