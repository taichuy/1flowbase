use super::*;

const LLM_ROUTING_COUNTER_TTL: time::Duration = time::Duration::hours(1);

pub(super) async fn llm_request_runtimes(
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<Vec<CompiledLlmRuntime>> {
    let request_count = if node
        .config
        .get("retry_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        1 + node
            .config
            .get("max_retries")
            .and_then(Value::as_u64)
            .unwrap_or(1) as usize
    } else {
        1
    };

    let Some(routing) = runtime.routing.as_ref() else {
        return Ok(vec![runtime.clone(); request_count]);
    };
    if routing.routing_mode != LlmRoutingMode::FailoverQueue || routing.queue_targets.is_empty() {
        return Ok(vec![runtime.clone(); request_count]);
    }

    let mut request_runtimes = Vec::with_capacity(request_count);
    for attempt_index in 0..request_count {
        let target_index = match routing.distribution_rule {
            crate::compiled_plan::LlmDistributionRule::RoundRobin
                if routing.queue_targets.len() > 1 =>
            {
                let distribution_key = routing
                    .distribution_key
                    .as_deref()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        anyhow!("round_robin llm routing is missing distribution_key")
                    })?;
                let counter = runtime_context
                    .next_llm_routing_counter(distribution_key, Some(LLM_ROUTING_COUNTER_TTL))
                    .await?;
                (counter - 1).rem_euclid(routing.queue_targets.len() as i64) as usize
            }
            crate::compiled_plan::LlmDistributionRule::RetryRoundRobin
                if routing.queue_targets.len() > 1 =>
            {
                attempt_index % routing.queue_targets.len()
            }
            _ => 0,
        };
        let target = &routing.queue_targets[target_index];
        let mut request_runtime = runtime.clone();
        request_runtime.provider_instance_id = target.provider_instance_id.clone();
        request_runtime.provider_instance_display_name =
            target.provider_instance_display_name.clone();
        request_runtime.provider_code = target.provider_code.clone();
        request_runtime.protocol = target.protocol.clone();
        request_runtime.model = target.upstream_model_id.clone();
        request_runtimes.push(request_runtime);
    }

    Ok(request_runtimes)
}

pub(super) struct AttemptMetricInput<'a> {
    pub(super) attempt_index: usize,
    pub(super) retry_reason: Option<&'a str>,
    pub(super) runtime: &'a CompiledLlmRuntime,
    pub(super) status: &'a str,
    pub(super) failed_after_first_token: bool,
    pub(super) error_payload: Option<&'a Value>,
    pub(super) usage: &'a ProviderUsage,
    pub(super) event_count: usize,
    pub(super) started_at: OffsetDateTime,
    pub(super) first_token_at: Option<OffsetDateTime>,
    pub(super) finished_at: OffsetDateTime,
    pub(super) time_to_first_token_ms: Option<u64>,
}

pub(super) fn build_attempt_metric(input: AttemptMetricInput<'_>) -> Value {
    json!({
        "attempt_index": input.attempt_index,
        "is_retry": input.attempt_index > 0,
        "retry_reason": input.retry_reason,
        "provider_instance_id": input.runtime.provider_instance_id,
        "provider_instance_display_name": input.runtime.provider_instance_display_name,
        "provider_code": input.runtime.provider_code,
        "protocol": input.runtime.protocol,
        "upstream_model_id": input.runtime.model,
        "model": input.runtime.model,
        "status": input.status,
        "failed_after_first_token": input.failed_after_first_token,
        "event_count": input.event_count,
        "started_at": offset_datetime_json(Some(input.started_at)),
        "first_token_at": offset_datetime_json(input.first_token_at),
        "finished_at": offset_datetime_json(Some(input.finished_at)),
        "time_to_first_token_ms": input.time_to_first_token_ms,
        "usage": serde_json::to_value(input.usage).unwrap_or(Value::Null),
        "error_code": input.error_payload
            .and_then(|payload| payload.get("error_code"))
            .cloned()
            .unwrap_or(Value::Null),
        "error_message_ref": input.error_payload
            .and_then(|payload| payload.get("message"))
            .and_then(Value::as_str)
            .map(|message| format!("runtime_artifact:inline:error:{message}"))
            .map(Value::String)
            .unwrap_or(Value::Null),
    })
}

pub(super) fn build_llm_metrics_payload(
    runtime: &CompiledLlmRuntime,
    usage: ProviderUsage,
    finish_reason: Option<ProviderFinishReason>,
    event_count: usize,
    attempts: Vec<Value>,
    first_token_at: Option<OffsetDateTime>,
    time_to_first_token_ms: Option<u64>,
) -> Value {
    json!({
        "provider_instance_id": runtime.provider_instance_id,
        "provider_instance_display_name": runtime.provider_instance_display_name,
        "provider_code": runtime.provider_code,
        "protocol": runtime.protocol,
        "model": runtime.model,
        "event_count": event_count,
        "first_token_at": offset_datetime_json(first_token_at),
        "time_to_first_token_ms": time_to_first_token_ms,
        "route": build_llm_route_payload(runtime),
        "usage": serde_json::to_value(&usage).unwrap_or(Value::Null),
        "finish_reason": finish_reason
            .as_ref()
            .map(|reason| serde_json::to_value(reason).unwrap_or(Value::Null))
            .unwrap_or(Value::Null),
        "queue_snapshot_id": runtime
            .routing
            .as_ref()
            .and_then(|routing| routing.queue_snapshot_id.clone())
            .map(Value::String)
            .unwrap_or(Value::Null),
        "attempts": attempts,
    })
}

pub(super) fn offset_datetime_json(value: Option<OffsetDateTime>) -> Value {
    value
        .and_then(|datetime| datetime.format(&Rfc3339).ok())
        .map(Value::String)
        .unwrap_or(Value::Null)
}

pub(super) fn build_llm_route_payload(runtime: &CompiledLlmRuntime) -> Value {
    match runtime.routing.as_ref() {
        Some(routing) => json!({
            "routing_mode": routing.routing_mode,
            "fixed_model_target": routing.fixed_model_target,
            "queue_template_id": routing.queue_template_id,
            "provider_instance_id": runtime.provider_instance_id,
            "provider_code": runtime.provider_code,
            "upstream_model_id": runtime.model,
            "protocol": runtime.protocol,
        }),
        None => json!({
            "routing_mode": "fixed_model",
            "provider_instance_id": runtime.provider_instance_id,
            "provider_code": runtime.provider_code,
            "upstream_model_id": runtime.model,
            "protocol": runtime.protocol,
        }),
    }
}
