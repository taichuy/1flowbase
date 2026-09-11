use super::*;

impl PgControlPlaneStore {
    pub(super) async fn rebuild_gateway_projection(
        &self,
        scope: &GatewayLogScope,
        conversation_id: Uuid,
    ) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        let visible:bool=sqlx::query_scalar("select exists(select 1 from gateway_log_conversations where id=$1 and scope_id=$2 and application_id=$3 and ($4::uuid is null or api_key_id=$4))")
            .bind(conversation_id).bind(scope.scope_id).bind(scope.application_id).bind(scope.api_key_id).fetch_one(&mut *tx).await?;
        if !visible {
            return Ok(false);
        }
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("gateway-log:{}", scope.application_id))
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from gateway_log_output_items where conversation_id=$1")
            .bind(conversation_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from gateway_log_tool_results where conversation_id=$1")
            .bind(conversation_id)
            .execute(&mut *tx)
            .await?;
        // Fixed-size keyset batches bound memory; no ephemeral artifacts are read.
        let mut after = Uuid::nil();
        loop {
            let rows=sqlx::query("select g.flow_run_id,g.context,f.started_at from gateway_log_invocations g join flow_runs f on f.id=g.flow_run_id where g.conversation_id=$1 and g.flow_run_id>$2 order by g.flow_run_id limit 250")
                .bind(conversation_id).bind(after).fetch_all(&mut *tx).await?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                after = row.try_get("flow_run_id")?;
                let context: Value = row.try_get("context")?;
                if let Some(results) = context.get("tool_results").and_then(Value::as_array) {
                    Self::project_gateway_tool_results(
                        &mut tx,
                        conversation_id,
                        after,
                        row.try_get("started_at")?,
                        results,
                    )
                    .await?;
                }
                let mut sequence = 0_i64;
                loop {
                    let events=sqlx::query("select * from runtime_events where flow_run_id=$1 and event_type='provider_output_item_done' and sequence>$2 order by sequence limit 250")
                        .bind(after).bind(sequence).fetch_all(&mut *tx).await?;
                    if events.is_empty() {
                        break;
                    }
                    for event in events {
                        let event = map_runtime_event_record(event)?;
                        sequence = event.sequence;
                        Self::project_gateway_output(&mut tx, &event).await?;
                    }
                }
            }
        }
        tx.commit().await?;
        Ok(true)
    }
}
