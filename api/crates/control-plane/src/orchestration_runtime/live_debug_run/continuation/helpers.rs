use super::*;
use crate::orchestration_runtime::answer_presentation;
use orchestration_runtime::payload_builder::PublicOutputContract;
use serde_json::Map;
use std::collections::BTreeSet;

pub(super) fn inject_system_variables(
    variable_pool: &mut serde_json::Map<String, Value>,
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

    sys["conversation_id"] = json!(conversation_id);
    sys["dialog_count"] = json!(0);
    sys["user_id"] = json!(flow_run.created_by.to_string());
    if let Some(model_parameters) = model_parameters {
        sys["model_parameters"] = model_parameters;
    }

    variable_pool.insert("sys".to_string(), sys);
    if start_has_reasoning_effort || sys_has_reasoning_effort || has_model_parameters {
        insert_start_reasoning_effort(
            variable_pool,
            start_node_id,
            reasoning_effort.unwrap_or_default(),
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

pub(super) fn insert_start_reasoning_effort(
    variable_pool: &mut serde_json::Map<String, Value>,
    start_node_id: Option<&str>,
    reasoning_effort: String,
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
    }
}

pub(super) fn external_reasoning_effort(model_parameters: &Value) -> Option<String> {
    model_parameters
        .get("reasoning")
        .and_then(|value| value.get("effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn start_node_input_payload(
    variable_pool: &serde_json::Map<String, Value>,
    node_id: &str,
) -> Value {
    let mut payload = variable_pool
        .get(node_id)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if let Some(sys) = variable_pool.get("sys") {
        payload.insert("sys".to_string(), sys.clone());
    }
    if let Some(env) = variable_pool.get("env") {
        payload.insert("env".to_string(), env.clone());
    }
    if let Some(trigger) = variable_pool.get("trigger") {
        payload.insert("trigger".to_string(), trigger.clone());
    }

    Value::Object(payload)
}

pub(super) fn template_output_payload(
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    output_key: String,
    output_value: Value,
    variable_pool: &serde_json::Map<String, Value>,
) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert(output_key, output_value);

    if node.node_type == "answer" {
        if let Some(sys) = variable_pool.get("sys") {
            payload.insert("sys".to_string(), sys.clone());
        }
        if let Some(env) = variable_pool.get("env") {
            payload.insert("env".to_string(), env.clone());
        }
    }

    Value::Object(payload)
}

pub(super) fn project_node_variable_payload(
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    output_payload: &Value,
) -> Result<Value> {
    if node.node_type == "code" {
        return Ok(output_payload.clone());
    }

    PublicOutputContract::from_compiled_outputs(&node.outputs)?
        .project_variable_payload(output_payload)
}

pub(super) fn can_continue_to_terminal_template_nodes(
    plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    failed_node_index: usize,
    active_node_ids: &BTreeSet<String>,
) -> bool {
    let mut has_terminal_template_node = false;
    for node_id in plan.topological_order.iter().skip(failed_node_index + 1) {
        if !active_node_ids.contains(node_id) {
            continue;
        }

        let Some(node) = plan.nodes.get(node_id) else {
            return false;
        };
        if !matches!(node.node_type.as_str(), "template_transform" | "answer") {
            return false;
        }
        has_terminal_template_node = true;
    }
    has_terminal_template_node
}

pub(super) async fn emit_answer_presentation_for_node<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: uuid::Uuid,
    answer_presentation: Option<&Arc<Mutex<answer_presentation::AnswerPresentationCursor>>>,
    node_id: &str,
    node_run_id: uuid::Uuid,
    output_payload: &Value,
) where
    R: OrchestrationRuntimeRepository,
{
    let Some(answer_presentation) = answer_presentation else {
        return;
    };
    let events =
        answer_presentation
            .lock()
            .await
            .complete_node(node_id, node_run_id, output_payload);
    for event in events {
        append_runtime_event(service, flow_run_id, event).await;
    }
}

pub(super) async fn materialize_ready_answer_node_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run_id: uuid::Uuid,
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    variable_pool: &Map<String, Value>,
) -> Result<Option<Value>>
where
    R: OrchestrationRuntimeRepository,
{
    let Some(ready) =
        answer_presentation::ready_answer_output_from_variable_pool(compiled_plan, variable_pool)
    else {
        return Ok(None);
    };
    let Some(answer_node) = compiled_plan.nodes.get(&ready.answer_node_id) else {
        return Ok(None);
    };
    let started_at = OffsetDateTime::now_utc();
    let node_run = service
        .repository
        .create_node_run(&CreateNodeRunInput {
            flow_run_id,
            node_id: answer_node.node_id.clone(),
            node_type: answer_node.node_type.clone(),
            node_alias: answer_node.alias.clone(),
            status: domain::NodeRunStatus::Running,
            input_payload: json!({
                "presentation": {
                    "kind": "answer",
                    "complete": ready.complete,
                    "materialized_from": "waiting_prefix"
                }
            }),
            debug_payload: json!({}),
            started_at,
        })
        .await?;
    append_runtime_event(
        service,
        flow_run_id,
        debug_stream_events::node_started(&node_run),
    )
    .await;

    ensure_node_run_transition(
        domain::NodeRunStatus::Running,
        domain::NodeRunStatus::Succeeded,
        "materialize_waiting_answer_node",
    )?;
    let output_payload = answer_presentation::ready_answer_output_payload(&ready, variable_pool);
    update_node_run_and_emit(
        service,
        flow_run_id,
        &UpdateNodeRunInput {
            node_run_id: node_run.id,
            status: domain::NodeRunStatus::Succeeded,
            output_payload: output_payload.clone(),
            error_payload: None,
            metrics_payload: json!({
                "preview_mode": true,
                "answer_presentation": {
                    "partial": !ready.complete,
                    "materialized_from": "waiting_prefix"
                }
            }),
            debug_payload: json!({
                "answer_presentation": {
                    "partial": !ready.complete,
                    "materialized_from": "waiting_prefix"
                }
            }),
            finished_at: Some(started_at),
        },
    )
    .await?;

    if ready.text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output_payload))
    }
}

pub(super) async fn fail_current_live_run_after_node_error<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: &ContinueFlowDebugRunCommand,
    flow_run: &domain::FlowRunRecord,
    node_run_id: uuid::Uuid,
    output_payload: Value,
    error_payload: Value,
) -> Result<domain::ApplicationRunDetail>
where
    R: OrchestrationRuntimeRepository,
{
    ensure_flow_run_transition(
        domain::FlowRunStatus::Running,
        domain::FlowRunStatus::Failed,
        "continue_flow_debug_run",
    )?;
    service
        .repository
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id: flow_run.id,
            status: domain::FlowRunStatus::Failed,
            output_payload,
            error_payload: Some(error_payload.clone()),
            finished_at: Some(OffsetDateTime::now_utc()),
        })
        .await?;
    emit_flow_failed_and_close(service, flow_run.id, error_payload.clone()).await;
    service
        .repository
        .append_run_event(&AppendRunEventInput {
            flow_run_id: flow_run.id,
            node_run_id: Some(node_run_id),
            event_type: "flow_run_failed".to_string(),
            payload: error_payload,
        })
        .await?;
    load_run_detail(&service.repository, command.application_id, flow_run.id).await
}

pub(super) fn inject_application_environment_variables(
    variable_pool: &mut serde_json::Map<String, Value>,
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
