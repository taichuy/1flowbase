use super::*;

const LLM_ROUTING_COUNTER_TTL: time::Duration = time::Duration::hours(1);

pub(super) async fn llm_attempt_runtimes(
    runtime: &CompiledLlmRuntime,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<Vec<CompiledLlmRuntime>> {
    let Some(routing) = runtime.routing.as_ref() else {
        return Ok(vec![runtime.clone()]);
    };
    if routing.routing_mode != LlmRoutingMode::FailoverQueue || routing.queue_targets.is_empty() {
        return Ok(vec![runtime.clone()]);
    }

    let rotate_by = if routing.distribution_rule
        == crate::compiled_plan::LlmDistributionRule::RoundRobin
        && routing.queue_targets.len() > 1
    {
        let distribution_key = routing
            .distribution_key
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("round_robin llm routing is missing distribution_key"))?;
        let counter = runtime_context
            .next_llm_routing_counter(distribution_key, Some(LLM_ROUTING_COUNTER_TTL))
            .await?;
        (counter - 1).rem_euclid(routing.queue_targets.len() as i64) as usize
    } else {
        0
    };

    Ok(routing
        .queue_targets
        .iter()
        .cycle()
        .skip(rotate_by)
        .take(routing.queue_targets.len())
        .map(|target| {
            let mut attempt = runtime.clone();
            attempt.provider_instance_id = target.provider_instance_id.clone();
            attempt.provider_code = target.provider_code.clone();
            attempt.protocol = target.protocol.clone();
            attempt.model = target.upstream_model_id.clone();
            attempt
        })
        .collect())
}

pub(super) struct AttemptMetricInput<'a> {
    pub(super) attempt_index: usize,
    pub(super) runtime: &'a CompiledLlmRuntime,
    pub(super) status: &'a str,
    pub(super) failed_after_first_token: bool,
    pub(super) error_payload: Option<&'a Value>,
    pub(super) usage: &'a ProviderUsage,
    pub(super) event_count: usize,
    pub(super) first_token_at: Option<OffsetDateTime>,
    pub(super) time_to_first_token_ms: Option<u64>,
}

pub(super) fn build_attempt_metric(input: AttemptMetricInput<'_>) -> Value {
    json!({
        "attempt_index": input.attempt_index,
        "provider_instance_id": input.runtime.provider_instance_id,
        "provider_code": input.runtime.provider_code,
        "protocol": input.runtime.protocol,
        "upstream_model_id": input.runtime.model,
        "model": input.runtime.model,
        "status": input.status,
        "failed_after_first_token": input.failed_after_first_token,
        "event_count": input.event_count,
        "first_token_at": offset_datetime_json(input.first_token_at),
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
