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

pub(super) fn canonical_assistant_content_blocks(
    events: &[ProviderStreamEvent],
    final_content: Option<&str>,
) -> Vec<Value> {
    let mut blocks = Vec::new();
    let mut current_kind: Option<&'static str> = None;
    let mut current_text = String::new();

    let flush = |blocks: &mut Vec<Value>, kind: &mut Option<&'static str>, text: &mut String| {
        if let Some(kind) = kind.take() {
            if !text.is_empty() {
                blocks.push(json!({ "type": kind, "text": std::mem::take(text) }));
            }
        }
    };

    for event in events {
        let (kind, delta) = match event {
            ProviderStreamEvent::ReasoningDelta { delta } => ("reasoning", delta.as_str()),
            ProviderStreamEvent::TextDelta { delta } => ("text", delta.as_str()),
            _ => continue,
        };
        if delta.is_empty() {
            continue;
        }
        if current_kind != Some(kind) {
            flush(&mut blocks, &mut current_kind, &mut current_text);
            current_kind = Some(kind);
        }
        current_text.push_str(delta);
    }
    flush(&mut blocks, &mut current_kind, &mut current_text);

    if !blocks
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("text"))
    {
        if let Some(content) = final_content.filter(|content| !content.is_empty()) {
            blocks.push(json!({ "type": "text", "text": content }));
        }
    }

    blocks
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
    final_content.is_some_and(|content| !strip_llm_think_tags(content).trim().is_empty())
        || !result.tool_calls.is_empty()
        || !result.mcp_calls.is_empty()
        || (native_responses_passthrough
            && result
                .response_id
                .as_deref()
                .is_some_and(|response_id| !response_id.trim().is_empty()))
}

/// Billable-output criteria shared with the control-plane billing guard: an
/// invocation output is billable exactly when the executor would accept it as a
/// valid provider result.
pub fn billable_provider_output(
    events: &[ProviderStreamEvent],
    result: &ProviderInvocationResult,
    native_responses_passthrough: bool,
) -> bool {
    // A stream that produced content and then terminated with an upstream or
    // protocol failure belongs to the executor's failure path. Billing must
    // not turn that mixed outcome into provider_usage_unavailable merely
    // because the partial content looks valid.
    if events.iter().any(|event| {
        matches!(
            event,
            ProviderStreamEvent::Error { .. } | ProviderStreamEvent::OutputProtocolFailure { .. }
        )
    }) || matches!(result.finish_reason, Some(ProviderFinishReason::Error))
    {
        return false;
    }
    let final_content = resolve_final_llm_content(
        result.final_content.clone(),
        collect_dify_style_deltas(events),
    );
    has_valid_provider_output(
        final_content.as_deref(),
        result,
        native_responses_passthrough,
    )
}

pub(super) fn reasoning_only_provider_output_error(
    final_content: Option<&str>,
    result: &ProviderInvocationResult,
    native_responses_passthrough: bool,
) -> Option<ProviderRuntimeError> {
    let is_reasoning_only = final_content.is_some_and(|content| {
        !content.trim().is_empty()
            && strip_llm_think_tags(content).trim().is_empty()
            && result.tool_calls.is_empty()
            && result.mcp_calls.is_empty()
            && !(native_responses_passthrough
                && result
                    .response_id
                    .as_deref()
                    .is_some_and(|response_id| !response_id.trim().is_empty()))
    });
    is_reasoning_only.then(|| {
        ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderInvalidResponse,
            "provider returned reasoning without visible content, tool calls, or MCP calls",
        )
    })
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

pub(super) fn invalid_finish_reason_error(
    finish_reason: Option<&ProviderFinishReason>,
    result: &ProviderInvocationResult,
) -> Option<ProviderRuntimeError> {
    let message = match finish_reason {
        Some(ProviderFinishReason::Unknown) => "provider returned an unknown finish_reason",
        None => "provider returned no finish_reason",
        _ => return None,
    };
    let mut error =
        ProviderRuntimeError::new(ProviderRuntimeErrorKind::ProviderInvalidResponse, message);
    if let Some(stream_termination) =
        provider_stream_termination_diagnostic(&result.provider_metadata)
    {
        error = error.with_provider_details(json!({
            "stream_termination": stream_termination,
        }));
    }
    Some(error)
}

fn provider_stream_termination_diagnostic(provider_metadata: &Value) -> Option<Value> {
    let stream_termination = provider_metadata.get("stream_termination")?.as_object()?;
    let raw_finish_reason = match stream_termination.get("raw_finish_reason") {
        Some(Value::String(reason)) if reason.len() <= 256 => Value::String(reason.clone()),
        Some(Value::Null) => Value::Null,
        _ => return None,
    };
    let raw_finish_reason_status = stream_termination
        .get("raw_finish_reason_status")
        .and_then(Value::as_str)
        .filter(|status| matches!(*status, "recognized" | "unrecognized" | "missing"))?;
    let transport_termination = stream_termination
        .get("transport_termination")
        .and_then(Value::as_str)
        .filter(|termination| matches!(*termination, "done" | "eof" | "error"))?;
    Some(json!({
        "raw_finish_reason": raw_finish_reason,
        "raw_finish_reason_status": raw_finish_reason_status,
        "transport_termination": transport_termination,
    }))
}

pub(super) fn first_provider_error(
    events: &[ProviderStreamEvent],
) -> Option<&ProviderRuntimeError> {
    events.iter().find_map(|event| match event {
        ProviderStreamEvent::Error { error } => Some(error),
        _ => None,
    })
}

pub(super) fn first_provider_output_protocol_failure(
    events: &[ProviderStreamEvent],
) -> Option<&ProviderOutputProtocolFailure> {
    events.iter().find_map(|event| match event {
        ProviderStreamEvent::OutputProtocolFailure { failure } => Some(failure),
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
            ProviderStreamEvent::OutputProtocolFailure { .. } => return saw_content_delta,
            ProviderStreamEvent::Finish {
                reason: ProviderFinishReason::Error,
            } => return saw_content_delta,
            _ => {}
        }
    }

    saw_content_delta && matches!(finish_reason, Some(ProviderFinishReason::Error))
}

pub(crate) fn build_provider_error_payload(
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
    if let Some(stream_termination) = error
        .provider_details
        .as_ref()
        .and_then(provider_stream_termination_diagnostic)
    {
        payload["stream_termination"] = stream_termination;
    }
    if error.kind == ProviderRuntimeErrorKind::SemanticCapabilityUnsupported {
        if let Some(details) = error.provider_details.as_ref().and_then(Value::as_object) {
            if let Some(route_id) = details
                .get("route_id")
                .and_then(Value::as_str)
                .filter(|value| value.len() <= 128)
            {
                payload["route_id"] = Value::String(route_id.to_string());
            }
            payload["missing_capabilities"] = Value::Array(
                details
                    .get("missing_capabilities")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|value| value.len() <= 128)
                    .take(32)
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            );
            if let Some(projection) = details
                .get("projection")
                .and_then(allowlisted_projection_diagnostic)
            {
                payload["projection"] = projection;
            }
        }
    }
    payload
}

fn allowlisted_projection_diagnostic(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let causes = object
        .get("causes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(8)
        .filter_map(allowlisted_projection_cause)
        .collect::<Vec<_>>();
    Some(json!({
        "cause_count": object.get("cause_count").and_then(Value::as_u64),
        "causes_capped": object.get("causes_capped").and_then(Value::as_bool).unwrap_or(false),
        "causes": causes,
    }))
}

fn allowlisted_projection_cause(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let cause = object
        .get("cause")
        .and_then(Value::as_str)
        .filter(|cause| {
            matches!(
                *cause,
                "unsupported" | "invalid_canonical_contract" | "missing_capabilities"
            )
        })?;
    let mut output = Map::from_iter([("cause".to_string(), Value::String(cause.to_string()))]);
    if let Some(error_code) = object
        .get("error_code")
        .and_then(Value::as_str)
        .filter(|code| *code == "reasoning_only_message_unsupported")
    {
        output.insert(
            "error_code".to_string(),
            Value::String(error_code.to_string()),
        );
    }
    if let Some(block) = object.get("block").and_then(allowlisted_block_locator) {
        output.insert("block".to_string(), block);
    }
    if let Some(receipt) = object.get("receipt").and_then(allowlisted_receipt) {
        output.insert("receipt".to_string(), receipt);
    }
    if let Some(missing) = object.get("missing_capabilities").and_then(Value::as_array) {
        output.insert(
            "missing_capabilities".to_string(),
            Value::Array(
                missing
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|value| value.len() <= 128)
                    .take(32)
                    .map(|value| Value::String(value.to_string()))
                    .collect(),
            ),
        );
    }
    Some(Value::Object(output))
}

fn allowlisted_receipt(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let fidelity = object
        .get("fidelity")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "exact" | "lossy" | "unsupported"));
    let loss_codes = object
        .get("loss_codes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|value| {
            matches!(
                *value,
                "reasoning_history_omitted"
                    | "signed_reasoning_omitted"
                    | "redacted_reasoning_omitted"
            )
        })
        .take(8)
        .map(|value| Value::String(value.to_string()))
        .collect::<Vec<_>>();
    let error_code = object
        .get("error_code")
        .and_then(Value::as_str)
        .filter(|value| *value == "reasoning_only_message_unsupported");
    let provenance = object.get("provenance").and_then(Value::as_object).map(|provenance| {
        json!({
            "source": provenance.get("source").and_then(Value::as_str)
                .filter(|value| *value == "canonical_invocation"),
            "preserved_count": provenance.get("preserved_count").and_then(Value::as_u64),
            "omitted_count": provenance.get("omitted_count").and_then(Value::as_u64),
            "locators_capped": provenance.get("locators_capped").and_then(Value::as_bool).unwrap_or(false),
            "preserved_blocks": allowlisted_locator_array(provenance.get("preserved_blocks")),
            "omitted_blocks": allowlisted_locator_array(provenance.get("omitted_blocks")),
        })
    });
    Some(json!({
        "fidelity": fidelity,
        "loss_codes": loss_codes,
        "error_code": error_code,
        "provenance": provenance,
    }))
}

fn allowlisted_locator_array(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(16)
        .filter_map(allowlisted_block_locator)
        .collect()
}

fn allowlisted_block_locator(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let block_kind = object
        .get("block_kind")
        .and_then(Value::as_str)
        .filter(|kind| {
            matches!(
                *kind,
                "text"
                    | "image"
                    | "image_url"
                    | "document"
                    | "tool_use"
                    | "tool_result"
                    | "reasoning"
                    | "redacted_reasoning"
            )
        })?;
    Some(json!({
        "message_index": object.get("message_index").and_then(Value::as_u64),
        "block_index": object.get("block_index").and_then(Value::as_u64),
        "block_kind": block_kind,
    }))
}

pub(super) fn build_output_protocol_failure_payload(
    runtime: &CompiledLlmRuntime,
    failure: &ProviderOutputProtocolFailure,
) -> Value {
    json!({
        "provider_instance_id": runtime.provider_instance_id,
        "provider_code": runtime.provider_code,
        "protocol": failure.protocol,
        "error_code": "provider_output_protocol_failure",
        "protocol_error_code": failure.error_code,
        "message": failure.message,
        "provider_details": failure.provider_details,
    })
}

pub(super) fn provider_error_allows_retry(error: &ProviderRuntimeError) -> bool {
    match error.kind {
        ProviderRuntimeErrorKind::ProviderAffinityMismatch
        | ProviderRuntimeErrorKind::ProviderTransportUnavailable
        | ProviderRuntimeErrorKind::SemanticCapabilityUnsupported => false,
        ProviderRuntimeErrorKind::ProviderUpstreamError => {
            provider_status_code(error.provider_details.as_ref()).is_none_or(|status| status >= 500)
        }
        _ => true,
    }
}

fn provider_status_code(details: Option<&Value>) -> Option<u16> {
    details
        .and_then(|details| details.get("status_code").or_else(|| details.get("status")))
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
            ProviderStreamEvent::NativeEvent { .. }
            | ProviderStreamEvent::ReasoningSignatureDelta { .. }
            | ProviderStreamEvent::OutputItem { .. } => None,
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
    if let Some(contract_error @ PluginFrameworkError::InvalidProviderContract { .. }) =
        error.downcast_ref::<PluginFrameworkError>()
    {
        return ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderInvalidResponse,
            contract_error.to_string(),
        );
    }
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
