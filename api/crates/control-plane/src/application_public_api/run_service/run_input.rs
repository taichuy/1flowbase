use serde_json::{json, Map, Value};
use uuid::Uuid;

use super::super::{
    model_catalog::{extract_agent_model_catalog_from_start_node, find_agent_model},
    native::{NativeRunRequest, NativeRunValidationError},
};

pub(super) fn generate_external_conversation_id() -> String {
    format!("conv_{}", Uuid::now_v7().simple())
}

pub(crate) fn freeze_run_input_environment(
    input_payload: Value,
    variables: &[domain::ApplicationEnvironmentVariable],
    external_model_parameters: Option<Value>,
    start_node_id: Option<&str>,
) -> Value {
    let mut payload = input_payload.as_object().cloned().unwrap_or_default();
    payload.insert(
        "env".to_string(),
        Value::Object(application_environment_variable_payload(variables)),
    );
    if let Some(model_parameters) = external_model_parameters {
        let mut sys = payload
            .remove("sys")
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        let reasoning_effort = external_reasoning_effort(&model_parameters).unwrap_or_default();
        sys.insert("model_parameters".to_string(), model_parameters);
        insert_start_reasoning_effort(&mut payload, start_node_id, reasoning_effort);
        payload.insert("sys".to_string(), Value::Object(sys));
    }
    Value::Object(payload)
}

pub(crate) fn compiled_plan_start_node_id(plan: &Value) -> Option<String> {
    plan.get("nodes")
        .and_then(Value::as_object)?
        .iter()
        .find_map(|(node_id, node)| {
            matches!(
                node.get("node_type").and_then(Value::as_str),
                Some("start" | "workflow_start")
            )
            .then(|| node_id.clone())
        })
}

fn insert_start_reasoning_effort(
    payload: &mut Map<String, Value>,
    start_node_id: Option<&str>,
    reasoning_effort: String,
) {
    let start_node_id = start_node_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("node-start");
    let start_payload = payload
        .entry(start_node_id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if !start_payload.is_object() {
        *start_payload = Value::Object(Map::new());
    }
    if let Some(start_payload) = start_payload.as_object_mut() {
        start_payload.insert(
            "reasoning_effort".to_string(),
            Value::String(reasoning_effort),
        );
    }
}

fn external_reasoning_effort(model_parameters: &Value) -> Option<String> {
    model_parameters
        .get("reasoning")
        .and_then(|value| value.get("effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn validate_external_model_parameters(
    request: &NativeRunRequest,
    document_snapshot: &Value,
) -> std::result::Result<Option<Value>, NativeRunValidationError> {
    let Some(model_parameters) = request.execution.get("model_parameters") else {
        return Ok(None);
    };
    let model_parameters =
        model_parameters
            .as_object()
            .ok_or(NativeRunValidationError::InvalidModelParameters(
                "execution.model_parameters",
            ))?;
    for key in model_parameters.keys() {
        if key != "reasoning" {
            return Err(NativeRunValidationError::InvalidModelParameters(
                "execution.model_parameters",
            ));
        }
    }
    let Some(reasoning) = model_parameters.get("reasoning") else {
        return Ok(Some(json!({})));
    };
    let reasoning =
        reasoning
            .as_object()
            .ok_or(NativeRunValidationError::InvalidModelParameters(
                "execution.model_parameters.reasoning",
            ))?;
    for key in reasoning.keys() {
        if !matches!(key.as_str(), "enabled" | "effort" | "budget_tokens") {
            return Err(NativeRunValidationError::InvalidModelParameters(
                "execution.model_parameters.reasoning",
            ));
        }
    }

    let enabled = reasoning
        .get("enabled")
        .map(|value| {
            value
                .as_bool()
                .ok_or(NativeRunValidationError::InvalidModelParameters(
                    "execution.model_parameters.reasoning.enabled",
                ))
        })
        .transpose()?
        .unwrap_or(true);
    let effort = reasoning
        .get("effort")
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .ok_or(NativeRunValidationError::InvalidModelParameters(
                    "execution.model_parameters.reasoning.effort",
                ))
        })
        .transpose()?;
    if let Some(effort) = effort.as_deref() {
        if !is_known_reasoning_effort(effort) {
            return Err(NativeRunValidationError::InvalidModelParameters(
                "execution.model_parameters.reasoning.effort",
            ));
        }
    }
    let budget_tokens = reasoning
        .get("budget_tokens")
        .map(|value| {
            value.as_u64().filter(|value| *value > 0).ok_or(
                NativeRunValidationError::InvalidModelParameters(
                    "execution.model_parameters.reasoning.budget_tokens",
                ),
            )
        })
        .transpose()?;

    if enabled || effort.is_some() || budget_tokens.is_some() {
        let model_id = request
            .model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(NativeRunValidationError::InvalidModelParameters("model"))?;
        let models = extract_agent_model_catalog_from_start_node(document_snapshot);
        let model = find_agent_model(&models, model_id)
            .ok_or(NativeRunValidationError::InvalidModelParameters("model"))?;
        let supports_reasoning = model.capabilities.reasoning
            || model.reasoning.as_ref().is_some_and(|reasoning| {
                reasoning.default_effort.is_some() || !reasoning.supported_efforts.is_empty()
            });
        if !supports_reasoning {
            return Err(NativeRunValidationError::InvalidModelParameters(
                "execution.model_parameters.reasoning",
            ));
        }
        if let Some(effort) = effort.as_deref() {
            if let Some(reasoning) = model.reasoning.as_ref() {
                if !reasoning.supported_efforts.is_empty()
                    && !reasoning
                        .supported_efforts
                        .iter()
                        .any(|supported| supported == effort)
                {
                    return Err(NativeRunValidationError::InvalidModelParameters(
                        "execution.model_parameters.reasoning.effort",
                    ));
                }
            }
        }
        if let (Some(budget_tokens), Some(max_output_tokens)) =
            (budget_tokens, model.max_output_tokens)
        {
            if budget_tokens > max_output_tokens {
                return Err(NativeRunValidationError::InvalidModelParameters(
                    "execution.model_parameters.reasoning.budget_tokens",
                ));
            }
        }
    }

    let mut clean_reasoning = Map::new();
    clean_reasoning.insert("enabled".to_string(), Value::Bool(enabled));
    if let Some(effort) = effort {
        clean_reasoning.insert("effort".to_string(), Value::String(effort));
    }
    if let Some(budget_tokens) = budget_tokens {
        clean_reasoning.insert("budget_tokens".to_string(), json!(budget_tokens));
    }

    Ok(Some(json!({
        "reasoning": Value::Object(clean_reasoning)
    })))
}

fn is_known_reasoning_effort(effort: &str) -> bool {
    matches!(effort, "minimal" | "low" | "medium" | "high" | "xhigh")
}

fn application_environment_variable_payload(
    variables: &[domain::ApplicationEnvironmentVariable],
) -> Map<String, Value> {
    variables
        .iter()
        .map(|variable| (variable.name.clone(), variable.value.clone()))
        .collect()
}
