use super::*;

pub(super) struct BuiltProviderInvocation {
    pub(super) input: ProviderInvocationInput,
    pub(super) debug_context: LlmInvocationDebugContext,
}

#[derive(Debug, Clone)]
pub(super) struct LlmInvocationDebugContext {
    context_policy: Value,
    effective_system: Vec<NativePromptBlock>,
    provider_messages: Vec<Value>,
    compatibility_promotions: Vec<Value>,
    system_sources: Vec<Value>,
    previous_response_id: Option<String>,
    effective_max_output_tokens: Option<u64>,
    max_output_tokens_source: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LlmDebugInvocation<'a> {
    pub(super) messages: &'a [Value],
    pub(super) context: Option<&'a LlmInvocationDebugContext>,
}

impl LlmInvocationDebugContext {
    fn from_provider_context(
        context_policy: Value,
        previous_response_id: Option<String>,
        context: &ProviderPromptContext,
        model_parameters: &ResolvedLlmModelParameters,
    ) -> Self {
        Self {
            context_policy,
            effective_system: context.system.clone(),
            provider_messages: prompt_messages_from_provider_messages(&context.messages),
            compatibility_promotions: context.compatibility_promotions.clone(),
            system_sources: context.system_sources.clone(),
            previous_response_id,
            effective_max_output_tokens: model_parameters.effective_max_output_tokens,
            max_output_tokens_source: model_parameters.max_output_tokens_source,
        }
    }

    pub(super) fn to_payload(&self, result: Option<&ProviderInvocationResult>) -> Value {
        let mut payload = Map::new();
        payload.insert("context_policy".to_string(), self.context_policy.clone());
        payload.insert(
            "effective_system".to_string(),
            serde_json::to_value(&self.effective_system).unwrap_or(Value::Null),
        );
        payload.insert(
            "provider_messages".to_string(),
            Value::Array(self.provider_messages.clone()),
        );
        payload.insert(
            "compatibility_promotions".to_string(),
            Value::Array(self.compatibility_promotions.clone()),
        );
        payload.insert(
            "system_sources".to_string(),
            Value::Array(self.system_sources.clone()),
        );
        if let Some(previous_response_id) = &self.previous_response_id {
            payload.insert(
                "previous_response_id".to_string(),
                Value::String(previous_response_id.clone()),
            );
        }
        let effective_max_output_tokens = result
            .and_then(|result| {
                result
                    .provider_metadata
                    .get("effective_max_output_tokens")
                    .and_then(Value::as_u64)
            })
            .or(self.effective_max_output_tokens);
        payload.insert(
            "effective_max_output_tokens".to_string(),
            effective_max_output_tokens.map_or(Value::Null, Value::from),
        );
        payload.insert(
            "max_output_tokens_source".to_string(),
            Value::String(self.max_output_tokens_source.to_string()),
        );
        Value::Object(payload)
    }
}

#[derive(Debug, Clone)]
pub(super) struct ProviderPromptContext {
    pub(super) system: Vec<NativePromptBlock>,
    pub(super) messages: Vec<ProviderMessage>,
    pub(super) compatibility_promotions: Vec<Value>,
    pub(super) system_sources: Vec<Value>,
}

#[derive(Debug, Clone)]
pub(super) struct SystemPromptPart {
    pub(super) blocks: Vec<NativePromptBlock>,
    pub(super) source: Value,
}

pub(super) fn build_provider_invocation(
    plan: &CompiledPlan,
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    runtime_context: &ExecutionRuntimeContext,
) -> Result<BuiltProviderInvocation, Value> {
    let previous_response_id =
        pending_llm_tool_callback_previous_response_id(node, runtime, variable_pool);
    let context_policy = llm_context_policy(node, runtime);
    let mut provider_context = if previous_response_id.is_some() {
        let prompt_messages =
            if let Some(messages) = pending_llm_tool_callback_delta_messages(node, variable_pool) {
                messages
            } else {
                binding_prompt_messages_with_context_sources(
                    plan,
                    node,
                    rendered_templates,
                    resolved_inputs,
                    variable_pool,
                    &context_policy,
                    runtime_context,
                )?
            };
        provider_context_from_prompt_messages(prompt_messages)?
    } else {
        provider_context_from_prompt_messages(binding_prompt_messages_with_context_sources(
            plan,
            node,
            rendered_templates,
            resolved_inputs,
            variable_pool,
            &context_policy,
            runtime_context,
        )?)?
    };
    if provider_context.system.is_empty() {
        if let Some(system) = pending_llm_tool_callback_system(node, variable_pool)? {
            provider_context.system = system;
            provider_context.system_sources.push(json!({
                "source": format!("{}.{}", node.node_id, LLM_TOOL_CALLBACK_STATE_KEY),
                "source_kind": "pending_tool_callback_transcript",
                "target": "effective_system"
            }));
        }
    }

    let trace_context = BTreeMap::from([
        ("node_id".to_string(), node.node_id.clone()),
        ("node_alias".to_string(), node.alias.clone()),
    ]);
    let model_parameters = resolve_model_parameters(node, runtime, variable_pool);
    let debug_context = LlmInvocationDebugContext::from_provider_context(
        context_policy,
        previous_response_id.clone(),
        &provider_context,
        &model_parameters,
    );

    let mut run_context = BTreeMap::from([(
        "resolved_inputs".to_string(),
        Value::Object(resolved_inputs.clone()),
    )]);
    if let Some(media_tools) = visible_internal_llm_media_tool_context(node) {
        run_context.insert("visible_internal_llm_media_tools".to_string(), media_tools);
    }

    let mut input = ProviderInvocationInput {
        contract_version: Default::default(),
        provider_instance_id: runtime.provider_instance_id.clone(),
        provider_code: runtime.provider_code.clone(),
        protocol: runtime.protocol.clone(),
        model: runtime.model.clone(),
        previous_response_id,
        provider_config: Value::Null,
        messages: provider_context.messages,
        system: provider_context.system,
        request_context: runtime_context.native_model_request_context.clone(),
        required_capabilities: BTreeSet::new(),
        tools: provider_tools(
            node,
            resolved_inputs,
            rendered_templates,
            variable_pool,
            runtime_context,
        ),
        mcp_bindings: Vec::new(),
        response_format: build_response_format(&node.config),
        model_parameters: model_parameters.values,
        client_protocol_envelope: runtime_context.client_protocol_envelope.clone(),
        trace_context,
        run_context,
    };
    input.synchronize_required_capabilities();

    Ok(BuiltProviderInvocation {
        input,
        debug_context,
    })
}

pub(super) fn prompt_messages_from_provider_messages(messages: &[ProviderMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let mut payload = Map::new();
            payload.insert(
                "role".to_string(),
                serde_json::to_value(&message.role).unwrap_or(Value::Null),
            );
            payload.insert(
                "content".to_string(),
                Value::String(message.content.clone()),
            );
            if let Some(name) = &message.name {
                payload.insert("name".to_string(), Value::String(name.clone()));
            }
            if let Some(tool_call_id) = &message.tool_call_id {
                payload.insert(
                    "tool_call_id".to_string(),
                    Value::String(tool_call_id.clone()),
                );
            }
            if let Some(is_error) = message.is_error {
                payload.insert("is_error".to_string(), Value::Bool(is_error));
            }
            if let Some(tool_calls) = &message.tool_calls {
                payload.insert("tool_calls".to_string(), tool_calls.clone());
            }
            if let Some(content_blocks) = &message.content_blocks {
                payload.insert("content_blocks".to_string(), content_blocks.clone());
            }

            Value::Object(payload)
        })
        .collect()
}

pub(super) fn build_llm_debug_invocation_messages(
    node: &CompiledNode,
    resolved_inputs: &Map<String, Value>,
    rendered_templates: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
    invocation_input: &ProviderInvocationInput,
) -> Vec<Value> {
    if invocation_input.previous_response_id.is_some()
        || pending_llm_tool_callback_state(variable_pool, &node.node_id).is_some()
    {
        return binding_prompt_messages(node, rendered_templates, resolved_inputs, variable_pool);
    }

    prompt_messages_from_provider_messages(&invocation_input.messages)
}

pub(super) fn has_pending_tool_calls(output_payload: &Value) -> bool {
    output_payload
        .get("tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|tool_calls| !tool_calls.is_empty())
}
