use super::model_attempts::{
    append_model_attempts_from_metrics, enqueue_provider_request_log_tasks, usage_i64,
    winner_attempt_id,
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

pub(super) async fn persist_llm_context_observability<R>(
    repository: &R,
    scope_id: Uuid,
    application_name: &str,
    task_queue: Option<&Arc<dyn crate::ports::TaskQueue>>,
    flow_run_id: Uuid,
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
            provider_continuation_metadata: json!({}),
        })
        .await?;

    let usage = trace.metrics_payload.get("usage").cloned();
    let raw_usage = usage.clone().unwrap_or_else(|| json!({}));
    let usage_status = if usage.is_some() && trace.error_payload.is_none() {
        domain::UsageLedgerStatus::Recorded
    } else {
        domain::UsageLedgerStatus::UnavailableError
    };

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
        &attempts,
        &trace.metrics_payload,
    )
    .await;
    let usage_attempt_id = winner_attempt_id(&attempts);

    let usage_ledger = repository
        .append_usage_ledger(&AppendUsageLedgerInput {
            flow_run_id,
            node_run_id: Some(node_run_id),
            span_id: Some(span_id),
            failover_attempt_id: usage_attempt_id,
            provider_instance_id: trace
                .metrics_payload
                .get("provider_instance_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok()),
            gateway_route_id: None,
            model_id: trace
                .metrics_payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string),
            upstream_model_id: trace
                .metrics_payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string),
            upstream_request_id: None,
            input_tokens: usage_i64(&raw_usage, "input_tokens"),
            cached_input_tokens: usage_i64(&raw_usage, "cached_input_tokens"),
            output_tokens: usage_i64(&raw_usage, "output_tokens"),
            reasoning_output_tokens: usage_i64(&raw_usage, "reasoning_tokens"),
            total_tokens: usage_i64(&raw_usage, "total_tokens"),
            input_cache_hit_tokens: usage_i64(&raw_usage, "input_cache_hit_tokens"),
            input_cache_miss_tokens: usage_i64(&raw_usage, "input_cache_miss_tokens"),
            cache_read_tokens: usage_i64(&raw_usage, "cache_read_tokens"),
            cache_write_tokens: usage_i64(&raw_usage, "cache_write_tokens"),
            price_snapshot: None,
            cost_snapshot: None,
            usage_status,
            raw_usage: raw_usage.clone(),
            normalized_usage: raw_usage,
        })
        .await?;
    if let Some(failover_attempt_id) = usage_attempt_id {
        repository
            .link_usage_ledger_to_model_failover_attempt(
                &LinkUsageLedgerToModelFailoverAttemptInput {
                    failover_attempt_id,
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
