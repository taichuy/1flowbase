use super::*;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LlmToolPromptTranscript {
    system: Vec<NativePromptBlock>,
    messages: Vec<Value>,
}

pub(super) fn llm_tool_prompt_transcript(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
    invocation: &ProviderInvocationInput,
) -> LlmToolPromptTranscript {
    LlmToolPromptTranscript {
        system: invocation.system.clone(),
        messages: pending_llm_tool_callback_history(node, variable_pool)
            .unwrap_or_else(|| prompt_messages_from_provider_messages(&invocation.messages)),
    }
}

pub(super) fn build_llm_tool_callback_wait(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
    output_payload: &Value,
    transcript: &LlmToolPromptTranscript,
) -> Result<Option<LlmToolCallbackWait>> {
    if !has_pending_tool_calls(output_payload) {
        return Ok(None);
    }

    let history = llm_callback_history_after_assistant_tool_call(transcript, output_payload);
    let serialized_system = serde_json::to_value(&transcript.system)
        .map_err(|error| anyhow!("failed to serialize LLM tool callback system: {error}"))?;
    Ok(Some(LlmToolCallbackWait {
        node_id: node.node_id.clone(),
        node_alias: node.alias.clone(),
        request_payload: build_llm_tool_callback_request_payload(output_payload, &history),
        checkpoint_variable_pool: variable_pool_with_pending_llm_tool_callback(
            node,
            variable_pool,
            output_payload,
            serialized_system,
            history,
        ),
        node_trace: None,
    }))
}

pub(super) fn build_llm_tool_callback_request_payload(
    output_payload: &Value,
    history: &[Value],
) -> Value {
    let mut payload = Map::new();

    for key in [
        "text",
        "tool_calls",
        "finish_reason",
        "usage",
        "provider_route",
        "response_id",
        "provider_continuation",
        "provider_metadata",
    ] {
        if let Some(value) = output_payload.get(key) {
            payload.insert(key.to_string(), value.clone());
        }
    }
    payload.insert(
        "callback_kind".to_string(),
        Value::String(LLM_TOOL_CALLBACK_KIND.to_string()),
    );
    payload.insert("history".to_string(), Value::Array(history.to_vec()));

    Value::Object(payload)
}

pub(super) fn variable_pool_with_pending_llm_tool_callback(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
    output_payload: &Value,
    system: Value,
    history: Vec<Value>,
) -> Map<String, Value> {
    let mut checkpoint_variable_pool = variable_pool.clone();
    let mut callback_state = Map::new();
    callback_state.insert(
        "callback_kind".to_string(),
        Value::String(LLM_TOOL_CALLBACK_KIND.to_string()),
    );
    callback_state.insert(
        "pending_tool_calls".to_string(),
        output_payload
            .get("tool_calls")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    callback_state.insert("system".to_string(), system);
    callback_state.insert("history".to_string(), Value::Array(history));
    if let Some(response_id) = output_payload
        .get("response_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        callback_state.insert(
            "response_id".to_string(),
            Value::String(response_id.to_string()),
        );
    }
    if let Some(provider_route) = output_payload.get("provider_route") {
        callback_state.insert("provider_route".to_string(), provider_route.clone());
    }
    if let Some(provider_continuation) = output_payload.get("provider_continuation") {
        callback_state.insert(
            "provider_continuation".to_string(),
            provider_continuation.clone(),
        );
    }
    if let Some(provider_metadata) = output_payload.get("provider_metadata") {
        callback_state.insert("provider_metadata".to_string(), provider_metadata.clone());
    }
    let mut node_state = Map::new();
    node_state.insert(
        LLM_TOOL_CALLBACK_STATE_KEY.to_string(),
        Value::Object(callback_state),
    );
    checkpoint_variable_pool.insert(node.node_id.clone(), Value::Object(node_state));
    checkpoint_variable_pool
}

pub(super) fn llm_callback_history_after_assistant_tool_call(
    transcript: &LlmToolPromptTranscript,
    output_payload: &Value,
) -> Vec<Value> {
    let mut history = transcript.messages.clone();
    let mut assistant_message = Map::new();
    assistant_message.insert("role".to_string(), Value::String("assistant".to_string()));
    assistant_message.insert(
        "content".to_string(),
        Value::String(
            output_payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
    );
    assistant_message.insert(
        "tool_calls".to_string(),
        output_payload
            .get("tool_calls")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    if let Some(usage) = output_payload.get("usage") {
        assistant_message.insert("usage".to_string(), usage.clone());
    }
    history.push(Value::Object(assistant_message));
    history
}

pub(super) fn apply_mixed_llm_tool_callback_results(
    variable_pool: &mut Map<String, Value>,
    waiting_node_id: &str,
    internal_tool_results: &[Value],
    external_tool_calls: &[Value],
) -> Result<()> {
    let state = variable_pool
        .get_mut(waiting_node_id)
        .and_then(|node_state| node_state.get_mut(LLM_TOOL_CALLBACK_STATE_KEY))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("llm tool callback state not found for {waiting_node_id}"))?;
    let history = state
        .get_mut("history")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("llm tool callback state is missing history"))?;
    for tool_result in internal_tool_results {
        let object = tool_result
            .as_object()
            .ok_or_else(|| anyhow!("internal tool result must be an object"))?;
        let tool_call_id = object
            .get("tool_call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("internal tool result is missing tool_call_id"))?;
        let mut message = Map::new();
        message.insert("role".to_string(), Value::String("tool".to_string()));
        message.insert(
            "tool_call_id".to_string(),
            Value::String(tool_call_id.to_string()),
        );
        let (content, content_blocks) = tool_result_prompt_content(
            object
                .get("content")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        message.insert("content".to_string(), content);
        if let Some(content_blocks) = content_blocks {
            message.insert("content_blocks".to_string(), content_blocks);
        }
        if let Some(is_error) = object.get("is_error").and_then(Value::as_bool) {
            message.insert("is_error".to_string(), Value::Bool(is_error));
        }
        if let Some(name) = object
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            message.insert("name".to_string(), Value::String(name.to_string()));
        }
        history.push(Value::Object(message));
    }
    state.insert(
        "pending_tool_calls".to_string(),
        Value::Array(external_tool_calls.to_vec()),
    );
    Ok(())
}

pub(super) fn discard_llm_tool_callback_calls(
    variable_pool: &mut Map<String, Value>,
    waiting_node_id: &str,
    discarded_tool_call_ids: &BTreeSet<String>,
    retained_tool_calls: &[Value],
) -> Result<()> {
    let state = variable_pool
        .get_mut(waiting_node_id)
        .and_then(|node_state| node_state.get_mut(LLM_TOOL_CALLBACK_STATE_KEY))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("llm tool callback state not found for {waiting_node_id}"))?;
    let assistant = state
        .get_mut("history")
        .and_then(Value::as_array_mut)
        .and_then(|history| history.last_mut())
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("llm tool callback state is missing assistant history"))?;
    let tool_calls = assistant
        .get_mut("tool_calls")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("llm tool callback assistant history is missing tool_calls"))?;
    tool_calls.retain(|tool_call| {
        tool_call
            .get("id")
            .and_then(Value::as_str)
            .is_none_or(|id| !discarded_tool_call_ids.contains(id))
    });
    state.insert(
        "pending_tool_calls".to_string(),
        Value::Array(retained_tool_calls.to_vec()),
    );
    Ok(())
}

pub(super) fn refresh_llm_tool_callback_wait_from_checkpoint(
    wait: &mut LlmToolCallbackWait,
    checkpoint_variable_pool: Map<String, Value>,
) -> Result<()> {
    let state = pending_llm_tool_callback_state(&checkpoint_variable_pool, &wait.node_id)
        .ok_or_else(|| anyhow!("llm tool callback state not found for {}", wait.node_id))?;
    let history = state
        .get("history")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("llm tool callback state is missing history"))?;
    let pending_tool_calls = state
        .get("pending_tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("llm tool callback state is missing pending_tool_calls"))?;
    let request_payload = wait
        .request_payload
        .as_object_mut()
        .ok_or_else(|| anyhow!("llm tool callback request payload must be an object"))?;
    request_payload.insert("history".to_string(), Value::Array(history));
    request_payload.insert("tool_calls".to_string(), Value::Array(pending_tool_calls));
    wait.checkpoint_variable_pool = checkpoint_variable_pool;
    Ok(())
}

pub(super) fn append_llm_tool_result_messages(
    variable_pool: &mut Map<String, Value>,
    waiting_node_id: &str,
    resume_payload: &Value,
) -> Result<()> {
    let state = pending_llm_tool_callback_state(variable_pool, waiting_node_id)
        .ok_or_else(|| anyhow!("llm tool callback state not found for {waiting_node_id}"))?;
    let response_id = state
        .get("response_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned);
    let system = state.get("system").cloned();
    let provider_route = state.get("provider_route").cloned();
    let provider_continuation = state.get("provider_continuation").cloned();
    let provider_metadata = state.get("provider_metadata").cloned();
    let visible_internal_transcript = state.get("visible_internal_llm_tool_transcript").cloned();
    let visible_internal_events = state.get("visible_internal_llm_tool_events").cloned();
    let pending_tool_calls = state
        .get("pending_tool_calls")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("llm tool callback state is missing pending_tool_calls"))?;
    let tool_results = resume_payload
        .get("tool_results")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("llm tool callback resume payload requires tool_results"))?;
    let mut history = state
        .get("history")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow!("llm tool callback state is missing history"))?;
    let mut expected_ids = BTreeSet::new();
    let mut ordered_ids = Vec::new();
    let mut pending_tool_names_by_id = BTreeMap::new();
    let mut delta_messages = Vec::new();

    for tool_call in pending_tool_calls {
        let id = tool_call
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("pending tool call is missing id"))?;
        expected_ids.insert(id.to_string());
        ordered_ids.push(id.to_string());
        if let Some(name) = tool_call
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            pending_tool_names_by_id.insert(id.to_string(), name.to_string());
        }
    }

    let mut results_by_id = BTreeMap::new();
    for tool_result in tool_results {
        let object = tool_result
            .as_object()
            .ok_or_else(|| anyhow!("tool result must be an object"))?;
        let tool_call_id = object
            .get("tool_call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("tool result is missing tool_call_id"))?;
        if !expected_ids.contains(tool_call_id) {
            return Err(anyhow!("unexpected tool result for {tool_call_id}"));
        }
        if results_by_id
            .insert(tool_call_id.to_string(), object.clone())
            .is_some()
        {
            return Err(anyhow!("duplicate tool result for {tool_call_id}"));
        }
    }
    for expected_id in &ordered_ids {
        if !results_by_id.contains_key(expected_id) {
            return Err(anyhow!("missing tool result for {expected_id}"));
        }
    }

    for tool_call_id in ordered_ids {
        let result = results_by_id
            .remove(&tool_call_id)
            .ok_or_else(|| anyhow!("missing tool result for {tool_call_id}"))?;
        let mut message = Map::new();
        message.insert("role".to_string(), Value::String("tool".to_string()));
        message.insert(
            "tool_call_id".to_string(),
            Value::String(tool_call_id.clone()),
        );
        let (content, content_blocks) = tool_result_prompt_content(
            result
                .get("content")
                .cloned()
                .unwrap_or_else(|| Value::String(String::new())),
        );
        message.insert("content".to_string(), content);
        if let Some(content_blocks) = content_blocks {
            message.insert("content_blocks".to_string(), content_blocks);
        }
        if let Some(is_error) = result.get("is_error").and_then(Value::as_bool) {
            message.insert("is_error".to_string(), Value::Bool(is_error));
        }
        let name = result
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| pending_tool_names_by_id.get(&tool_call_id).cloned());
        if let Some(name) = name {
            message.insert("name".to_string(), Value::String(name.to_string()));
        }
        let message = Value::Object(message);
        history.push(message.clone());
        delta_messages.push(message);
    }

    let mut callback_state = Map::new();
    callback_state.insert(
        "callback_kind".to_string(),
        Value::String(LLM_TOOL_CALLBACK_KIND.to_string()),
    );
    if let Some(system) = system {
        callback_state.insert("system".to_string(), system);
    }
    callback_state.insert("history".to_string(), Value::Array(history));
    if let Some(response_id) = response_id {
        callback_state.insert("response_id".to_string(), Value::String(response_id));
    }
    if let Some(provider_route) = provider_route {
        callback_state.insert("provider_route".to_string(), provider_route);
    }
    if let Some(provider_continuation) = provider_continuation {
        callback_state.insert("provider_continuation".to_string(), provider_continuation);
    }
    if let Some(provider_metadata) = provider_metadata {
        callback_state.insert("provider_metadata".to_string(), provider_metadata);
    }
    if !delta_messages.is_empty() {
        callback_state.insert("delta_messages".to_string(), Value::Array(delta_messages));
    }
    if let Some(visible_internal_transcript) = visible_internal_transcript {
        callback_state.insert(
            "visible_internal_llm_tool_transcript".to_string(),
            visible_internal_transcript,
        );
    }
    if let Some(visible_internal_events) = visible_internal_events {
        callback_state.insert(
            "visible_internal_llm_tool_events".to_string(),
            visible_internal_events,
        );
    }
    let mut node_state = Map::new();
    node_state.insert(
        LLM_TOOL_CALLBACK_STATE_KEY.to_string(),
        Value::Object(callback_state),
    );
    variable_pool.insert(waiting_node_id.to_string(), Value::Object(node_state));

    Ok(())
}

pub(super) fn tool_result_prompt_content(value: Value) -> (Value, Option<Value>) {
    match value {
        Value::String(_) => (value, None),
        Value::Array(blocks) => {
            let content_blocks = normalize_tool_result_content_blocks(blocks);
            let text = content_blocks
                .iter()
                .filter_map(|entry| entry.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            (Value::String(text), Some(Value::Array(content_blocks)))
        }
        other => (Value::String(other.to_string()), None),
    }
}

fn normalize_tool_result_content_blocks(blocks: Vec<Value>) -> Vec<Value> {
    blocks
        .into_iter()
        .map(normalize_tool_result_content_block)
        .collect()
}

fn normalize_tool_result_content_block(block: Value) -> Value {
    let Value::Object(mut object) = block else {
        return block;
    };
    if object.get("type").and_then(Value::as_str) == Some("image") {
        normalize_image_block_base64_source(&mut object);
    }
    Value::Object(object)
}

fn normalize_image_block_base64_source(object: &mut Map<String, Value>) {
    let Some(source) = object.get_mut("source").and_then(Value::as_object_mut) else {
        return;
    };
    let source_type = source.get("type").and_then(Value::as_str);
    if !matches!(source_type, None | Some("base64"))
        || source
            .get("data")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .is_none()
    {
        return;
    }
    source
        .entry("type".to_string())
        .or_insert_with(|| Value::String("base64".to_string()));
    source
        .entry("media_type".to_string())
        .or_insert_with(|| Value::String("image/png".to_string()));
}

pub(super) fn pending_llm_tool_callback_state<'a>(
    variable_pool: &'a Map<String, Value>,
    node_id: &str,
) -> Option<&'a Map<String, Value>> {
    variable_pool
        .get(node_id)?
        .get(LLM_TOOL_CALLBACK_STATE_KEY)?
        .as_object()
}

pub(super) fn pending_llm_tool_callback_history(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Option<Vec<Value>> {
    pending_llm_tool_callback_state(variable_pool, &node.node_id)?
        .get("history")?
        .as_array()
        .cloned()
}

pub(super) fn pending_llm_tool_callback_visible_internal_transcript(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Option<String> {
    pending_llm_tool_callback_state(variable_pool, &node.node_id)?
        .get("visible_internal_llm_tool_transcript")?
        .as_str()
        .map(str::to_string)
}

pub(super) fn pending_llm_tool_callback_visible_internal_events(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Vec<Value> {
    pending_llm_tool_callback_state(variable_pool, &node.node_id)
        .and_then(|state| state.get("visible_internal_llm_tool_events"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn set_pending_llm_tool_callback_visible_internal_transcript(
    variable_pool: &mut Map<String, Value>,
    node_id: &str,
    transcript: String,
) -> Result<()> {
    let state = variable_pool
        .get_mut(node_id)
        .and_then(Value::as_object_mut)
        .and_then(|node_state| node_state.get_mut(LLM_TOOL_CALLBACK_STATE_KEY))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("llm tool callback state not found for {node_id}"))?;
    state.insert(
        "visible_internal_llm_tool_transcript".to_string(),
        Value::String(transcript),
    );
    Ok(())
}

pub(super) fn set_pending_llm_tool_callback_visible_internal_events(
    variable_pool: &mut Map<String, Value>,
    node_id: &str,
    events: Vec<Value>,
) -> Result<()> {
    let state = variable_pool
        .get_mut(node_id)
        .and_then(Value::as_object_mut)
        .and_then(|node_state| node_state.get_mut(LLM_TOOL_CALLBACK_STATE_KEY))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("llm tool callback state not found for {node_id}"))?;
    state.insert(
        "visible_internal_llm_tool_events".to_string(),
        Value::Array(events),
    );
    Ok(())
}

pub(super) fn pending_llm_tool_callback_delta_messages(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Option<Vec<Value>> {
    pending_llm_tool_callback_state(variable_pool, &node.node_id)?
        .get("delta_messages")?
        .as_array()
        .cloned()
}

pub(super) fn pending_llm_tool_callback_system(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
) -> Result<Option<Vec<NativePromptBlock>>, Value> {
    if let Some(system) = pending_llm_tool_callback_state(variable_pool, &node.node_id)
        .and_then(|state| state.get("system"))
    {
        return serde_json::from_value(system.clone())
            .map(Some)
            .map_err(|error| {
                json!({
                    "error_code": "llm_callback_context_invalid",
                    "message": "LLM tool callback system context is invalid",
                    "runtime_message": error.to_string(),
                })
            });
    }

    let Some(history) = pending_llm_tool_callback_history(node, variable_pool) else {
        return Ok(None);
    };
    provider_messages_from_prompt_messages(history).map(|context| Some(context.0))
}

pub(super) fn pending_llm_tool_callback_previous_response_id(
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    variable_pool: &Map<String, Value>,
) -> Option<String> {
    let state = pending_llm_tool_callback_state(variable_pool, &node.node_id)?;
    if !pending_llm_tool_callback_route_matches(runtime, state.get("provider_route")?) {
        return None;
    }
    if !pending_llm_tool_callback_uses_responses_websocket_cursor(state) {
        return None;
    }
    state
        .get("response_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn pending_llm_tool_callback_uses_responses_websocket_cursor(
    state: &Map<String, Value>,
) -> bool {
    state
        .get("provider_metadata")
        .and_then(|metadata| metadata.get("transport"))
        .and_then(Value::as_str)
        == Some(RESPONSES_WEBSOCKET_TRANSPORT)
}

pub(super) fn pending_llm_tool_callback_route_matches(
    runtime: &CompiledLlmRuntime,
    provider_route: &Value,
) -> bool {
    let Some(provider_route) = provider_route.as_object() else {
        return false;
    };

    provider_route
        .get("provider_instance_id")
        .and_then(Value::as_str)
        == Some(runtime.provider_instance_id.as_str())
        && provider_route.get("provider_code").and_then(Value::as_str)
            == Some(runtime.provider_code.as_str())
        && provider_route.get("protocol").and_then(Value::as_str) == Some(runtime.protocol.as_str())
        && provider_route.get("model").and_then(Value::as_str) == Some(runtime.model.as_str())
}

pub fn pending_llm_tool_callback_requires_ephemeral_provider_continuation(
    variable_pool: &Map<String, Value>,
    node_id: &str,
) -> bool {
    pending_llm_tool_callback_state(variable_pool, node_id)
        .and_then(|state| state.get("provider_continuation"))
        .and_then(|continuation| continuation.get("storage"))
        .and_then(Value::as_str)
        == Some("ephemeral")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_prompt_content_normalizes_bare_base64_image_source() {
        let (_content, content_blocks) = tool_result_prompt_content(json!([
            {
                "type": "image",
                "source": {
                    "data": "aW1hZ2U="
                }
            }
        ]));

        let blocks = content_blocks.expect("image blocks should be preserved");
        assert_eq!(blocks[0]["type"], json!("image"));
        assert_eq!(blocks[0]["source"]["type"], json!("base64"));
        assert_eq!(blocks[0]["source"]["media_type"], json!("image/png"));
        assert_eq!(blocks[0]["source"]["data"], json!("aW1hZ2U="));
    }
}
