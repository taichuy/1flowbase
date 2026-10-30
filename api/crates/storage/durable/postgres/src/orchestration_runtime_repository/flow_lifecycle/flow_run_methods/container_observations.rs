use super::*;
use control_plane_contracts::ports::{
    ContainerObservationOperation as Operation, ContainerObservationResult as Receipt,
    PendingContainerObservation,
};

impl PgControlPlaneStore {
    pub(super) async fn list_pending_container_observations(
        &self,
        flow_run_id: Uuid,
        after_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<PendingContainerObservation>> {
        let rows=sqlx::query("select o.id,o.flow_run_id,e.node_run_id,n.started_at,runtime_original_json(o.trace_payload,o.raw_json_payloads,'trace_payload') as trace_payload from flow_container_observations o join flow_container_executions e using(flow_run_id,execution_id) join node_runs n on n.id=e.node_run_id where o.flow_run_id=$1 and o.completed_at is null and o.sequence>coalesce((select sequence from flow_container_observations where id=$2 and flow_run_id=$1),0) order by o.sequence limit $3")
            .bind(flow_run_id).bind(after_id).bind(limit.clamp(1,32)).fetch_all(self.pool()).await?;
        Ok(rows
            .into_iter()
            .map(|row| PendingContainerObservation {
                id: row.get("id"),
                flow_run_id: row.get("flow_run_id"),
                node_run_id: row.get("node_run_id"),
                started_at: row.get("started_at"),
                trace_payload: row.get("trace_payload"),
            })
            .collect())
    }

    pub(super) async fn apply_container_observation(
        &self,
        observation_id: Uuid,
        record_id: Uuid,
        operation: &Operation,
    ) -> Result<Receipt> {
        let flow_run_id: Uuid =
            sqlx::query_scalar("select flow_run_id from flow_container_observations where id=$1")
                .bind(observation_id)
                .fetch_one(self.pool())
                .await?;
        let operation_run = match operation {
            Operation::Span(input) => input.flow_run_id,
            Operation::ContextProjection(input) => input.flow_run_id,
            Operation::Attempt(input) => input.flow_run_id,
            Operation::Usage(input) => input.flow_run_id,
            Operation::RunEvent(input) => input.flow_run_id,
            Operation::RuntimeEvent(input) => input.flow_run_id,
            Operation::Item(input) => input.flow_run_id,
            Operation::Capability(input) => input.flow_run_id,
        };
        if flow_run_id != operation_run {
            return Err(ControlPlaneError::Conflict("container_observation_scope_mismatch").into());
        }
        match operation {
            Operation::Span(input) => {
                if let Some(row) = sqlx::query(
                    r#"select id,
                flow_run_id,
                node_run_id,
                parent_span_id,
                kind,
                name,
                status,
                capability_id,
                input_ref,
                output_ref,
                error_payload,
                metadata,
                started_at,
                finished_at from runtime_spans where id=$1"#,
                )
                .bind(record_id)
                .fetch_optional(self.pool())
                .await?
                {
                    return Ok(Receipt::Span(map_runtime_span_record(row)?));
                }
                Ok(Receipt::Span(
                    self.append_runtime_span_with_id(input, record_id).await?,
                ))
            }
            Operation::ContextProjection(input) => {
                if let Some(row) = sqlx::query(
                    r#"select id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                projection_kind,
                merge_stage_ref,
                source_transcript_ref,
                source_item_refs,
                compaction_event_id,
                summary_version,
                model_input_ref,
                model_input_hash,
                compacted_summary_ref,
                previous_projection_id,
                token_estimate,
                provider_continuation_metadata,
                created_at from runtime_context_projections where id=$1"#,
                )
                .bind(record_id)
                .fetch_optional(self.pool())
                .await?
                {
                    return Ok(Receipt::ContextProjection(map_context_projection_record(
                        row,
                    )));
                }
                Ok(Receipt::ContextProjection(
                    self.append_context_projection_with_id(input, record_id)
                        .await?,
                ))
            }
            Operation::Attempt(input) => {
                if let Some(row) = sqlx::query(
                    r#"select id,
                flow_run_id,
                node_run_id,
                llm_turn_span_id,
                queue_snapshot_id,
                attempt_index,
                provider_instance_id,
                provider_code,
                upstream_model_id,
                protocol,
                request_ref,
                request_hash,
                started_at,
                first_token_at,
                finished_at,
                status,
                failed_after_first_token,
                upstream_request_id,
                error_code,
                error_message_ref,
                usage_ledger_id,
                cost_ledger_id,
                response_ref from model_failover_attempt_ledger where id=$1"#,
                )
                .bind(record_id)
                .fetch_optional(self.pool())
                .await?
                {
                    return Ok(Receipt::Attempt(map_model_failover_attempt_ledger_record(
                        row,
                    )));
                }
                Ok(Receipt::Attempt(
                    self.append_model_failover_attempt_ledger_with_id(input, record_id)
                        .await?,
                ))
            }
            Operation::Usage(input) => {
                if let Some(row) = sqlx::query(
                    r#"select id,
                flow_run_id,
                node_run_id,
                span_id,
                failover_attempt_id,
                provider_instance_id,
                gateway_route_id,
                model_id,
                upstream_model_id,
                upstream_request_id,
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
                total_tokens,
                input_cache_hit_tokens,
                input_cache_miss_tokens,
                cache_read_tokens,
                cache_write_tokens,
                price_snapshot,
                cost_snapshot,
                usage_status,
                raw_usage,
                normalized_usage,
                created_at from runtime_usage_ledger where id=$1"#,
                )
                .bind(record_id)
                .fetch_optional(self.pool())
                .await?
                {
                    return Ok(Receipt::Usage(map_usage_ledger_record(row)?));
                }
                Ok(Receipt::Usage(
                    self.append_usage_ledger_with_id(input, record_id).await?,
                ))
            }
            Operation::RunEvent(input) => {
                if let Some(row)=sqlx::query(r#"select id,
                flow_run_id,
                node_run_id,
                sequence,
                event_type,
                runtime_original_json(payload, flow_run_events.raw_json_payloads, 'payload') as payload,
                created_at from flow_run_events where id=$1"#).bind(record_id).fetch_optional(self.pool()).await? {
                    return Ok(Receipt::RunEvent(map_run_event_record(row)));
                }
                Ok(Receipt::RunEvent(
                    self.append_run_event_with_id(input, record_id, true)
                        .await?,
                ))
            }
            Operation::RuntimeEvent(input) => {
                if let Some(row)=sqlx::query(r#"select id,
                flow_run_id,
                node_run_id,
                span_id,
                parent_span_id,
                sequence,
                event_type,
                layer,
                source,
                trust_level,
                item_id,
                ledger_ref,
                runtime_event_original_payload(payload, runtime_events.raw_json_payloads, flow_run_id) as payload,
                visibility,
                durability,
                created_at from runtime_events where id=$1"#).bind(record_id).fetch_optional(self.pool()).await? {
                    return Ok(Receipt::RuntimeEvent(map_runtime_event_record(row)?));
                }
                Ok(Receipt::RuntimeEvent(
                    self.append_runtime_event_with_id(input, record_id, true)
                        .await?,
                ))
            }
            Operation::Item(input) => {
                if let Some(row) = sqlx::query(
                    r#"select id,
                flow_run_id,
                span_id,
                kind,
                status,
                source_event_id,
                input_ref,
                output_ref,
                usage_ledger_id,
                trust_level,
                created_at,
                updated_at from runtime_items where id=$1"#,
                )
                .bind(record_id)
                .fetch_optional(self.pool())
                .await?
                {
                    return Ok(Receipt::Item(map_runtime_item_record(row)?));
                }
                Ok(Receipt::Item(
                    self.append_runtime_item_with_id(input, record_id).await?,
                ))
            }
            Operation::Capability(input) => {
                if let Some(row) = sqlx::query(
                    r#"select id,
                flow_run_id,
                span_id,
                capability_id,
                requested_by_span_id,
                requester_kind,
                arguments_ref,
                authorization_status,
                authorization_reason,
                result_ref,
                normalized_result,
                started_at,
                finished_at,
                error_payload,
                created_at from capability_invocations where id=$1"#,
                )
                .bind(record_id)
                .fetch_optional(self.pool())
                .await?
                {
                    return Ok(Receipt::Capability(map_capability_invocation_record(row)));
                }
                Ok(Receipt::Capability(
                    self.append_capability_invocation_with_id(input, record_id)
                        .await?,
                ))
            }
        }
    }

    pub(super) async fn finish_container_observation(
        &self,
        observation_id: Uuid,
        debug_refs: &Value,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let flow_run_id: Uuid =
            sqlx::query_scalar("select flow_run_id from flow_container_observations where id=$1")
                .bind(observation_id)
                .fetch_one(&mut *tx)
                .await?;
        lock_application_run_native_projection(&mut tx, flow_run_id).await?;
        sqlx::query("select id from flow_runs where id=$1 for update")
            .bind(flow_run_id)
            .fetch_one(&mut *tx)
            .await?;
        let row=sqlx::query("select o.sequence,e.node_run_id,e.last_observation_sequence,o.completed_at from flow_container_observations o join flow_container_executions e using(flow_run_id,execution_id) where o.id=$1 for update of o,e")
            .bind(observation_id).fetch_one(&mut *tx).await?;
        if row
            .get::<Option<OffsetDateTime>, _>("completed_at")
            .is_some()
        {
            tx.commit().await?;
            return Ok(());
        }
        let sequence: i64 = row.get("sequence");
        if sequence > row.get::<i64, _>("last_observation_sequence") {
            let node_run_id: Uuid = row.get("node_run_id");
            let mut debug:Value=sqlx::query_scalar("select runtime_original_json(debug_payload,raw_json_payloads,'debug_payload') from node_run_records where id=$1").bind(node_run_id).fetch_one(&mut *tx).await?;
            if let (Some(debug), Some(refs)) = (debug.as_object_mut(), debug_refs.as_object()) {
                debug.extend(refs.clone());
            }
            sqlx::query("update node_run_records set debug_payload=($2::jsonb->0),raw_json_payloads=(raw_json_payloads-'debug_payload') || jsonb_strip_nulls(jsonb_build_object('debug_payload',($2::jsonb->1))) where id=$1")
                .bind(node_run_id).bind(lossless_json_parameter(&debug)).execute(&mut *tx).await?;
            sqlx::query("update flow_container_executions set last_observation_sequence=$2 where node_run_id=$1").bind(node_run_id).bind(sequence).execute(&mut *tx).await?;
        }
        sqlx::query("update flow_container_observations set completed_at=now(),trace_payload=null,raw_json_payloads='{}' where id=$1")
            .bind(observation_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
