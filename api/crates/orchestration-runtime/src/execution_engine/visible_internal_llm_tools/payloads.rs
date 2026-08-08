use super::types::*;
use super::*;

pub(super) fn visible_internal_llm_tool_node_error(
    tool_call: &Value,
    tool: &VisibleInternalLlmTool,
    node: &CompiledNode,
    message: &str,
    runtime_message: Option<String>,
    details: Option<Value>,
) -> Value {
    json!({
        "error_code": "visible_internal_llm_tool_failed",
        "message": message,
        "runtime_message": runtime_message,
        "tool_call_id": tool_call_id(tool_call),
        "tool_name": tool.name,
        "target_node_id": tool.target_node_id,
        "node_id": node.node_id,
        "details": details,
    })
}

pub(super) fn visible_internal_llm_tool_output_text(output_payload: &Value) -> String {
    output_payload
        .get("text")
        .or_else(|| output_payload.get("answer"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            output_payload
                .as_object()
                .and_then(|object| object.values().find_map(Value::as_str))
                .map(str::to_string)
        })
        .unwrap_or_default()
}

pub(crate) fn output_tool_calls(output_payload: &Value) -> Option<Vec<Value>> {
    output_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .filter(|calls| !calls.is_empty())
        .cloned()
}

pub(super) fn visible_internal_tool_calls<'a>(
    tool_calls: &[Value],
    tools: &'a [VisibleInternalLlmTool],
) -> Vec<(Value, &'a VisibleInternalLlmTool)> {
    tool_calls
        .iter()
        .filter_map(|tool_call| {
            let name = tool_call.get("name").and_then(Value::as_str)?;
            let tool = tools.iter().find(|tool| tool.name == name)?;
            Some((tool_call.clone(), tool))
        })
        .collect()
}

pub(super) fn visible_internal_media_tool_calls_are_repeated_after_route(
    internal_tool_calls: &[(Value, &VisibleInternalLlmTool)],
    route_events: &[Value],
) -> bool {
    !route_events.is_empty()
        && !internal_tool_calls.is_empty()
        && internal_tool_calls
            .iter()
            .all(|(tool_call, tool)| visible_internal_tool_call_has_media(tool_call, tool))
}

pub(super) fn remove_visible_internal_tool_calls(
    output_payload: &mut Value,
    internal_tool_calls: &[(Value, &VisibleInternalLlmTool)],
) {
    let internal_call_ids = internal_tool_calls
        .iter()
        .map(|(tool_call, _)| tool_call_id(tool_call))
        .collect::<BTreeSet<_>>();
    let Some(tool_calls) = output_payload
        .get_mut("tool_calls")
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    tool_calls.retain(|tool_call| !internal_call_ids.contains(&tool_call_id(tool_call)));
    if tool_calls.is_empty() {
        if let Some(payload) = output_payload.as_object_mut() {
            payload.remove("tool_calls");
        }
    }
}

fn visible_internal_tool_call_has_media(tool_call: &Value, tool: &VisibleInternalLlmTool) -> bool {
    tool.input_schema
        .as_ref()
        .and_then(|schema| schema.get("properties"))
        .and_then(|properties| properties.get("media"))
        .is_some()
        && tool_call
            .get("arguments")
            .and_then(|arguments| arguments.get("media"))
            .and_then(Value::as_array)
            .is_some_and(|media| !media.is_empty())
}

pub(super) fn append_output_text(target: &mut String, output_payload: &Value) {
    if let Some(text) = output_payload.get("text").and_then(Value::as_str) {
        target.push_str(text);
    }
}

pub(super) fn attach_visible_internal_llm_tool_events(
    execution: &mut LlmNodeExecution,
    route_events: &[Value],
) {
    if route_events.is_empty() {
        return;
    }
    if !execution.debug_payload.is_object() {
        execution.debug_payload = json!({});
    }
    if let Some(debug) = execution.debug_payload.as_object_mut() {
        debug.insert(
            "visible_internal_llm_tool_events".to_string(),
            Value::Array(route_events.to_vec()),
        );
    }
}

pub(super) fn execution_with_visible_transcript(
    mut execution: LlmNodeExecution,
    visible_transcript: String,
    provider_events: Vec<ProviderStreamEvent>,
    route_events: Vec<Value>,
) -> LlmNodeExecution {
    if !visible_transcript.is_empty() {
        if let Some(output) = execution.output_payload.as_object_mut() {
            output.insert("text".to_string(), Value::String(visible_transcript));
        }
    }
    if !provider_events.is_empty() {
        execution.provider_events = provider_events;
    }
    attach_visible_internal_llm_tool_events(&mut execution, &route_events);
    execution.pending_callback = None;
    execution
}

enum VisibleInternalLlmToolRecoverableError {
    ExpectedGuidance(String),
    ProviderFailure(String),
}

pub(super) fn visible_internal_llm_tool_recoverable_result(
    tool_call: &Value,
    tool_name: &str,
    error_payload: &Value,
) -> Option<Value> {
    match visible_internal_llm_tool_recoverable_error(error_payload)? {
        VisibleInternalLlmToolRecoverableError::ExpectedGuidance(content) => Some(
            visible_internal_llm_tool_result(tool_call, tool_name, content),
        ),
        VisibleInternalLlmToolRecoverableError::ProviderFailure(content) => Some(
            visible_internal_llm_tool_error_result(tool_call, tool_name, content),
        ),
    }
}

fn visible_internal_llm_tool_recoverable_error(
    error_payload: &Value,
) -> Option<VisibleInternalLlmToolRecoverableError> {
    visible_internal_llm_tool_expected_guidance(error_payload)
        .map(VisibleInternalLlmToolRecoverableError::ExpectedGuidance)
        .or_else(|| {
            visible_internal_llm_tool_provider_failure_content(error_payload)
                .map(VisibleInternalLlmToolRecoverableError::ProviderFailure)
        })
}

fn visible_internal_llm_tool_expected_guidance(error_payload: &Value) -> Option<String> {
    if visible_internal_llm_tool_error_code(error_payload)
        == Some("visible_internal_llm_tool_media_unavailable")
    {
        return Some(
            visible_internal_llm_tool_error_detail(error_payload, "hint")
                .unwrap_or("If this path is local to an external client, read the file with a client file tool first and call the routed LLM tool again after the image content block is present in history.")
                .to_string(),
        );
    }

    if visible_internal_llm_tool_error_code(error_payload)
        == Some("visible_internal_llm_tool_mixed_round_callback_unavailable")
    {
        return Some(
            error_payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Call this routed LLM tool again in its own round after external tool callbacks finish.")
                .to_string(),
        );
    }

    if visible_internal_llm_tool_error_code(error_payload)
        == Some("visible_internal_llm_tool_external_callback_forbidden")
    {
        return Some(
            visible_internal_llm_tool_error_detail(error_payload, "message")
                .or_else(|| error_payload.get("message").and_then(Value::as_str))
                .unwrap_or("visible internal LLM tool external callback is forbidden by policy")
                .to_string(),
        );
    }

    if visible_internal_llm_tool_error_mentions(error_payload, "model_multimodal_unsupported") {
        return Some("model_multimodal_unsupported: use a model that supports the requested media input, or read the file with a client file tool and retry after the media content is available.".to_string());
    }

    None
}

fn visible_internal_llm_tool_provider_failure_content(error_payload: &Value) -> Option<String> {
    let details = visible_internal_llm_tool_provider_details(error_payload)?;
    for key in [
        "provider_summary",
        "response_body",
        "body",
        "raw_body",
        "error",
        "content",
        "message",
        "error_code",
    ] {
        if let Some(content) = visible_internal_llm_tool_provider_field_content(details.get(key)) {
            return Some(content);
        }
    }

    None
}

fn visible_internal_llm_tool_provider_details(
    error_payload: &Value,
) -> Option<&Map<String, Value>> {
    let details = error_payload.get("details").and_then(Value::as_object)?;
    let has_provider_route = details.get("provider_instance_id").is_some()
        || details.get("provider_code").is_some()
        || details.get("protocol").is_some();
    let has_error_details = details
        .get("error_code")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
        || details
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());

    (has_provider_route && has_error_details).then_some(details)
}

fn visible_internal_llm_tool_provider_field_content(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(content) => (!content.trim().is_empty()).then(|| content.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn visible_internal_llm_tool_error_code(error_payload: &Value) -> Option<&str> {
    visible_internal_llm_tool_error_detail(error_payload, "error_code")
        .or_else(|| error_payload.get("error_code").and_then(Value::as_str))
}

fn visible_internal_llm_tool_error_detail<'a>(
    error_payload: &'a Value,
    key: &str,
) -> Option<&'a str> {
    error_payload
        .get("details")
        .and_then(|details| details.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn visible_internal_llm_tool_error_mentions(error_payload: &Value, needle: &str) -> bool {
    [
        error_payload.get("message").and_then(Value::as_str),
        error_payload.get("runtime_message").and_then(Value::as_str),
        visible_internal_llm_tool_error_detail(error_payload, "message"),
        visible_internal_llm_tool_error_detail(error_payload, "error_code"),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.contains(needle))
}

pub(super) fn visible_internal_llm_tool_route_event(
    event_type: &str,
    main_node_id: &str,
    tool_call: &Value,
    tool: &VisibleInternalLlmTool,
    details: Value,
) -> Value {
    let mut payload = Map::new();
    payload.insert(
        "event_type".to_string(),
        Value::String(event_type.to_string()),
    );
    payload.insert(
        "main_node_id".to_string(),
        Value::String(main_node_id.to_string()),
    );
    payload.insert(
        "target_node_id".to_string(),
        Value::String(tool.target_node_id.clone()),
    );
    payload.insert("tool_name".to_string(), Value::String(tool.name.clone()));
    payload.insert(
        "tool_mode".to_string(),
        Value::String(tool.tool_mode.as_str().to_string()),
    );
    payload.insert(
        "execution_mode".to_string(),
        Value::String(tool.execution_mode.as_str().to_string()),
    );
    payload.insert(
        "tool_call_id".to_string(),
        Value::String(tool_call_id(tool_call)),
    );
    if let Some(arguments) = tool_call.get("arguments") {
        payload.insert("arguments".to_string(), arguments.clone());
    }
    if let Some(details) = details.as_object() {
        for (key, value) in details {
            payload.insert(key.clone(), value.clone());
        }
    }

    Value::Object(payload)
}

pub(super) fn visible_internal_llm_tool_failure(
    node: &CompiledNode,
    provider_events: Vec<ProviderStreamEvent>,
    error_payload: Value,
    route_events: Vec<Value>,
) -> Result<LlmNodeExecution> {
    let runtime = node.llm_runtime.as_ref().ok_or_else(|| {
        anyhow!(
            "compiled llm node is missing runtime metadata: {}",
            node.node_id
        )
    })?;

    let mut execution = build_failed_llm_execution(
        node,
        runtime,
        error_payload,
        LlmFailureProjection::NoNodeOutput,
        None,
        build_llm_metrics_payload(
            runtime,
            ProviderUsage::default(),
            Some(ProviderFinishReason::Error),
            provider_events.len(),
            Vec::new(),
            None,
            None,
        ),
        provider_events,
        LlmDebugInvocation {
            messages: &[],
            context: None,
        },
    )?;
    attach_visible_internal_llm_tool_events(&mut execution, &route_events);
    Ok(execution)
}

pub(crate) fn tool_call_id(tool_call: &Value) -> String {
    tool_call
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("tool_call")
        .to_string()
}

pub(super) fn visible_internal_llm_tool_result(
    tool_call: &Value,
    tool_name: &str,
    content: String,
) -> Value {
    json!({
        "tool_call_id": tool_call_id(tool_call),
        "name": tool_name,
        "content": content
    })
}

pub(super) fn visible_internal_llm_tool_error_result(
    tool_call: &Value,
    tool_name: &str,
    content: String,
) -> Value {
    json!({
        "tool_call_id": tool_call_id(tool_call),
        "name": tool_name,
        "content": content,
        "is_error": true
    })
}
