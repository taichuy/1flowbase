use serde_json::{json, Map, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use super::super::native::NativeExecutionModelParameters;
use super::super::{
    client_protocol_envelope::{
        anthropic_messages_envelope_with_beta, merge_anthropic_messages_envelopes,
        ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
    },
    model_catalog::{extract_agent_model_catalog_from_start_node, find_agent_model},
    native::NativeRunRequest,
};

const ANTHROPIC_CONTEXT_1M_TOKENS: u64 = 1_000_000;

pub(super) fn generate_external_conversation_id() -> String {
    format!("conv_{}", Uuid::now_v7().simple())
}

pub(super) fn enrich_anthropic_context_beta_from_start_model(
    request: &mut NativeRunRequest,
    document_snapshot: &Value,
) {
    let Some(envelope) = request.client_protocol_envelope.as_ref() else {
        return;
    };
    if envelope.source_protocol != "anthropic_messages" {
        return;
    }
    let Some(model_id) = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return;
    };
    let catalog = extract_agent_model_catalog_from_start_node(document_snapshot);
    let Some(model) = find_agent_model(&catalog, model_id) else {
        return;
    };
    if model.context_window.unwrap_or_default() < ANTHROPIC_CONTEXT_1M_TOKENS {
        return;
    }
    request.client_protocol_envelope = merge_anthropic_messages_envelopes(
        request.client_protocol_envelope.take(),
        Some(anthropic_messages_envelope_with_beta(
            ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
        )),
    );
}

pub(crate) fn freeze_run_input_environment(
    input_payload: Value,
    variables: &[domain::ApplicationEnvironmentVariable],
    external_model_parameters: Option<&NativeExecutionModelParameters>,
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
        let reasoning_effort = model_parameters
            .reasoning()
            .and_then(|reasoning| reasoning.effort())
            .unwrap_or_default()
            .to_string();
        let max_output_tokens = model_parameters.max_output_tokens();
        sys.insert(
            "model_parameters".to_string(),
            model_parameters.canonical_value(),
        );
        insert_start_model_parameters(
            &mut payload,
            start_node_id,
            reasoning_effort,
            max_output_tokens,
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

fn insert_start_model_parameters(
    payload: &mut Map<String, Value>,
    start_node_id: Option<&str>,
    reasoning_effort: String,
    max_output_tokens: Option<u64>,
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
        if let Some(max_output_tokens) = max_output_tokens {
            start_payload.insert("max_output_tokens".to_string(), json!(max_output_tokens));
        }
    }
}

fn application_environment_variable_payload(
    variables: &[domain::ApplicationEnvironmentVariable],
) -> Map<String, Value> {
    variables
        .iter()
        .map(|variable| (variable.name.clone(), variable.value.clone()))
        .collect()
}
