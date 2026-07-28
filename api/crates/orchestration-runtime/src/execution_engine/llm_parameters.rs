use super::*;

pub(super) fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

pub(super) struct ResolvedLlmModelParameters {
    pub(super) values: BTreeMap<String, Value>,
    pub(super) effective_max_output_tokens: Option<u64>,
    pub(super) max_output_tokens_source: &'static str,
}

pub(super) fn resolve_model_parameters(
    plan: &CompiledPlan,
    node: &CompiledNode,
    runtime: &CompiledLlmRuntime,
    resolved_inputs: &Map<String, Value>,
    variable_pool: &Map<String, Value>,
) -> Result<ResolvedLlmModelParameters, Value> {
    if let Some(field) = legacy_max_tokens_field(&node.config) {
        return Err(json!({
            "error_code": "unsupported_model_parameter",
            "message": "max_tokens is unsupported; use max_output_tokens",
            "field": field,
        }));
    }
    let mut parameters = build_configured_model_parameters(&node.config);
    if llm_follows_external_reasoning(&node.config) {
        apply_external_reasoning_parameters(&mut parameters, runtime, variable_pool);
    }
    if !parameters.contains_key("requested_context_window") {
        if let Some(requested_context_window) = external_requested_context_window(variable_pool) {
            parameters.insert(
                "requested_context_window".to_string(),
                json!(requested_context_window),
            );
        }
    }
    if !parameters.contains_key("tool_choice") {
        let external_tool_choice = resolved_inputs
            .get("tool_choice")
            .cloned()
            .or_else(|| run_level_tool_choice(plan, variable_pool));
        if let Some(tool_choice) = external_tool_choice.filter(|value| {
            !value.is_null()
                && !value.as_str().is_some_and(|value| value.trim().is_empty())
                && !value.as_object().is_some_and(Map::is_empty)
        }) {
            parameters.insert("tool_choice".to_string(), tool_choice);
        }
    }

    let configured_max_output_tokens = parameters.get("max_output_tokens").and_then(parameter_u64);
    let (effective_max_output_tokens, max_output_tokens_source) =
        if parameters.contains_key("max_output_tokens") {
            (configured_max_output_tokens, "llm_node")
        } else if llm_follows_external_max_output_tokens(&node.config) {
            match external_max_output_tokens(variable_pool) {
                Some(max_output_tokens) => {
                    parameters.insert("max_output_tokens".to_string(), json!(max_output_tokens));
                    (Some(max_output_tokens), "external_request")
                }
                None => (None, "provider_default"),
            }
        } else {
            (None, "provider_default")
        };

    Ok(ResolvedLlmModelParameters {
        values: parameters,
        effective_max_output_tokens,
        max_output_tokens_source,
    })
}

fn legacy_max_tokens_field(config: &Value) -> Option<&'static str> {
    if config.get("max_tokens").is_some() {
        return Some("max_tokens");
    }
    config
        .get("llm_parameters")
        .and_then(Value::as_object)
        .and_then(|parameters| parameters.get("items"))
        .and_then(Value::as_object)
        .and_then(|items| {
            items
                .contains_key("max_tokens")
                .then_some("llm_parameters.items.max_tokens")
        })
}

pub(super) fn build_configured_model_parameters(config: &Value) -> BTreeMap<String, Value> {
    if let Some(items) = config
        .get("llm_parameters")
        .and_then(Value::as_object)
        .and_then(|value| value.get("items"))
        .and_then(Value::as_object)
    {
        return items
            .iter()
            .filter_map(|(key, item)| {
                let enabled = item
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let value = item.get("value").cloned().unwrap_or(Value::Null);
                enabled.then_some((key.clone(), value))
            })
            .collect();
    }

    [
        "temperature",
        "top_p",
        "presence_penalty",
        "frequency_penalty",
        "max_output_tokens",
        "seed",
    ]
    .into_iter()
    .filter_map(|key| {
        config
            .get(key)
            .cloned()
            .map(|value| (key.to_string(), value))
    })
    .collect()
}

pub(super) fn llm_follows_external_reasoning(config: &Value) -> bool {
    config
        .get("external_reasoning_policy")
        .and_then(|value| value.get("follow_external_reasoning"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn llm_follows_external_max_output_tokens(config: &Value) -> bool {
    config
        .get("external_model_parameter_policy")
        .and_then(|value| value.get("follow_external_max_output_tokens"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn external_max_output_tokens(variable_pool: &Map<String, Value>) -> Option<u64> {
    variable_pool
        .get("sys")
        .and_then(|value| value.get("model_parameters"))
        .and_then(|value| value.get("max_output_tokens"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
}

fn external_requested_context_window(variable_pool: &Map<String, Value>) -> Option<u64> {
    variable_pool
        .get("sys")
        .and_then(|value| value.get("model_parameters"))
        .and_then(|value| value.get("requested_context_window"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
}

fn parameter_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

pub(super) fn apply_external_reasoning_parameters(
    parameters: &mut BTreeMap<String, Value>,
    runtime: &CompiledLlmRuntime,
    variable_pool: &Map<String, Value>,
) {
    let Some(reasoning) = variable_pool
        .get("sys")
        .and_then(|value| value.get("model_parameters"))
        .and_then(|value| value.get("reasoning"))
        .and_then(Value::as_object)
    else {
        return;
    };
    let enabled = reasoning
        .get("mode")
        .and_then(Value::as_str)
        .map(|mode| mode != "disabled")
        .or_else(|| reasoning.get("enabled").and_then(Value::as_bool))
        .unwrap_or(true);
    let effort = reasoning
        .get("effort")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if is_bailian_reasoning_runtime(runtime) {
        insert_model_parameter_if_absent(parameters, "enable_thinking", json!(enabled));
        if enabled {
            if let Some(effort) = effort {
                insert_model_parameter_if_absent(parameters, "reasoning_effort", json!(effort));
            }
        }
        return;
    }

    if is_anthropic_reasoning_runtime(runtime) || is_openai_reasoning_runtime(runtime) {
        let reasoning = ["mode", "effort", "budget_tokens"]
            .into_iter()
            .filter_map(|key| {
                reasoning
                    .get(key)
                    .cloned()
                    .map(|value| (key.to_string(), value))
            })
            .collect::<Map<_, _>>();
        if !reasoning.is_empty() {
            insert_model_parameter_if_absent(parameters, "reasoning", Value::Object(reasoning));
        }
    }
}

pub(super) fn insert_model_parameter_if_absent(
    parameters: &mut BTreeMap<String, Value>,
    key: &'static str,
    value: Value,
) {
    parameters.entry(key.to_string()).or_insert(value);
}

pub(super) fn is_openai_reasoning_runtime(runtime: &CompiledLlmRuntime) -> bool {
    runtime.provider_code == "openai"
        || runtime.provider_code == "openai_compatible"
        || runtime.protocol == "openai_responses"
        || runtime.protocol == "openai_compatible"
}

pub(super) fn is_anthropic_reasoning_runtime(runtime: &CompiledLlmRuntime) -> bool {
    runtime.provider_code == "anthropic" || runtime.protocol == "anthropic_messages"
}

pub(super) fn is_bailian_reasoning_runtime(runtime: &CompiledLlmRuntime) -> bool {
    runtime.provider_code == "aliyun_bailian" || runtime.provider_code == "bailian"
}
