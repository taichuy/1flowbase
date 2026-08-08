use std::collections::HashSet;

use super::*;

pub(super) fn build_response_format(config: &Value) -> Option<Value> {
    let response_format = config.get("response_format")?;

    if response_format
        .get("mode")
        .and_then(Value::as_str)
        .is_some_and(|mode| mode == "text")
    {
        return None;
    }

    Some(response_format.clone())
}

pub(super) const LLM_CONTEXT_SOURCE_KEY: &str = "__context_source";

pub(super) fn llm_context_policy(node: &CompiledNode, runtime: &CompiledLlmRuntime) -> Value {
    runtime
        .routing
        .as_ref()
        .map(|routing| routing.context_policy.clone())
        .filter(|value| value.is_object())
        .or_else(|| node.config.get("context_policy").cloned())
        .filter(|value| value.is_object())
        .unwrap_or_else(|| json!({ "integration_context": "enabled" }))
}

pub(super) fn integration_context_enabled(context_policy: &Value) -> bool {
    context_policy
        .get("integration_context")
        .and_then(Value::as_str)
        != Some("disabled")
}

pub(super) fn binding_prompt_messages<'a>(
    node: &'a CompiledNode,
    rendered_templates: &'a Map<String, Value>,
    resolved_inputs: &'a Map<String, Value>,
    variable_pool: &'a Map<String, Value>,
) -> Vec<Value> {
    if let Some(history) = pending_llm_tool_callback_history(node, variable_pool) {
        return history;
    }

    let mut messages = compatible_history_messages(node, resolved_inputs, variable_pool);
    messages.extend(prompt_messages_from_bindings(
        Some(rendered_templates),
        resolved_inputs,
    ));
    messages
}

pub(super) fn binding_prompt_messages_with_context_sources(
    plan: &CompiledPlan,
    node: &CompiledNode,
    rendered_templates: &Map<String, Value>,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    context_policy: &Value,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<Vec<Value>, Value> {
    if let Some(history) = pending_llm_tool_callback_history(node, variable_pool) {
        return Ok(annotate_prompt_messages(
            history,
            "pending_tool_callback_history",
            format!("{}.{}", node.node_id, LLM_TOOL_CALLBACK_STATE_KEY),
        ));
    }

    let mut messages = Vec::new();
    if integration_context_enabled(context_policy) {
        messages.extend(run_level_system_prompt_messages(
            plan,
            node,
            resolved_inputs,
            variable_pool,
            runtime_context,
        )?);
        messages.extend(selected_context_messages_with_sources(
            node,
            variable_pool,
            context_policy,
        )?);
        if !context_policy_has_selector(context_policy) {
            if runtime_context
                .native_model_prompt_context
                .messages
                .is_empty()
            {
                messages.extend(compatible_history_messages_with_context_sources(
                    plan,
                    node,
                    resolved_inputs,
                    variable_pool,
                ));
            } else {
                messages.extend(annotate_prompt_messages(
                    runtime_context.native_model_prompt_context.messages.clone(),
                    "history",
                    "native_model_prompt_context.messages".to_string(),
                ));
            }
        }
    }
    messages.extend(annotate_prompt_messages(
        prompt_messages_from_bindings(Some(rendered_templates), resolved_inputs),
        "node_prompt",
        "bindings.prompt_messages".to_string(),
    ));
    Ok(messages)
}

pub(super) fn context_policy_has_selector(context_policy: &Value) -> bool {
    context_policy
        .get("context_selector")
        .and_then(Value::as_array)
        .is_some_and(|selector| selector.len() >= 2)
}

pub(super) fn selected_context_messages_with_sources(
    node: &CompiledNode,
    variable_pool: &Map<String, Value>,
    context_policy: &Value,
) -> Result<Vec<Value>, Value> {
    let Some(selector) = context_policy
        .get("context_selector")
        .and_then(Value::as_array)
        .map(|selector| {
            selector
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|selector| selector.len() >= 2)
    else {
        return Ok(Vec::new());
    };

    let Some(value) = read_variable_pool_selector(variable_pool, &selector) else {
        return Err(build_llm_context_selector_error_payload(
            node,
            &selector,
            "selector path not found",
        ));
    };

    if !value_is_llm_context_messages(value) {
        return Err(build_llm_context_selector_error_payload(
            node,
            &selector,
            "selector value must be an array of messages with role and content",
        ));
    }

    Ok(annotate_prompt_messages(
        value.as_array().cloned().unwrap_or_default(),
        "context_selector",
        selector.join("."),
    ))
}

pub(super) fn read_variable_pool_selector<'a>(
    variable_pool: &'a Map<String, Value>,
    selector: &[String],
) -> Option<&'a Value> {
    let (first, rest) = selector.split_first()?;
    let mut current = variable_pool.get(first)?;

    for segment in rest {
        current = current.as_object()?.get(segment)?;
    }

    Some(current)
}

pub(super) fn build_llm_context_selector_error_payload(
    node: &CompiledNode,
    selector: &[String],
    message: &str,
) -> Value {
    json!({
        "error_code": "llm_context_selector_error",
        "message": "LLM context selector validation failed",
        "runtime_message": format!(
            "node {} context_selector {}: {message}",
            node.node_id,
            selector.join(".")
        ),
    })
}

pub(super) fn run_level_system_prompt_messages(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<Vec<Value>, Value> {
    let mut messages = Vec::new();
    if !runtime_context
        .native_model_prompt_context
        .system
        .is_empty()
    {
        let native_system = serde_json::to_value(
            &runtime_context.native_model_prompt_context.system,
        )
        .map_err(|error| {
            json!({
                "error_code": "llm_context_serialization_failed",
                "message": "Native model prompt context could not be serialized",
                "runtime_message": error.to_string(),
            })
        })?;
        if !prompt_binding_selects_any_system(node) {
            messages.push(system_prompt_message_with_source(
                native_system.clone(),
                "run_level_system",
                "native_model_prompt_context.system",
            ));
        }

        if let Some(system) = resolved_inputs
            .get("system")
            .and_then(system_prompt_value)
            .filter(|system| system != &native_system)
        {
            messages.push(system_prompt_message_with_source(
                system,
                "run_level_system",
                "resolved_inputs.system",
            ));
        }
        for node_id in connected_upstream_node_ids(plan, &node.node_id) {
            if prompt_binding_selects_system(node, node_id) {
                continue;
            }
            if let Some(system) = variable_pool
                .get(node_id)
                .and_then(|payload| payload.get("system"))
                .and_then(system_prompt_value)
                .filter(|system| system != &native_system)
            {
                messages.push(system_prompt_message_with_source(
                    system,
                    "run_level_system",
                    format!("{node_id}.system"),
                ));
            }
        }
        return Ok(messages);
    }

    if let Some(system) = resolved_inputs.get("system").and_then(system_prompt_value) {
        messages.push(system_prompt_message_with_source(
            system,
            "run_level_system",
            "resolved_inputs.system",
        ));
    }

    for node_id in connected_upstream_node_ids(plan, &node.node_id) {
        if prompt_binding_selects_system(node, node_id) {
            continue;
        }
        if let Some(system) = variable_pool
            .get(node_id)
            .and_then(|payload| payload.get("system"))
            .and_then(system_prompt_value)
        {
            messages.push(system_prompt_message_with_source(
                system,
                "run_level_system",
                format!("{node_id}.system"),
            ));
        }
    }

    Ok(messages)
}

fn prompt_binding_selects_any_system(node: &CompiledNode) -> bool {
    node.bindings.get("prompt_messages").is_some_and(|binding| {
        binding
            .selector_paths
            .iter()
            .any(|selector| selector.last().is_some_and(|segment| segment == "system"))
    })
}

fn prompt_binding_selects_system(node: &CompiledNode, source_node_id: &str) -> bool {
    node.bindings.get("prompt_messages").is_some_and(|binding| {
        binding.selector_paths.iter().any(|selector| {
            selector.as_slice() == [source_node_id.to_string(), "system".to_string()]
        })
    })
}

fn system_prompt_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(text) => (!text.trim().is_empty()).then(|| value.clone()),
        Value::Array(_) => serde_json::from_value::<Vec<NativePromptBlock>>(value.clone())
            .ok()
            .filter(|blocks| !blocks.is_empty())
            .map(|_| value.clone()),
        _ => None,
    }
}

pub(super) fn system_prompt_message_with_source(
    content: Value,
    source_kind: &str,
    source: impl Into<String>,
) -> Value {
    let mut message = Map::new();
    message.insert("role".to_string(), Value::String("system".to_string()));
    message.insert("content".to_string(), content);
    message.insert(
        LLM_CONTEXT_SOURCE_KEY.to_string(),
        json!({
            "source_kind": source_kind,
            "source": source.into(),
            "target": "effective_system",
        }),
    );
    Value::Object(message)
}

pub(super) fn compatible_history_messages_with_context_sources(
    plan: &CompiledPlan,
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> Vec<Value> {
    let direct_history = resolved_inputs
        .get("history")
        .and_then(Value::as_array)
        .cloned();
    if let Some(history) = direct_history {
        return annotate_prompt_messages(history, "history", "resolved_inputs.history".to_string());
    }

    connected_upstream_node_ids(plan, &node.node_id)
        .into_iter()
        .filter_map(|node_id| {
            variable_pool
                .get(node_id)?
                .get("history")
                .and_then(Value::as_array)
                .cloned()
                .filter(|history| !history.is_empty())
                .map(|history| {
                    annotate_prompt_messages(history, "history", format!("{node_id}.history"))
                })
        })
        .next()
        .unwrap_or_default()
}

fn connected_upstream_node_ids<'a>(plan: &'a CompiledPlan, node_id: &str) -> Vec<&'a str> {
    let mut reachable = BTreeSet::new();
    let mut stack = plan
        .nodes
        .get(node_id)
        .map(|node| node.dependency_node_ids.clone())
        .unwrap_or_default();

    while let Some(current) = stack.pop() {
        if !reachable.insert(current.clone()) {
            continue;
        }
        if let Some(node) = plan.nodes.get(&current) {
            stack.extend(node.dependency_node_ids.iter().cloned());
        }
    }

    plan.topological_order
        .iter()
        .filter(|candidate| reachable.contains(candidate.as_str()))
        .map(String::as_str)
        .collect()
}

pub(super) fn annotate_prompt_messages(
    messages: Vec<Value>,
    source_kind: &str,
    source: String,
) -> Vec<Value> {
    messages
        .into_iter()
        .enumerate()
        .map(|(index, message)| annotate_prompt_message(message, source_kind, &source, index))
        .collect()
}

pub(super) fn annotate_prompt_message(
    message: Value,
    source_kind: &str,
    source: &str,
    index: usize,
) -> Value {
    match message {
        Value::Object(mut object) => {
            object.insert(
                LLM_CONTEXT_SOURCE_KEY.to_string(),
                json!({
                    "source": source,
                    "source_kind": source_kind,
                    "message_index": index,
                    "target": "effective_system",
                }),
            );
            Value::Object(object)
        }
        other => other,
    }
}

pub(super) fn prompt_messages_from_bindings(
    rendered_templates: Option<&Map<String, Value>>,
    resolved_inputs: &Map<String, Value>,
) -> Vec<Value> {
    rendered_templates
        .and_then(|templates| templates.get("prompt_messages"))
        .or_else(|| resolved_inputs.get("prompt_messages"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn provider_messages_from_prompt_messages(
    prompt_messages: Vec<Value>,
) -> Result<(Vec<NativePromptBlock>, Vec<ProviderMessage>), Value> {
    let context = provider_context_from_prompt_messages(prompt_messages)?;

    Ok((context.system, context.messages))
}

pub(super) fn provider_context_from_prompt_messages(
    prompt_messages: Vec<Value>,
) -> Result<ProviderPromptContext, Value> {
    let mut system_parts = Vec::new();
    let mut messages = Vec::new();
    let mut compatibility_promotions = Vec::new();
    let mut system_sources = Vec::new();

    for (index, message) in prompt_messages.iter().enumerate() {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .map(provider_message_role)
            .unwrap_or(ProviderMessageRole::User);

        if role == ProviderMessageRole::System {
            let blocks = system_prompt_blocks_from_value(message.get("content"), index)?;
            if blocks.is_empty() {
                continue;
            }
            let source = system_source_payload(message, index);
            system_parts.push(SystemPromptPart {
                blocks,
                source: source.clone(),
            });
            if source.get("source_kind").and_then(Value::as_str) == Some("history") {
                compatibility_promotions.push(source.clone());
            }
            system_sources.push(source);
        } else {
            let content = message
                .get("content")
                .and_then(value_to_text)
                .unwrap_or_default();
            let carries_tool_payload = message.get("tool_calls").is_some()
                || message.get("tool_call_id").is_some()
                || message.get("is_error").is_some()
                || message.get("content_blocks").is_some();
            if content.trim().is_empty() && !carries_tool_payload {
                continue;
            }
            messages.push(ProviderMessage {
                role,
                content,
                name: message
                    .get("name")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                tool_call_id: message
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                is_error: message.get("is_error").and_then(Value::as_bool),
                tool_calls: message.get("tool_calls").map(provider_tool_calls_payload),
                content_blocks: message.get("content_blocks").cloned(),
            });
        }
    }

    let system = if messages.is_empty() {
        seed_user_turn_from_system_only_node_prompt(
            &mut messages,
            &system_parts,
            &mut compatibility_promotions,
        )
    } else {
        system_prompt_blocks(&system_parts)
    };

    Ok(ProviderPromptContext {
        system,
        messages,
        compatibility_promotions,
        system_sources,
    })
}

fn system_prompt_blocks_from_value(
    value: Option<&Value>,
    message_index: usize,
) -> Result<Vec<NativePromptBlock>, Value> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let blocks = match value {
        Value::String(text) => (!text.trim().is_empty())
            .then(|| NativePromptBlock::text(text.clone()))
            .into_iter()
            .collect(),
        Value::Array(_) => serde_json::from_value::<Vec<NativePromptBlock>>(value.clone())
            .map_err(|_| invalid_system_prompt_blocks_payload(message_index))?,
        _ => return Err(invalid_system_prompt_blocks_payload(message_index)),
    };
    if blocks
        .iter()
        .any(|block| block.text_content().trim().is_empty())
    {
        return Err(invalid_system_prompt_blocks_payload(message_index));
    }
    Ok(blocks)
}

fn invalid_system_prompt_blocks_payload(message_index: usize) -> Value {
    json!({
        "error_code": "system_prompt_blocks_invalid",
        "message": "system prompt content must be text or typed prompt blocks",
        "message_index": message_index,
    })
}

pub(super) fn seed_user_turn_from_system_only_node_prompt(
    messages: &mut Vec<ProviderMessage>,
    system_parts: &[SystemPromptPart],
    compatibility_promotions: &mut Vec<Value>,
) -> Vec<NativePromptBlock> {
    let seeded_content = system_parts
        .iter()
        .filter(|part| system_prompt_part_can_seed_user_turn(&part.source))
        .flat_map(|part| part.blocks.iter().map(NativePromptBlock::text_content))
        .collect::<Vec<_>>()
        .join("\n\n");
    if seeded_content.trim().is_empty() {
        return system_prompt_blocks(system_parts);
    }

    messages.push(ProviderMessage {
        role: ProviderMessageRole::User,
        content: seeded_content,
        name: None,
        tool_call_id: None,
        is_error: None,
        tool_calls: None,
        content_blocks: None,
    });
    compatibility_promotions.push(json!({
        "source_kind": "node_prompt_system_only",
        "source": "bindings.prompt_messages",
        "target": "provider_messages",
    }));

    system_prompt_blocks(system_parts)
}

pub(super) fn system_prompt_part_can_seed_user_turn(source: &Value) -> bool {
    matches!(
        source.get("source_kind").and_then(Value::as_str),
        Some("node_prompt" | "prompt_messages")
    )
}

pub(super) fn system_prompt_blocks(system_parts: &[SystemPromptPart]) -> Vec<NativePromptBlock> {
    system_parts
        .iter()
        .flat_map(|part| part.blocks.iter().cloned())
        .collect()
}

pub(super) fn system_source_payload(message: &Value, fallback_index: usize) -> Value {
    let source = message
        .get(LLM_CONTEXT_SOURCE_KEY)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(Map::new);

    json!({
        "source": source
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("prompt_messages"),
        "source_kind": source
            .get("source_kind")
            .and_then(Value::as_str)
            .unwrap_or("prompt_messages"),
        "message_index": source
            .get("message_index")
            .and_then(Value::as_u64)
            .unwrap_or(fallback_index as u64),
        "target": "effective_system",
    })
}

pub(super) fn provider_tool_calls_payload(tool_calls: &Value) -> Value {
    let Some(tool_calls) = tool_calls.as_array() else {
        return tool_calls.clone();
    };

    Value::Array(
        tool_calls
            .iter()
            .map(|tool_call| {
                let Some(object) = tool_call.as_object() else {
                    return tool_call.clone();
                };
                let mut provider_tool_call = object.clone();
                provider_tool_call.remove("call_usage");
                provider_tool_call.remove("call_input_tokens");
                provider_tool_call.remove("call_cached_input_tokens");
                provider_tool_call.remove("call_output_tokens");
                provider_tool_call.remove("result_input_tokens");
                provider_tool_call.remove("result_context_usage");
                provider_tool_call.remove("result_context_input_tokens");
                provider_tool_call.remove("result_context_cached_input_tokens");
                provider_tool_call.remove("token_delta");
                provider_tool_call.remove("token_count_method");
                Value::Object(provider_tool_call)
            })
            .collect(),
    )
}

pub(super) fn provider_message_role(role: &str) -> ProviderMessageRole {
    match role {
        "system" => ProviderMessageRole::System,
        "assistant" => ProviderMessageRole::Assistant,
        "tool" => ProviderMessageRole::Tool,
        _ => ProviderMessageRole::User,
    }
}

pub(super) fn compatible_history_messages(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> Vec<Value> {
    if let Some(history) = pending_llm_tool_callback_history(node, variable_pool) {
        return history;
    }

    let direct_history = resolved_inputs
        .get("history")
        .and_then(Value::as_array)
        .cloned();
    if let Some(history) = direct_history {
        return history;
    }

    node.dependency_node_ids
        .iter()
        .filter_map(|node_id| variable_pool.get(node_id))
        .find_map(|payload| {
            payload
                .get("history")
                .and_then(Value::as_array)
                .cloned()
                .filter(|history| !history.is_empty())
        })
        .unwrap_or_default()
}

pub(super) fn provider_tools(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
) -> Vec<Value> {
    if claude_code_control_run_blocks_tools(resolved_inputs, variable_pool) {
        return Vec::new();
    }

    let mut tools = external_provider_tools(
        node,
        resolved_inputs,
        rendered_templates,
        variable_pool,
        runtime_context,
    );
    tools.extend(
        runtime_context
            .runtime_internal_tool_registrations(node)
            .into_iter()
            .map(|registration| registration.provider_tool),
    );
    if !media_route_has_returned_to_main(node, resolved_inputs, variable_pool) {
        tools.extend(visible_internal_llm_provider_tools(node));
    }
    tools
}

fn claude_code_control_run_blocks_tools(
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> bool {
    payload_has_claude_code_control(resolved_inputs)
        || variable_pool
            .values()
            .filter_map(Value::as_object)
            .any(payload_has_claude_code_control)
}

fn payload_has_claude_code_control(payload: &Map<String, Value>) -> bool {
    payload
        .get("compatibility")
        .and_then(|compatibility| compatibility.get("claude_code_control"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| matches!(value, "compact_summary" | "session_title"))
}

fn external_provider_tools(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
) -> Vec<Value> {
    if visible_internal_llm_tool_blocks_external_tools(variable_pool) {
        return Vec::new();
    }

    for candidate in [
        rendered_templates.get("tools"),
        resolved_inputs.get("tools"),
        resolved_inputs
            .get("compatibility")
            .and_then(|value| value.get("tools")),
        node.config.get("tools"),
        node.config
            .get("compatibility")
            .and_then(|value| value.get("tools")),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(tools) = candidate.as_array() {
            if !tools.is_empty() {
                return merge_external_provider_tools(
                    provider_tool_payloads(tools),
                    &runtime_context.tools,
                );
            }
        }
    }

    node.dependency_node_ids
        .iter()
        .filter_map(|node_id| variable_pool.get(node_id))
        .find_map(|payload| {
            payload
                .get("compatibility")
                .and_then(|compatibility| compatibility.get("tools"))
                .and_then(Value::as_array)
                .map(|tools| provider_tool_payloads(tools))
                .filter(|tools| !tools.is_empty())
                .or_else(|| {
                    payload
                        .get("tools")
                        .and_then(Value::as_array)
                        .map(|tools| provider_tool_payloads(tools))
                        .filter(|tools| !tools.is_empty())
                })
        })
        .map(|tools| merge_external_provider_tools(tools, &runtime_context.tools))
        .unwrap_or_else(|| runtime_context.tools.clone())
}

fn merge_external_provider_tools(mut configured: Vec<Value>, mounted: &[Value]) -> Vec<Value> {
    let mut names = HashSet::new();
    for (index, tool) in configured.iter_mut().enumerate() {
        let Some(name) = provider_tool_name(tool) else {
            continue;
        };
        if names.insert(name.clone()) {
            continue;
        }
        let suffix = format!("_configured_{index}");
        let keep = 64usize.saturating_sub(suffix.len());
        let qualified = format!("{}{suffix}", &name[..name.len().min(keep)]);
        set_provider_tool_name(tool, &qualified);
        names.insert(qualified);
    }
    for (index, tool) in mounted.iter().enumerate() {
        let mut tool = tool.clone();
        if let Some(name) = provider_tool_name(&tool) {
            if !names.insert(name.clone()) {
                let suffix = format!("_run_{index}");
                let keep = 64usize.saturating_sub(suffix.len());
                let qualified = format!("{}{suffix}", &name[..name.len().min(keep)]);
                set_provider_tool_name(&mut tool, &qualified);
                names.insert(qualified);
            }
        }
        configured.push(tool);
    }
    configured
}

fn set_provider_tool_name(tool: &mut Value, name: &str) {
    if let Some(function) = tool.get_mut("function").and_then(Value::as_object_mut) {
        function.insert("name".to_string(), Value::String(name.to_string()));
    } else if let Some(object) = tool.as_object_mut() {
        object.insert("name".to_string(), Value::String(name.to_string()));
    }
}

fn provider_tool_name(tool: &Value) -> Option<String> {
    tool.get("function")
        .and_then(|value| value.get("name"))
        .or_else(|| tool.get("name"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn media_route_has_returned_to_main(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> bool {
    visible_internal_llm_node_has_media_tool(node)
        && media_route_context_mentions_image_path(resolved_inputs, variable_pool)
        && !pending_llm_tool_callback_visible_internal_events(node, variable_pool).is_empty()
}

fn media_route_context_mentions_image_path(
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> bool {
    ["query", "task"].iter().any(|key| {
        resolved_inputs
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(text_mentions_image_path)
    }) || resolved_inputs
        .get("files")
        .is_some_and(files_mention_image_path)
        || variable_pool.values().any(|payload| {
            ["query", "task"].iter().any(|key| {
                payload
                    .get(*key)
                    .and_then(Value::as_str)
                    .is_some_and(text_mentions_image_path)
            }) || payload.get("files").is_some_and(files_mention_image_path)
        })
}

fn files_mention_image_path(files: &Value) -> bool {
    files.as_array().is_some_and(|files| {
        files.iter().any(|file| {
            file.get("path")
                .or_else(|| file.get("file_path"))
                .and_then(Value::as_str)
                .is_some_and(text_mentions_image_path)
        })
    })
}

fn text_mentions_image_path(text: &str) -> bool {
    let Ok(pattern) = regex::Regex::new(r"(?i)[A-Za-z0-9_./\\:-]+\.(png|jpe?g|gif|webp|bmp)")
    else {
        return false;
    };
    pattern.is_match(text)
}

pub(super) fn run_level_provider_tools(
    plan: &CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> Vec<Value> {
    for candidate in [
        variable_pool.get("tools"),
        variable_pool
            .get("compatibility")
            .and_then(|value| value.get("tools")),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(tools) = candidate.as_array() {
            let provider_tools = provider_tool_payloads(tools);
            if !provider_tools.is_empty() {
                return provider_tools;
            }
        }
    }

    for node_id in &plan.topological_order {
        let Some(start_node) = plan.nodes.get(node_id) else {
            continue;
        };
        if start_node.node_type != "start" {
            continue;
        }
        let Some(payload) = variable_pool.get(node_id) else {
            continue;
        };
        for candidate in [
            payload.get("tools"),
            payload
                .get("compatibility")
                .and_then(|value| value.get("tools")),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(tools) = candidate.as_array() {
                let provider_tools = provider_tool_payloads(tools);
                if !provider_tools.is_empty() {
                    return provider_tools;
                }
            }
        }
    }

    Vec::new()
}

pub(super) fn run_level_tool_choice(
    plan: &CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> Option<Value> {
    if let Some(tool_choice) = variable_pool.get("tool_choice") {
        return Some(tool_choice.clone());
    }
    plan.topological_order.iter().find_map(|node_id| {
        let node = plan.nodes.get(node_id)?;
        if node.node_type != "start" {
            return None;
        }
        variable_pool.get(node_id)?.get("tool_choice").cloned()
    })
}

pub(super) fn provider_tool_payloads(tools: &[Value]) -> Vec<Value> {
    tools.iter().map(provider_tool_payload).collect()
}

pub(super) fn provider_tool_payload(tool: &Value) -> Value {
    if tool.get("function").is_some() {
        return tool.clone();
    }

    let Some(object) = tool.as_object() else {
        return tool.clone();
    };
    let Some(name) = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return tool.clone();
    };

    let mut function = Map::new();
    function.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(description) = object
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        function.insert(
            "description".to_string(),
            Value::String(description.to_string()),
        );
    }
    if let Some(input_schema) = object.get("input_schema") {
        function.insert("parameters".to_string(), input_schema.clone());
    }

    json!({
        "type": "function",
        "function": Value::Object(function),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_tools_preserve_duplicate_occurrences_with_source_qualified_names() {
        let configured = vec![
            json!({"type": "function", "function": { "name": "search" }}),
            json!({"type": "function", "function": { "name": "search" }}),
        ];
        let mounted = vec![
            json!({"type": "function", "function": { "name": "search" }}),
            json!({"type": "function", "function": { "name": "mcp_catalog" }}),
        ];

        let merged = merge_external_provider_tools(configured, &mounted);

        assert_eq!(merged.len(), 4);
        assert_eq!(provider_tool_name(&merged[0]).as_deref(), Some("search"));
        assert_eq!(
            provider_tool_name(&merged[1]).as_deref(),
            Some("search_configured_1")
        );
        assert_eq!(
            provider_tool_name(&merged[2]).as_deref(),
            Some("search_run_0")
        );
        assert_eq!(
            provider_tool_name(&merged[3]).as_deref(),
            Some("mcp_catalog")
        );
    }
}
