use super::*;
use crate::orchestration_runtime::{
    answer_presentation,
    provider_invoker::{CanonicalProviderDelta, RuntimeCanonicalStreamWriter},
};
use serde_json::Map;
use std::collections::BTreeMap;

pub(super) fn ready_waiting_answer_output_payload(
    compiled_plan: Option<&orchestration_runtime::compiled_plan::CompiledPlan>,
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> Result<Option<Value>> {
    let Some(compiled_plan) = compiled_plan else {
        return Ok(None);
    };
    let checkpoint = outcome
        .checkpoint_snapshot
        .as_ref()
        .ok_or_else(|| anyhow!("waiting Answer Presentation is missing checkpoint state"))?;
    let projection = canonical_flow_projection(outcome, &checkpoint.variable_pool)?;
    let variable_pool = &projection.variable_pool;
    let waiting_node_id = waiting_node_id(outcome)?;
    let Some(ready) = answer_presentation::ready_waiting_answer_output_from_variable_pool(
        compiled_plan,
        variable_pool,
        &checkpoint.active_node_ids,
        waiting_node_id,
    ) else {
        return Ok(None);
    };
    let output_payload = answer_presentation::ready_answer_output_payload(&ready, variable_pool);
    if ready.text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(output_payload))
    }
}

pub(super) fn answer_presentation_terminal_events(
    compiled_plan: Option<&orchestration_runtime::compiled_plan::CompiledPlan>,
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    prepared_node_runs: Option<&PreparedNodeRuns>,
    answer_node_id: &str,
    output_payload: &Value,
) -> Result<Vec<crate::ports::RuntimeEventPayload>> {
    if let Some(compiled_plan) = compiled_plan {
        let projection = canonical_flow_projection(outcome, &outcome.variable_pool)?;
        if let Some(events) = presentation_events_from_canonical_pool(
            compiled_plan,
            &projection,
            prepared_node_runs,
            answer_node_id,
        ) {
            if !events.is_empty() {
                return Ok(events);
            }
        }
    }

    let Some(answer) = output_payload.get("answer").and_then(Value::as_str) else {
        return Ok(Vec::new());
    };
    let mut writer = RuntimeCanonicalStreamWriter::new(answer_node_id);
    writer.write(&ProviderStreamEvent::TextDelta {
        delta: answer.to_string(),
    })?;
    writer.complete()?;
    let text = writer.state().accumulated().text().as_str();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![debug_stream_events::answer_text_delta(
        answer_node_id,
        text.to_string(),
        0,
        None,
        None,
        None,
    )])
}

pub(super) fn canonical_terminal_output_payload(
    compiled_plan: Option<&orchestration_runtime::compiled_plan::CompiledPlan>,
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
) -> Result<Value> {
    let mut output_payload = final_flow_output_payload(outcome);
    if outcome.operation_terminal.is_some() {
        return Ok(output_payload);
    }
    let events = answer_presentation_terminal_events(
        compiled_plan,
        outcome,
        None,
        answer_node_id(outcome),
        &output_payload,
    )?;
    let answer = events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| debug_stream_events::is_answer_presentation_delta_payload(&event.payload))
        .filter_map(|event| event.payload.get("text").and_then(Value::as_str))
        .collect::<String>();
    if !answer.is_empty() {
        answer_presentation::mark_canonical_answer_presentation_output(&mut output_payload);
        if let Some(output) = output_payload.as_object_mut() {
            output.insert("answer".to_string(), Value::String(answer));
        }
    }
    Ok(output_payload)
}

fn presentation_events_from_canonical_pool(
    compiled_plan: &orchestration_runtime::compiled_plan::CompiledPlan,
    projection: &CanonicalFlowProjection,
    prepared_node_runs: Option<&PreparedNodeRuns>,
    answer_node_id: &str,
) -> Option<Vec<crate::ports::RuntimeEventPayload>> {
    let presentation =
        orchestration_runtime::answer_presentation::AnswerPresentationPlan::candidates_from_plan(
            compiled_plan,
        )
        .into_iter()
        .find(|presentation| presentation.answer_node_id == answer_node_id)?;
    let mut cursor = answer_presentation::AnswerPresentationCursor::from_presentation(presentation);
    let mut events = Vec::new();

    for node_id in &compiled_plan.topological_order {
        let Some(output_payload) = projection.variable_pool.get(node_id) else {
            continue;
        };
        let node_run_id = prepared_node_runs
            .and_then(|node_runs| node_runs.get(node_id))
            .map(|node_run| node_run.id);
        let belongs_to_current_segment =
            prepared_node_runs.is_none_or(|node_runs| node_runs.contains_key(node_id));
        if belongs_to_current_segment {
            if let Some(deltas) = projection.deltas_by_node.get(node_id) {
                let source_node_run_id = node_run_id.unwrap_or_else(Uuid::nil);
                for delta in deltas {
                    let event = match delta.kind {
                        super::super::canonical_stream::CanonicalContentKind::Text => {
                            ProviderStreamEvent::TextDelta {
                                delta: delta.text.clone(),
                            }
                        }
                        super::super::canonical_stream::CanonicalContentKind::Reasoning => {
                            ProviderStreamEvent::ReasoningDelta {
                                delta: delta.text.clone(),
                            }
                        }
                    };
                    events.extend(cursor.push_provider_event(node_id, source_node_run_id, &event));
                }
            }
            events.extend(cursor.complete_node_with_run_id(node_id, node_run_id, output_payload));
        } else {
            let _ = cursor.complete_node_with_run_id(node_id, node_run_id, output_payload);
        }
    }

    Some(events)
}

struct CanonicalFlowProjection {
    variable_pool: Map<String, Value>,
    deltas_by_node: BTreeMap<String, Vec<CanonicalProviderDelta>>,
}

fn canonical_flow_projection(
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    base: &Map<String, Value>,
) -> Result<CanonicalFlowProjection> {
    let mut variable_pool = base.clone();
    let mut deltas_by_node = BTreeMap::new();
    for trace in &outcome.node_traces {
        if trace.node_type != "llm" {
            continue;
        }
        let (text, deltas) = canonical_trace_projection(trace)?;
        deltas_by_node.insert(trace.node_id.clone(), deltas);
        let Some(text) = text else {
            continue;
        };
        let output_payload = variable_pool
            .entry(trace.node_id.clone())
            .or_insert_with(|| trace.output_payload.clone());
        if let Some(output) = output_payload.as_object_mut() {
            output.insert("text".to_string(), Value::String(text));
        }
    }
    Ok(CanonicalFlowProjection {
        variable_pool,
        deltas_by_node,
    })
}

fn canonical_trace_projection(
    trace: &orchestration_runtime::execution_state::NodeExecutionTrace,
) -> Result<(Option<String>, Vec<CanonicalProviderDelta>)> {
    let mut writer = RuntimeCanonicalStreamWriter::new(trace.node_id.clone());
    let mut deltas = Vec::new();
    let has_content_event = trace.provider_events.iter().any(|event| {
        matches!(
            event,
            ProviderStreamEvent::TextDelta { .. } | ProviderStreamEvent::ReasoningDelta { .. }
        )
    });
    if !has_content_event {
        if let Some(text) = trace.output_payload.get("text").and_then(Value::as_str) {
            deltas.extend(writer.write(&ProviderStreamEvent::TextDelta {
                delta: text.to_string(),
            })?);
        }
    }
    for event in &trace.provider_events {
        deltas.extend(writer.write(event)?);
    }
    deltas.extend(writer.complete()?);
    let text = writer.state().accumulated().text().as_str();
    Ok(((!text.is_empty()).then(|| text.to_string()), deltas))
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

    if let Some(terminal) = &outcome.operation_terminal {
        return terminal
            .as_payload()
            .expect("runtime-owned Native operation terminal must serialize");
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
