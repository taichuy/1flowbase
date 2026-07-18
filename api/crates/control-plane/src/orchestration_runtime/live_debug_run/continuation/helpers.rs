use serde_json::{json, Map, Value};

pub(super) fn inject_system_variables(
    variable_pool: &mut Map<String, Value>,
    flow_run: &domain::FlowRunRecord,
    application_type: domain::ApplicationType,
    start_node_id: Option<&str>,
) {
    let mut sys = json!({
        "application_id": flow_run.application_id.to_string(),
        "workflow_id": flow_run.flow_id.to_string(),
        "workflow_run_id": flow_run.id.to_string(),
    });
    if application_type == domain::ApplicationType::Workflow {
        variable_pool.insert("sys".to_string(), sys);
        return;
    }

    let conversation_id = flow_run
        .external_conversation_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or(&flow_run.debug_session_id);
    let model_parameters = variable_pool
        .get("sys")
        .and_then(|value| value.get("model_parameters"))
        .cloned();
    let has_model_parameters = model_parameters.is_some();
    let start_has_reasoning_effort = start_node_id
        .and_then(|node_id| variable_pool.get(node_id))
        .and_then(Value::as_object)
        .is_some_and(|payload| payload.contains_key("reasoning_effort"));
    let sys_has_reasoning_effort = variable_pool
        .get("sys")
        .and_then(Value::as_object)
        .is_some_and(|payload| payload.contains_key("reasoning_effort"));
    let reasoning_effort = variable_pool
        .get(start_node_id.unwrap_or("node-start"))
        .and_then(|value| value.get("reasoning_effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            variable_pool
                .get("sys")
                .and_then(|value| value.get("reasoning_effort"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            model_parameters
                .as_ref()
                .and_then(external_reasoning_effort)
        });
    let max_output_tokens = model_parameters
        .as_ref()
        .and_then(external_max_output_tokens);

    sys["conversation_id"] = json!(conversation_id);
    sys["dialog_count"] = json!(0);
    sys["user_id"] = json!(flow_run.created_by.to_string());
    if let Some(model_parameters) = model_parameters {
        sys["model_parameters"] = model_parameters;
    }

    variable_pool.insert("sys".to_string(), sys);
    if start_has_reasoning_effort || sys_has_reasoning_effort || has_model_parameters {
        insert_start_model_parameters(
            variable_pool,
            start_node_id,
            reasoning_effort.unwrap_or_default(),
            max_output_tokens,
        );
    }
}

pub(super) fn compiled_plan_start_node_id(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
) -> Option<&str> {
    compiled_plan
        .nodes
        .values()
        .find(|node| matches!(node.node_type.as_str(), "start" | "workflow_start"))
        .map(|node| node.node_id.as_str())
}

fn insert_start_model_parameters(
    variable_pool: &mut Map<String, Value>,
    start_node_id: Option<&str>,
    reasoning_effort: String,
    max_output_tokens: Option<u64>,
) {
    let start_node_id = start_node_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("node-start");
    let start_payload = variable_pool
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
        if let Some(max_output_tokens) = max_output_tokens {
            start_payload.insert("max_output_tokens".to_string(), json!(max_output_tokens));
        }
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

fn external_max_output_tokens(model_parameters: &Value) -> Option<u64> {
    model_parameters
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
}

pub(super) fn inject_application_environment_variables(
    variable_pool: &mut Map<String, Value>,
    variables: &[domain::ApplicationEnvironmentVariable],
) {
    variable_pool.insert(
        "env".to_string(),
        Value::Object(
            variables
                .iter()
                .map(|variable| (variable.name.clone(), variable.value.clone()))
                .collect(),
        ),
    );
}
