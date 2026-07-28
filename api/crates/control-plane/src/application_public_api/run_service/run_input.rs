use serde_json::{json, Map, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use super::super::native::NativeExecutionModelParameters;

pub(super) fn generate_external_conversation_id() -> String {
    format!("conv_{}", Uuid::now_v7().simple())
}

pub(crate) fn freeze_run_input_environment(
    input_payload: Value,
    variables: &[domain::ApplicationEnvironmentVariable],
    external_model_parameters: Option<&NativeExecutionModelParameters>,
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
        sys.insert(
            "model_parameters".to_string(),
            model_parameters.canonical_value(),
        );
        payload.insert("sys".to_string(), Value::Object(sys));
    }
    Value::Object(payload)
}

pub(crate) enum WorkflowRunTriggerContext<'a> {
    Extension,
    Schedule {
        scheduled_at: OffsetDateTime,
        timezone: &'a str,
    },
}

pub(crate) fn freeze_workflow_run_input_environment(
    input_payload: Value,
    variables: &[domain::ApplicationEnvironmentVariable],
    trigger_context: WorkflowRunTriggerContext<'_>,
) -> Result<Value, time::error::Format> {
    let mut payload = input_payload.as_object().cloned().unwrap_or_default();
    payload.remove("sys");
    payload.remove("trigger");
    payload.insert(
        "env".to_string(),
        Value::Object(application_environment_variable_payload(variables)),
    );
    let trigger = match trigger_context {
        WorkflowRunTriggerContext::Extension => json!({ "type": "extension" }),
        WorkflowRunTriggerContext::Schedule {
            scheduled_at,
            timezone,
        } => json!({
            "type": "schedule",
            "scheduled_at": scheduled_at.format(&Rfc3339)?,
            "timezone": timezone,
        }),
    };

    payload.insert("trigger".to_string(), trigger);
    Ok(Value::Object(payload))
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

fn application_environment_variable_payload(
    variables: &[domain::ApplicationEnvironmentVariable],
) -> Map<String, Value> {
    variables
        .iter()
        .map(|variable| (variable.name.clone(), variable.value.clone()))
        .collect()
}
