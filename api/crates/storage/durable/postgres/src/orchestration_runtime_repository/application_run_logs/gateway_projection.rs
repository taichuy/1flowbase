use super::*;

impl PgControlPlaneStore {
    // Shared writers and an exclusive rebuild lock serialize projection replacement
    // without acquiring flow-run locks in the opposite order to event appends.
    pub(super) async fn lock_gateway_projection(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        application_id: Uuid,
    ) -> Result<()> {
        sqlx::query("select pg_advisory_xact_lock_shared(hashtextextended($1,0))")
            .bind(format!("gateway-log:{application_id}"))
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    pub(super) async fn project_gateway_output(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        event: &domain::RuntimeEventRecord,
    ) -> Result<()> {
        if event.event_type != "provider_output_item_done" {
            return Ok(());
        }
        let application_id = sqlx::query_scalar::<_, Uuid>(
            "select application_id from gateway_log_invocations where flow_run_id=$1",
        )
        .bind(event.flow_run_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(application_id) = application_id else {
            return Ok(());
        };
        Self::lock_gateway_projection(tx, application_id).await?;
        let Some(item) = event.payload.get("item") else {
            return Ok(());
        };
        let Some(id) = item
            .get("call_id")
            .or_else(|| item.get("id"))
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        let kind = if item.get("call_id").is_some() {
            "tool"
        } else {
            item.get("type")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        };
        sqlx::query("insert into gateway_log_output_items(owner_id,item_key,conversation_id,turn_id,flow_run_id,sequence,observed_at,item) select coalesce(conversation_id,flow_run_id),$2,conversation_id,turn_id,flow_run_id,$3,$4,$5 from gateway_log_invocations where flow_run_id=$1 on conflict(owner_id,item_key) do update set conflicting=gateway_log_output_items.conflicting or gateway_log_output_items.item is distinct from excluded.item, flow_run_id=case when (excluded.flow_run_id,excluded.sequence)<(gateway_log_output_items.flow_run_id,gateway_log_output_items.sequence) then excluded.flow_run_id else gateway_log_output_items.flow_run_id end, turn_id=case when (excluded.flow_run_id,excluded.sequence)<(gateway_log_output_items.flow_run_id,gateway_log_output_items.sequence) then excluded.turn_id else gateway_log_output_items.turn_id end, sequence=case when (excluded.flow_run_id,excluded.sequence)<(gateway_log_output_items.flow_run_id,gateway_log_output_items.sequence) then excluded.sequence else gateway_log_output_items.sequence end, observed_at=case when (excluded.flow_run_id,excluded.sequence)<(gateway_log_output_items.flow_run_id,gateway_log_output_items.sequence) then excluded.observed_at else gateway_log_output_items.observed_at end, item=case when (excluded.flow_run_id,excluded.sequence)<(gateway_log_output_items.flow_run_id,gateway_log_output_items.sequence) then excluded.item else gateway_log_output_items.item end")
            .bind(event.flow_run_id).bind(format!("{kind}:{id}")).bind(event.sequence).bind(event.created_at).bind(item).execute(&mut **tx).await?;
        Ok(())
    }

    pub(super) async fn project_gateway_tool_results(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        conversation_id: Uuid,
        run_id: Uuid,
        received_at: OffsetDateTime,
        results: &[Value],
    ) -> Result<()> {
        for result in results {
            let Some(call_id) = result
                .get("call_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.len() <= 256)
            else {
                continue;
            };
            sqlx::query("insert into gateway_log_tool_results(conversation_id,call_id,flow_run_id,received_at,result) values($1,$2,$3,$4,$5) on conflict(conversation_id,call_id) do update set conflicting=gateway_log_tool_results.conflicting or gateway_log_tool_results.result is distinct from excluded.result, flow_run_id=case when excluded.flow_run_id<gateway_log_tool_results.flow_run_id then excluded.flow_run_id else gateway_log_tool_results.flow_run_id end, received_at=case when excluded.flow_run_id<gateway_log_tool_results.flow_run_id then excluded.received_at else gateway_log_tool_results.received_at end, result=case when excluded.flow_run_id<gateway_log_tool_results.flow_run_id then excluded.result else gateway_log_tool_results.result end")
                .bind(conversation_id).bind(call_id).bind(run_id).bind(received_at).bind(result).execute(&mut **tx).await?;
        }
        Ok(())
    }
}
