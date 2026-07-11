use anyhow::Result;
use plugin_framework::provider_contract::ProviderStreamEvent;
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::{
    sync::mpsc,
    time::{self as tokio_time, Duration, MissedTickBehavior},
};
use uuid::Uuid;

use crate::{
    capability_runtime::{host_tool_capability_id, mcp_tool_capability_id},
    ports::{
        AppendCapabilityInvocationInput, AppendContextProjectionInput,
        AppendModelFailoverAttemptLedgerInput, AppendUsageLedgerInput,
        LinkUsageLedgerToModelFailoverAttemptInput, OrchestrationRuntimeRepository,
    },
    runtime_observability::{
        append_host_event, append_provider_stream_events_raw,
        append_provider_stream_events_raw_filtered,
        projection::{estimate_tokens_for_text, model_input_hash},
        LiveEventCoalescer, PROVIDER_DELTA_COALESCE_MAX_BYTES,
        PROVIDER_DELTA_COALESCE_MAX_DELAY_MS,
    },
};

use super::super::{
    llm_observability_refs::LlmDebugObservabilityRefs,
    persistence::model_attempts::enqueue_provider_request_log_tasks,
};

pub(super) async fn run_live_event_persister<R>(
    repository: R,
    flow_run_id: Uuid,
    node_run_id: Uuid,
    span_id: Uuid,
    persist_text_run_events: bool,
    mut receiver: mpsc::UnboundedReceiver<ProviderStreamEvent>,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let mut coalescer = LiveEventCoalescer::new(PROVIDER_DELTA_COALESCE_MAX_BYTES);
    let mut flush_interval =
        tokio_time::interval(Duration::from_millis(PROVIDER_DELTA_COALESCE_MAX_DELAY_MS));
    flush_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            maybe_event = receiver.recv() => {
                let Some(event) = maybe_event else {
                    break;
                };
                write_provider_event_batch(
                    &repository,
                    flow_run_id,
                    Some(node_run_id),
                    Some(span_id),
                    persist_text_run_events,
                    &coalescer.push(event),
                )
                .await?;
            }
            _ = flush_interval.tick() => {
                write_provider_event_batch(
                    &repository,
                    flow_run_id,
                    Some(node_run_id),
                    Some(span_id),
                    persist_text_run_events,
                    &coalescer.flush_buffered(),
                )
                .await?;
            }
        }
    }

    write_provider_event_batch(
        &repository,
        flow_run_id,
        Some(node_run_id),
        Some(span_id),
        persist_text_run_events,
        &coalescer.finish(),
    )
    .await?;

    Ok(())
}

async fn write_provider_event_batch<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Option<Uuid>,
    span_id: Option<Uuid>,
    persist_text_run_events: bool,
    events: &[ProviderStreamEvent],
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    if events.is_empty() {
        return Ok(());
    }

    if persist_text_run_events {
        append_provider_stream_events_raw(repository, flow_run_id, node_run_id, span_id, events)
            .await?;
    } else {
        append_provider_stream_events_raw_filtered(
            repository,
            flow_run_id,
            node_run_id,
            span_id,
            events,
            |event| !matches!(event, ProviderStreamEvent::TextDelta { .. }),
        )
        .await?;
    }
    for event in events {
        append_provider_capability_intent(repository, flow_run_id, node_run_id, span_id, event)
            .await?;
    }

    Ok(())
}

pub(super) async fn persist_llm_context_observability<R>(
    repository: &R,
    scope_id: Uuid,
    application_name: &str,
    task_queue: Option<&std::sync::Arc<dyn crate::ports::TaskQueue>>,
    flow_run_id: Uuid,
    node_run_id: Uuid,
    span_id: Uuid,
    node_input: Value,
    metrics_payload: &Value,
    error_payload: Option<&Value>,
) -> Result<LlmDebugObservabilityRefs>
where
    R: OrchestrationRuntimeRepository,
{
    let model_input = json!({
        "node_input": node_input,
        "provider": metrics_payload.get("provider_code").cloned().unwrap_or(Value::Null),
        "model": metrics_payload.get("model").cloned().unwrap_or(Value::Null),
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

    let usage = metrics_payload.get("usage").cloned();
    let raw_usage = usage.clone().unwrap_or_else(|| json!({}));
    let usage_status = if usage.is_some() && error_payload.is_none() {
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
        metrics_payload,
        error_payload,
    )
    .await?;
    enqueue_provider_request_log_tasks(
        task_queue,
        scope_id,
        application_name,
        flow_run_id,
        &attempts,
        metrics_payload,
    )
    .await;
    let usage_attempt_id = winner_attempt_id(&attempts);

    let usage_ledger = repository
        .append_usage_ledger(&AppendUsageLedgerInput {
            flow_run_id,
            node_run_id: Some(node_run_id),
            span_id: Some(span_id),
            failover_attempt_id: usage_attempt_id,
            provider_instance_id: metrics_payload
                .get("provider_instance_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok()),
            gateway_route_id: None,
            model_id: metrics_payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string),
            upstream_model_id: metrics_payload
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

async fn append_model_attempts_from_metrics<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Uuid,
    span_id: Uuid,
    projection: &domain::ContextProjectionRecord,
    metrics_payload: &Value,
    error_payload: Option<&Value>,
) -> Result<Vec<domain::ModelFailoverAttemptLedgerRecord>>
where
    R: OrchestrationRuntimeRepository,
{
    let mut attempt_payloads = metrics_payload
        .get("attempts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if attempt_payloads.is_empty() {
        attempt_payloads.push(json!({
            "attempt_index": 0,
            "provider_instance_id": metrics_payload.get("provider_instance_id").cloned().unwrap_or(Value::Null),
            "provider_code": metrics_payload.get("provider_code").cloned().unwrap_or(Value::Null),
            "protocol": metrics_payload.get("protocol").cloned().unwrap_or(Value::Null),
            "upstream_model_id": metrics_payload.get("model").cloned().unwrap_or(Value::Null),
            "status": if error_payload.is_some() { "failed" } else { "succeeded" },
            "failed_after_first_token": false,
        }));
    }

    let mut records = Vec::with_capacity(attempt_payloads.len());
    for selected_attempt in attempt_payloads {
        let status = selected_attempt
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(if error_payload.is_some() {
                "failed"
            } else {
                "succeeded"
            });

        let record = repository
            .append_model_failover_attempt_ledger(&AppendModelFailoverAttemptLedgerInput {
                flow_run_id,
                node_run_id: Some(node_run_id),
                llm_turn_span_id: Some(span_id),
                queue_snapshot_id: metrics_payload
                    .get("queue_snapshot_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok()),
                attempt_index: selected_attempt
                    .get("attempt_index")
                    .and_then(Value::as_i64)
                    .unwrap_or(records.len() as i64) as i32,
                provider_instance_id: selected_attempt
                    .get("provider_instance_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok()),
                provider_code: selected_attempt
                    .get("provider_code")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                upstream_model_id: selected_attempt
                    .get("upstream_model_id")
                    .or_else(|| selected_attempt.get("model"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                protocol: selected_attempt
                    .get("protocol")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                request_ref: Some(projection.model_input_ref.clone()),
                request_hash: Some(projection.model_input_hash.clone()),
                started_at: OffsetDateTime::now_utc(),
                first_token_at: parse_attempt_first_token_at(&selected_attempt),
                finished_at: Some(OffsetDateTime::now_utc()),
                status: status.to_string(),
                failed_after_first_token: selected_attempt
                    .get("failed_after_first_token")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                upstream_request_id: selected_attempt
                    .get("upstream_request_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                error_code: selected_attempt
                    .get("error_code")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| {
                        (status != "succeeded").then(|| {
                            error_payload
                                .and_then(|payload| payload.get("error_kind"))
                                .and_then(Value::as_str)
                                .unwrap_or("provider_error")
                                .to_string()
                        })
                    }),
                error_message_ref: selected_attempt
                    .get("error_message_ref")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                usage_ledger_id: None,
                cost_ledger_id: None,
                response_ref: selected_attempt
                    .get("response_ref")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
            .await?;
        records.push(record);
    }

    Ok(records)
}

fn winner_attempt_id(attempts: &[domain::ModelFailoverAttemptLedgerRecord]) -> Option<Uuid> {
    attempts
        .iter()
        .find(|attempt| attempt.status == "succeeded")
        .map(|attempt| attempt.id)
}

fn parse_attempt_first_token_at(attempt: &Value) -> Option<OffsetDateTime> {
    attempt
        .get("first_token_at")
        .and_then(Value::as_str)
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
}

fn usage_i64(usage: &Value, field: &str) -> Option<i64> {
    usage.get(field).and_then(Value::as_i64)
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
