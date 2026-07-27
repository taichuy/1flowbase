use super::*;

pub(super) fn parse_structured_llm_output(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|error| anyhow!("invalid structured LLM output: {error}"))
}

pub(super) fn strip_llm_think_tags(text: &str) -> String {
    let mut answer = String::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("<think>") {
        answer.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<think>".len()..];
        if let Some(end) = after_start.find("</think>") {
            remaining = &after_start[end + "</think>".len()..];
        } else {
            remaining = "";
            break;
        }
    }
    answer.push_str(remaining);

    answer
}

pub(super) fn resolve_final_llm_content(
    result_content: Option<String>,
    stream_content: Option<String>,
) -> Option<String> {
    match (result_content, stream_content) {
        (Some(_), Some(stream)) if stream.contains("<think>") => Some(stream),
        (Some(result), _) => Some(result),
        (None, stream) => stream,
    }
}

pub(super) fn collect_dify_style_deltas(events: &[ProviderStreamEvent]) -> Option<String> {
    let mut content = String::new();
    let mut reasoning_delta_open = false;

    for event in events {
        match event {
            ProviderStreamEvent::ReasoningDelta { delta } => {
                if delta.is_empty() {
                    continue;
                }
                if !reasoning_delta_open {
                    content.push_str("<think>");
                    reasoning_delta_open = true;
                }
                content.push_str(delta);
            }
            ProviderStreamEvent::TextDelta { delta } => {
                if reasoning_delta_open {
                    content.push_str("</think>");
                    reasoning_delta_open = false;
                }
                content.push_str(delta);
            }
            _ => {}
        }
    }

    if reasoning_delta_open {
        content.push_str("</think>");
    }
    (!content.is_empty()).then_some(content)
}

pub(super) fn collect_usage(
    events: &[ProviderStreamEvent],
    result_usage: &ProviderUsage,
) -> ProviderUsage {
    let mut usage = result_usage.clone();
    for event in events {
        match event {
            ProviderStreamEvent::UsageSnapshot { usage: snapshot } => {
                usage = snapshot.clone();
            }
            ProviderStreamEvent::UsageDelta { usage: delta } => {
                apply_usage_delta(&mut usage, delta)
            }
            _ => {}
        }
    }
    usage
}

pub(super) fn apply_usage_delta(target: &mut ProviderUsage, delta: &ProviderUsage) {
    add_usage_value(&mut target.input_tokens, delta.input_tokens);
    add_usage_value(
        &mut target.input_cache_hit_tokens,
        delta.input_cache_hit_tokens,
    );
    add_usage_value(
        &mut target.input_cache_miss_tokens,
        delta.input_cache_miss_tokens,
    );
    add_usage_value(&mut target.output_tokens, delta.output_tokens);
    add_usage_value(&mut target.reasoning_tokens, delta.reasoning_tokens);
    add_usage_value(&mut target.cache_read_tokens, delta.cache_read_tokens);
    add_usage_value(&mut target.cache_write_tokens, delta.cache_write_tokens);
    add_usage_value(&mut target.total_tokens, delta.total_tokens);
}

pub(super) fn add_usage_value(target: &mut Option<u64>, delta: Option<u64>) {
    if let Some(delta) = delta {
        *target = Some(target.unwrap_or_default() + delta);
    }
}

pub(super) fn finish_reason_from_events(
    events: &[ProviderStreamEvent],
) -> Option<ProviderFinishReason> {
    events.iter().rev().find_map(|event| match event {
        ProviderStreamEvent::Finish { reason } => Some(reason.clone()),
        _ => None,
    })
}

pub(super) fn has_valid_provider_output(
    final_content: Option<&str>,
    result: &ProviderInvocationResult,
    native_responses_passthrough: bool,
) -> bool {
    final_content.is_some_and(|content| !content.trim().is_empty())
        || !result.tool_calls.is_empty()
        || !result.mcp_calls.is_empty()
        || (native_responses_passthrough
            && result
                .response_id
                .as_deref()
                .is_some_and(|response_id| !response_id.trim().is_empty()))
}

pub(super) fn build_empty_provider_response_error_payload(runtime: &CompiledLlmRuntime) -> Value {
    json!({
        "provider_instance_id": runtime.provider_instance_id,
        "provider_code": runtime.provider_code,
        "protocol": runtime.protocol,
        "error_code": "empty_response",
        "message": "provider returned no valid text, tool call, or MCP call",
    })
}

pub(super) fn invalid_tool_call_finish_error(
    finish_reason: Option<&ProviderFinishReason>,
    result: &ProviderInvocationResult,
) -> Option<ProviderRuntimeError> {
    (matches!(finish_reason, Some(ProviderFinishReason::ToolCall)) && result.tool_calls.is_empty())
        .then(|| {
            ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ProviderInvalidResponse,
                "provider returned finish_reason=tool_call without tool_calls",
            )
        })
}

pub(super) fn first_provider_error(
    events: &[ProviderStreamEvent],
) -> Option<&ProviderRuntimeError> {
    events.iter().find_map(|event| match event {
        ProviderStreamEvent::Error { error } => Some(error),
        _ => None,
    })
}

pub(super) fn content_delta_seen_before_terminal_failure(
    events: &[ProviderStreamEvent],
    finish_reason: Option<&ProviderFinishReason>,
) -> bool {
    let mut saw_content_delta = false;
    for event in events {
        match event {
            ProviderStreamEvent::TextDelta { .. } | ProviderStreamEvent::ReasoningDelta { .. } => {
                saw_content_delta = true
            }
            ProviderStreamEvent::Error { .. } => return saw_content_delta,
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            } => return saw_content_delta,
            _ => {}
        }
    }

    saw_content_delta && matches!(finish_reason, Some(ProviderFinishReason::Error))
}

pub(super) fn build_provider_error_payload(
    runtime: &CompiledLlmRuntime,
    error: &ProviderRuntimeError,
) -> Value {
    let mut payload = json!({
        "provider_instance_id": runtime.provider_instance_id,
        "provider_code": runtime.provider_code,
        "protocol": runtime.protocol,
        "error_code": serde_json::to_value(error.kind).unwrap_or(Value::Null),
        "message": error.message.clone(),
    });
    if let Some(status_code) = provider_status_code(error.provider_details.as_ref()) {
        payload["status_code"] = json!(status_code);
    }
    payload
}

pub(super) fn provider_error_allows_retry(error: &ProviderRuntimeError) -> bool {
    !matches!(
        error.kind,
        ProviderRuntimeErrorKind::ProviderAffinityMismatch
            | ProviderRuntimeErrorKind::ProviderTransportUnavailable
    )
}

fn provider_status_code(details: Option<&Value>) -> Option<u16> {
    details
        .and_then(|details| details.get("status"))
        .and_then(Value::as_u64)
        .and_then(|status| u16::try_from(status).ok())
        .filter(|status| (100..=599).contains(status))
}

pub(super) fn durable_provider_events(
    events: Vec<ProviderStreamEvent>,
) -> Vec<ProviderStreamEvent> {
    events
        .into_iter()
        .filter_map(|event| match event {
            ProviderStreamEvent::NativeEvent { .. } | ProviderStreamEvent::OutputItem { .. } => {
                None
            }
            ProviderStreamEvent::Error { error } => Some(ProviderStreamEvent::Error {
                error: ProviderRuntimeError::new(error.kind, error.message.clone()),
            }),
            other => Some(other),
        })
        .collect()
}

pub(super) fn recoverable_provider_error_message(error: &ProviderRuntimeError) -> String {
    error.message.clone()
}

pub(super) fn provider_runtime_error_from_anyhow(error: &anyhow::Error) -> ProviderRuntimeError {
    if let Some(PluginFrameworkError::RuntimeContract { error }) =
        error.downcast_ref::<PluginFrameworkError>()
    {
        return normalize_runtime_contract_error(error);
    }
    if let Some(ProviderCompactError::Runtime { error }) =
        error.downcast_ref::<ProviderCompactError>()
    {
        return normalize_runtime_contract_error(error);
    }
    if let Some(ProviderCountTokensError::Runtime { error }) =
        error.downcast_ref::<ProviderCountTokensError>()
    {
        return normalize_runtime_contract_error(error);
    }

    ProviderRuntimeError::normalize("invoke", error.to_string(), None)
}

pub(super) fn normalize_runtime_contract_error(
    error: &ProviderRuntimeError,
) -> ProviderRuntimeError {
    if error.kind != ProviderRuntimeErrorKind::ProviderInvalidResponse {
        return error.clone();
    }

    let normalized = ProviderRuntimeError::normalize(
        "invoke",
        &error.message,
        error.provider_summary.as_deref(),
    );
    let normalized = if let Some(provider_details) = &error.provider_details {
        normalized.with_provider_details(provider_details.clone())
    } else {
        normalized
    };
    if normalized.kind == ProviderRuntimeErrorKind::ProviderInvalidResponse {
        error.clone()
    } else {
        normalized
    }
}
