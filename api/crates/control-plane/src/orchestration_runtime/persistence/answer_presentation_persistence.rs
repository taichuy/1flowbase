use super::*;
use crate::orchestration_runtime::answer_presentation;

pub(super) async fn materialize_ready_answer_node_run<R>(
    repository: &R,
    flow_run_id: Uuid,
    compiled_plan: Option<&orchestration_runtime::compiled_plan::CompiledPlan>,
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    started_at: OffsetDateTime,
) -> Result<Option<Value>>
where
    R: OrchestrationRuntimeRepository,
{
    let Some(compiled_plan) = compiled_plan else {
        return Ok(None);
    };
    let checkpoint = outcome
        .checkpoint_snapshot
        .as_ref()
        .ok_or_else(|| anyhow!("waiting Answer Presentation is missing checkpoint state"))?;
    let variable_pool = &checkpoint.variable_pool;
    let waiting_node_id = waiting_node_id(outcome)?;
    let Some(ready) = answer_presentation::ready_waiting_answer_output_from_variable_pool(
        compiled_plan,
        variable_pool,
        &checkpoint.active_node_ids,
        waiting_node_id,
    ) else {
        return Ok(None);
    };
    let Some(answer_node) = compiled_plan.nodes.get(&ready.answer_node_id) else {
        return Ok(None);
    };
    let output_payload = answer_presentation::ready_answer_output_payload(&ready, variable_pool);
    let node_run = repository
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
    ensure_node_run_transition(
        domain::NodeRunStatus::Running,
        domain::NodeRunStatus::Succeeded,
        "materialize_waiting_answer_node",
    )?;
    repository
        .update_node_run(&UpdateNodeRunInput {
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
        })
        .await?;

    if ready.text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output_payload))
    }
}

pub(super) async fn append_answer_presentation_suffix<R>(
    repository: &R,
    flow_run_id: Uuid,
    answer_node_id: &str,
    output_payload: &Value,
) -> Result<Vec<crate::ports::RuntimeEventPayload>>
where
    R: OrchestrationRuntimeRepository,
{
    let Some(answer) = output_payload.get("answer").and_then(Value::as_str) else {
        return Ok(Vec::new());
    };
    if answer.is_empty() {
        return Ok(Vec::new());
    }

    let visible_answer = answer_presentation::visible_answer_text(answer);
    let existing = existing_answer_presentation_text(repository, flow_run_id, "text_delta").await?;
    let suffix = visible_answer
        .strip_prefix(&existing)
        .unwrap_or(&visible_answer);
    if suffix.is_empty() {
        return Ok(Vec::new());
    }

    let event = debug_stream_events::answer_text_delta(
        answer_node_id,
        suffix.to_string(),
        0,
        None,
        None,
        None,
    );
    runtime_event_persister::persist_runtime_event_payload(repository, flow_run_id, &event).await?;
    Ok(vec![event])
}

pub(super) async fn append_ready_answer_presentation_prefix<R>(
    repository: &R,
    flow_run_id: Uuid,
    compiled_plan: Option<&orchestration_runtime::compiled_plan::CompiledPlan>,
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> Result<Vec<crate::ports::RuntimeEventPayload>>
where
    R: OrchestrationRuntimeRepository,
{
    let Some(compiled_plan) = compiled_plan else {
        return Ok(Vec::new());
    };
    let checkpoint = outcome
        .checkpoint_snapshot
        .as_ref()
        .ok_or_else(|| anyhow!("waiting Answer Presentation is missing checkpoint state"))?;
    let variable_pool = &checkpoint.variable_pool;
    let waiting_node_id = waiting_node_id(outcome)?;
    let Some(ready) = answer_presentation::ready_waiting_answer_output_from_variable_pool(
        compiled_plan,
        variable_pool,
        &checkpoint.active_node_ids,
        waiting_node_id,
    ) else {
        return Ok(Vec::new());
    };
    let Some(presentation) =
        orchestration_runtime::answer_presentation::AnswerPresentationPlan::candidates_from_plan(
            compiled_plan,
        )
        .into_iter()
        .find(|presentation| presentation.answer_node_id == ready.answer_node_id)
    else {
        return Ok(Vec::new());
    };
    let mut cursor = answer_presentation::AnswerPresentationCursor::from_presentation(presentation);
    let mut candidate_events = Vec::new();

    for node_id in &compiled_plan.topological_order {
        let Some(output_payload) = variable_pool.get(node_id) else {
            continue;
        };
        candidate_events.extend(cursor.complete_node_with_run_id(node_id, None, output_payload));
    }

    append_missing_answer_presentation_events(repository, flow_run_id, candidate_events).await
}

fn waiting_node_id(
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> Result<&str> {
    match &outcome.stop_reason {
        orchestration_runtime::execution_state::ExecutionStopReason::WaitingHuman(wait) => {
            Ok(&wait.node_id)
        }
        orchestration_runtime::execution_state::ExecutionStopReason::WaitingCallback(wait) => {
            Ok(&wait.node_id)
        }
        _ => Err(anyhow!(
            "waiting Answer Presentation requires a waiting execution outcome"
        )),
    }
}

async fn append_missing_answer_presentation_events<R>(
    repository: &R,
    flow_run_id: Uuid,
    events: Vec<crate::ports::RuntimeEventPayload>,
) -> Result<Vec<crate::ports::RuntimeEventPayload>>
where
    R: OrchestrationRuntimeRepository,
{
    let existing_text =
        existing_answer_presentation_text(repository, flow_run_id, "text_delta").await?;
    let existing_reasoning =
        existing_answer_presentation_text(repository, flow_run_id, "reasoning_delta").await?;
    let candidate_text = answer_presentation_event_text(&events, "text_delta");
    let candidate_reasoning = answer_presentation_event_text(&events, "reasoning_delta");
    let mut skip_text_bytes = if candidate_text.starts_with(&existing_text) {
        existing_text.len()
    } else {
        0
    };
    let mut skip_reasoning_bytes = if candidate_reasoning.starts_with(&existing_reasoning) {
        existing_reasoning.len()
    } else {
        0
    };
    let mut appended = Vec::new();

    for mut event in events {
        let Some(text) = event.payload.get("text").and_then(Value::as_str) else {
            continue;
        };
        let skip_bytes = match event.event_type.as_str() {
            "text_delta" => &mut skip_text_bytes,
            "reasoning_delta" => &mut skip_reasoning_bytes,
            _ => continue,
        };
        let missing = missing_answer_delta_text(skip_bytes, text);
        if missing.is_empty() {
            continue;
        }
        if let Some(payload) = event.payload.as_object_mut() {
            payload.insert("text".to_string(), Value::String(missing));
        }
        runtime_event_persister::persist_runtime_event_payload(repository, flow_run_id, &event)
            .await?;
        appended.push(event);
    }

    Ok(appended)
}

fn answer_presentation_event_text(
    events: &[crate::ports::RuntimeEventPayload],
    event_type: &str,
) -> String {
    events
        .iter()
        .filter(|event| event.event_type == event_type)
        .filter_map(|event| event.payload.get("text").and_then(Value::as_str))
        .collect()
}

async fn existing_answer_presentation_text<R>(
    repository: &R,
    flow_run_id: Uuid,
    event_type: &str,
) -> Result<String>
where
    R: OrchestrationRuntimeRepository,
{
    Ok(repository
        .list_runtime_events(flow_run_id, 0)
        .await?
        .into_iter()
        .filter(|event| event.event_type == event_type)
        .filter(|event| debug_stream_events::is_answer_presentation_delta_payload(&event.payload))
        .filter_map(|event| {
            event
                .payload
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<String>())
}

fn missing_answer_delta_text(skip_bytes: &mut usize, next_delta: &str) -> String {
    if *skip_bytes >= next_delta.len() {
        *skip_bytes -= next_delta.len();
        return String::new();
    }
    if *skip_bytes == 0 {
        return next_delta.to_string();
    }
    let missing = next_delta
        .get(*skip_bytes..)
        .unwrap_or(next_delta)
        .to_string();
    *skip_bytes = 0;
    missing
}

pub(super) fn answer_node_id(
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> &str {
    outcome
        .node_traces
        .iter()
        .rev()
        .find(|trace| trace.node_type == "answer")
        .map(|trace| trace.node_id.as_str())
        .unwrap_or("assistant")
}

pub(super) fn final_flow_output_payload(
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> Value {
    if matches!(
        outcome.stop_reason,
        orchestration_runtime::execution_state::ExecutionStopReason::Failed(_)
    ) {
        if let Some(answer_payload) = outcome
            .node_traces
            .iter()
            .rev()
            .find(|trace| trace.node_type == "answer" && !is_empty_object(&trace.output_payload))
            .map(|trace| trace.output_payload.clone())
        {
            return answer_payload;
        }

        return outcome
            .node_traces
            .iter()
            .rev()
            .find(|trace| trace.error_payload.is_none() && !is_empty_object(&trace.output_payload))
            .map(|trace| trace.output_payload.clone())
            .unwrap_or_else(|| json!({}));
    }

    outcome
        .node_traces
        .last()
        .map(|trace| trace.output_payload.clone())
        .unwrap_or_else(|| json!({}))
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(|object| object.is_empty())
}
