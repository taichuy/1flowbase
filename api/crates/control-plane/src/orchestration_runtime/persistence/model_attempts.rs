use crate::ports::ProviderRequestLogTask;

use super::*;

pub(in crate::orchestration_runtime) async fn append_model_attempts_from_metrics<R>(
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
                started_at: parse_attempt_time(&selected_attempt, "started_at")
                    .unwrap_or_else(OffsetDateTime::now_utc),
                first_token_at: parse_attempt_first_token_at(&selected_attempt),
                finished_at: Some(
                    parse_attempt_time(&selected_attempt, "finished_at")
                        .unwrap_or_else(OffsetDateTime::now_utc),
                ),
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

pub(crate) async fn enqueue_provider_request_log_tasks(
    task_queue: Option<&std::sync::Arc<dyn crate::ports::TaskQueue>>,
    scope_id: Uuid,
    application_name: &str,
    flow_run_id: Uuid,
    attempts: &[domain::ModelFailoverAttemptLedgerRecord],
    metrics_payload: &Value,
) {
    let Some(task_queue) = task_queue else {
        return;
    };
    let metrics = metrics_payload.get("attempts").and_then(Value::as_array);
    for (index, attempt) in attempts.iter().enumerate() {
        let metric = metrics
            .and_then(|items| items.get(index))
            .unwrap_or(metrics_payload);
        let task = provider_request_log_task_from_attempt(
            scope_id,
            attempt.id,
            flow_run_id,
            application_name,
            attempt.started_at,
            attempt.finished_at.unwrap_or(attempt.started_at),
            metric,
        );
        let payload = match serde_json::to_value(&task) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(attempt_id = %attempt.id, error = %error, "failed to serialize provider request log task");
                continue;
            }
        };
        if let Err(error) = task_queue
            .enqueue(
                crate::ports::PROVIDER_REQUEST_LOG_QUEUE,
                payload,
                Some(&attempt.id.to_string()),
            )
            .await
        {
            tracing::warn!(attempt_id = %attempt.id, error = %error, "failed to enqueue provider request log task");
        }
    }
}

pub(super) fn provider_request_log_task_from_attempt(
    scope_id: Uuid,
    attempt_id: Uuid,
    flow_run_id: Uuid,
    application_name: &str,
    started_at: OffsetDateTime,
    finished_at: OffsetDateTime,
    attempt: &Value,
) -> ProviderRequestLogTask {
    let failed_after_first_token = attempt
        .get("failed_after_first_token")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let event_count = attempt
        .get("event_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let usage = attempt.get("usage").cloned().unwrap_or_else(|| json!({}));
    let output_tokens = usage_i64(&usage, "output_tokens");
    let raw_status = attempt
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    let status = if failed_after_first_token {
        "failed_after_first_token"
    } else if raw_status == "succeeded" && event_count == 0 && output_tokens.unwrap_or(0) == 0 {
        "empty_response"
    } else {
        raw_status
    };
    let first_token_at = parse_attempt_first_token_at(attempt);
    ProviderRequestLogTask {
        scope_id,
        attempt_id,
        flow_run_id,
        application_name: application_name.to_string(),
        attempt_index: attempt
            .get("attempt_index")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32
            + 1,
        is_retry: attempt
            .get("is_retry")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        retry_reason: attempt
            .get("retry_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_instance_id: attempt
            .get("provider_instance_id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok()),
        provider_instance_display_name: attempt
            .get("provider_instance_display_name")
            .and_then(Value::as_str)
            .map(str::to_string),
        provider_code: attempt
            .get("provider_code")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        protocol: attempt
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        upstream_model_id: attempt
            .get("upstream_model_id")
            .or_else(|| attempt.get("model"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        reasoning_effort: attempt
            .get("reasoning_effort")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: status.to_string(),
        error_code: attempt
            .get("error_code")
            .and_then(Value::as_str)
            .map(str::to_string),
        failed_after_first_token,
        input_tokens: usage_i64(&usage, "input_tokens"),
        output_tokens,
        total_tokens: usage_i64(&usage, "total_tokens"),
        started_at,
        first_token_at,
        finished_at: Some(finished_at),
        time_to_first_token_ms: attempt
            .get("time_to_first_token_ms")
            .and_then(Value::as_i64),
        total_duration_ms: Some(
            (finished_at - started_at)
                .whole_milliseconds()
                .try_into()
                .unwrap_or(i64::MAX),
        ),
    }
}

pub(in crate::orchestration_runtime) fn winner_attempt_id(
    attempts: &[domain::ModelFailoverAttemptLedgerRecord],
) -> Option<Uuid> {
    attempts
        .iter()
        .find(|attempt| attempt.status == "succeeded")
        .map(|attempt| attempt.id)
}

fn parse_attempt_first_token_at(attempt: &Value) -> Option<OffsetDateTime> {
    parse_attempt_time(attempt, "first_token_at")
}

fn parse_attempt_time(attempt: &Value, field: &str) -> Option<OffsetDateTime> {
    attempt
        .get(field)
        .and_then(Value::as_str)
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
}

pub(super) fn usage_i64(usage: &Value, field: &str) -> Option<i64> {
    usage.get(field).and_then(Value::as_i64)
}
