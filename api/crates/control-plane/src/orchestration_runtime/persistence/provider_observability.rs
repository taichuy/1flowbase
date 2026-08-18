use super::model_attempts::{
    append_model_attempts_from_metrics, enqueue_provider_request_log_tasks, usage_i64,
};
use super::*;
use std::sync::Arc;

pub(super) async fn append_provider_stream_events<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    events: &[ProviderStreamEvent],
) -> Result<Vec<domain::RunEventRecord>>
where
    R: OrchestrationRuntimeRepository,
{
    let runtime_bus = RuntimeEventBus::new((events.len() + 4).max(16));
    let events =
        coalesce_provider_stream_events(&runtime_bus, events, PROVIDER_DELTA_COALESCE_MAX_BYTES)?;
    let records =
        append_provider_stream_events_raw(repository, flow_run_id, node_run_id, span_id, &events)
            .await?;
    for event in &events {
        append_provider_capability_intent(repository, flow_run_id, node_run_id, span_id, event)
            .await?;
    }
    Ok(records)
}

async fn append_provider_capability_intent<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    event: &ProviderStreamEvent,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let (capability_id, call) = match event {
        ProviderStreamEvent::ToolCallCommit { call } => (
            host_tool_capability_id(&call.name),
            serde_json::to_value(call)?,
        ),
        ProviderStreamEvent::McpCallCommit { call } => (
            mcp_tool_capability_id(&call.server, &call.method),
            serde_json::to_value(call)?,
        ),
        _ => return Ok(()),
    };

    let event = append_host_event(
        repository,
        flow_run_id,
        node_run_id,
        span_id,
        "capability_call_requested",
        domain::RuntimeEventLayer::Capability,
        json!({
            "provider_only_intent": true,
            "capability_id": capability_id,
            "requested_by": "model",
            "call": call,
        }),
    )
    .await?;
    repository
        .append_capability_invocation(&AppendCapabilityInvocationInput {
            flow_run_id,
            span_id,
            capability_id,
            requested_by_span_id: span_id,
            requester_kind: "model".to_string(),
            arguments_ref: Some(format!("runtime_artifact:inline:{}", event.id)),
            authorization_status: "requested".to_string(),
            authorization_reason: None,
            result_ref: None,
            normalized_result: None,
            started_at: None,
            finished_at: None,
            error_payload: None,
        })
        .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn persist_llm_context_observability<R>(
    repository: &R,
    scope_id: Uuid,
    application_name: &str,
    task_queue: Option<&Arc<dyn crate::ports::TaskQueue>>,
    flow_run_id: Uuid,
    user_id: Uuid,
    application_id: Uuid,
    conversation_id: Option<&str>,
    node_run_id: Uuid,
    span_id: Uuid,
    trace: &orchestration_runtime::execution_state::NodeExecutionTrace,
) -> Result<LlmDebugObservabilityRefs>
where
    R: OrchestrationRuntimeRepository,
{
    let model_input = json!({
        "node_input": trace.input_payload,
        "provider": trace.metrics_payload.get("provider_code").cloned().unwrap_or(Value::Null),
        "model": trace.metrics_payload.get("model").cloned().unwrap_or(Value::Null),
    });
    let model_input_hash = model_input_hash(&model_input);
    let invocation_context = repository
        .append_provider_invocation_context(&AppendProviderInvocationContextInput {
            scope_id,
            application_id,
            flow_run_id,
            invocation_span_id: span_id,
            actual_context: json!({
                "effective_system": trace.debug_payload.get("effective_system").cloned().unwrap_or_else(|| json!([])),
                "provider_messages": trace.debug_payload.get("provider_messages").cloned().unwrap_or_else(|| json!([])),
            }),
            context_epoch: trace
                .debug_payload
                .get("context_epoch")
                .cloned()
                .unwrap_or_else(|| json!({ "declaration": "unknown" })),
        })
        .await?;
    let projection = repository
        .append_context_projection(&AppendContextProjectionInput {
            flow_run_id,
            node_run_id: Some(node_run_id),
            llm_turn_span_id: Some(span_id),
            projection_kind: "managed_full".to_string(),
            merge_stage_ref: None,
            source_transcript_ref: None,
            source_item_refs: json!([]),
            compaction_event_id: None,
            summary_version: None,
            model_input_ref: format!("runtime_artifact:inline:{model_input_hash}"),
            model_input_hash,
            compacted_summary_ref: None,
            previous_projection_id: None,
            token_estimate: Some(estimate_tokens_for_text(&model_input.to_string())),
            provider_continuation_metadata: json!({
                "context_version_id": invocation_context.id,
                "context_epoch": trace.debug_payload.get("context_epoch").cloned().unwrap_or_else(|| json!({ "declaration": "unknown" })),
            }),
        })
        .await?;

    let attempts = append_model_attempts_from_metrics(
        repository,
        flow_run_id,
        node_run_id,
        span_id,
        &projection,
        &trace.metrics_payload,
        trace.error_payload.as_ref(),
    )
    .await?;
    enqueue_provider_request_log_tasks(
        task_queue,
        scope_id,
        application_name,
        flow_run_id,
        node_run_id,
        user_id,
        Some(application_id),
        conversation_id,
        &attempts,
        &trace.metrics_payload,
    )
    .await;
    let attempt_metrics = trace
        .metrics_payload
        .get("attempts")
        .and_then(Value::as_array);
    for (index, attempt) in attempts.iter().enumerate() {
        if attempt.usage_ledger_id.is_some() {
            continue;
        }
        let metric = attempt_metrics
            .and_then(|metrics| metrics.get(index))
            .unwrap_or(&trace.metrics_payload);
        let raw_usage = metric.get("usage").cloned().unwrap_or_else(|| json!({}));
        let has_usage = [
            "input_tokens",
            "output_tokens",
            "input_cache_hit_tokens",
            "input_cache_miss_tokens",
        ]
        .iter()
        .any(|field| usage_i64(&raw_usage, field).is_some());
        let input_tokens = usage_i64(&raw_usage, "input_tokens");
        let output_tokens = usage_i64(&raw_usage, "output_tokens");
        let total_tokens =
            usage_i64(&raw_usage, "total_tokens").or_else(|| match (input_tokens, output_tokens) {
                (Some(input), Some(output)) => Some(input.saturating_add(output)),
                _ => None,
            });
        let usage_ledger = repository
            .append_usage_ledger(&AppendUsageLedgerInput {
                flow_run_id,
                node_run_id: Some(node_run_id),
                span_id: Some(span_id),
                failover_attempt_id: Some(attempt.id),
                provider_instance_id: attempt.provider_instance_id,
                gateway_route_id: None,
                model_id: Some(attempt.upstream_model_id.clone()),
                upstream_model_id: Some(attempt.upstream_model_id.clone()),
                upstream_request_id: attempt.upstream_request_id.clone(),
                input_tokens,
                cached_input_tokens: usage_i64(&raw_usage, "cached_input_tokens"),
                output_tokens,
                reasoning_output_tokens: usage_i64(&raw_usage, "reasoning_tokens"),
                total_tokens,
                input_cache_hit_tokens: usage_i64(&raw_usage, "input_cache_hit_tokens"),
                input_cache_miss_tokens: usage_i64(&raw_usage, "input_cache_miss_tokens"),
                cache_read_tokens: usage_i64(&raw_usage, "cache_read_tokens"),
                cache_write_tokens: usage_i64(&raw_usage, "cache_write_tokens"),
                price_snapshot: None,
                cost_snapshot: None,
                usage_status: if has_usage {
                    domain::UsageLedgerStatus::Recorded
                } else {
                    domain::UsageLedgerStatus::UnavailableError
                },
                raw_usage: raw_usage.clone(),
                normalized_usage: raw_usage,
            })
            .await?;
        repository
            .link_usage_ledger_to_model_failover_attempt(
                &LinkUsageLedgerToModelFailoverAttemptInput {
                    failover_attempt_id: attempt.id,
                    usage_ledger_id: usage_ledger.id,
                },
            )
            .await?;
    }

    Ok(LlmDebugObservabilityRefs::from_records(
        &projection,
        &attempts,
    ))
}
