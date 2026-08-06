use super::*;
use std::sync::Arc;
use time::format_description::well_known::Rfc3339;

pub(in crate::orchestration_runtime) type PreparedNodeRuns =
    std::collections::BTreeMap<String, domain::NodeRunRecord>;

pub(super) struct PersistedNodeTraces {
    pub(super) waiting_node_run: Option<domain::NodeRunRecord>,
    pub(super) stream_events: Vec<crate::ports::RuntimeEventPayload>,
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn persist_flow_debug_node_traces<R>(
    repository: &R,
    scope_id: Uuid,
    application_name: &str,
    application_id: Uuid,
    conversation_id: Option<&str>,
    task_queue: Option<&Arc<dyn crate::ports::TaskQueue>>,
    flow_run_id: Uuid,
    flow_span_id: Option<Uuid>,
    outcome: &orchestration_runtime::execution_state::FlowDebugExecutionOutcome,
    prepared_node_runs: Option<&PreparedNodeRuns>,
    base_started_at: OffsetDateTime,
) -> Result<PersistedNodeTraces>
where
    R: OrchestrationRuntimeRepository,
{
    let waiting_node_id = match &outcome.stop_reason {
        orchestration_runtime::execution_state::ExecutionStopReason::WaitingHuman(wait) => {
            Some((wait.node_id.as_str(), domain::NodeRunStatus::WaitingHuman))
        }
        orchestration_runtime::execution_state::ExecutionStopReason::WaitingCallback(wait) => {
            Some((
                wait.node_id.as_str(),
                domain::NodeRunStatus::WaitingCallback,
            ))
        }
        orchestration_runtime::execution_state::ExecutionStopReason::Failed(failure) => {
            Some((failure.node_id.as_str(), domain::NodeRunStatus::Failed))
        }
        orchestration_runtime::execution_state::ExecutionStopReason::Completed
        | orchestration_runtime::execution_state::ExecutionStopReason::Incomplete(_) => None,
    };
    let mut waiting_node_run = None;
    let mut stream_events = Vec::new();

    for (index, trace) in outcome.node_traces.iter().enumerate() {
        let fallback_started_at = base_started_at + Duration::seconds(index as i64);
        let node_run = if let Some(node_run) = prepared_node_runs
            .and_then(|node_runs| node_runs.get(&trace.node_id))
            .cloned()
        {
            node_run
        } else {
            repository
                .create_node_run(&CreateNodeRunInput {
                    flow_run_id,
                    node_id: trace.node_id.clone(),
                    node_type: trace.node_type.clone(),
                    node_alias: trace.node_alias.clone(),
                    status: domain::NodeRunStatus::Running,
                    input_payload: trace.input_payload.clone(),
                    debug_payload: json!({}),
                    started_at: fallback_started_at,
                })
                .await?
        };
        let started_at = node_run.started_at;
        let span_kind = if trace.node_type == "llm" {
            domain::RuntimeSpanKind::LlmTurn
        } else {
            domain::RuntimeSpanKind::Node
        };
        let node_span = append_host_span(
            repository,
            AppendHostSpanInput {
                flow_run_id,
                node_run_id: Some(node_run.id),
                parent_span_id: flow_span_id,
                kind: span_kind,
                name: trace.node_alias.clone(),
                started_at,
                metadata: json!({
                    "node_id": trace.node_id,
                    "node_type": trace.node_type,
                }),
            },
        )
        .await?;
        let next_node_started_at = outcome
            .node_traces
            .get(index + 1)
            .and_then(|next_trace| prepared_node_runs?.get(&next_trace.node_id))
            .map(|node_run| node_run.started_at);
        let completed_at = trace_finished_at(trace)
            .or(next_node_started_at)
            .unwrap_or_else(OffsetDateTime::now_utc)
            .max(started_at);
        let (status, finished_at) = match waiting_node_id {
            Some((waiting_id, waiting_status)) if waiting_id == trace.node_id => {
                if waiting_status == domain::NodeRunStatus::Failed {
                    (waiting_status, Some(completed_at))
                } else {
                    (waiting_status, None)
                }
            }
            _ => (domain::NodeRunStatus::Succeeded, Some(completed_at)),
        };
        ensure_node_run_transition(
            domain::NodeRunStatus::Running,
            status,
            "persist_flow_debug_node_trace",
        )?;
        let mut debug_payload = trace.debug_payload.clone();
        if trace.node_type == "llm" {
            let refs = persist_llm_context_observability(
                repository,
                scope_id,
                application_name,
                task_queue,
                flow_run_id,
                application_id,
                conversation_id,
                node_run.id,
                node_span.id,
                trace,
            )
            .await?;
            apply_llm_debug_observability_refs(&mut debug_payload, &refs);
        }
        let node_run = repository
            .update_node_run(&UpdateNodeRunInput {
                node_run_id: node_run.id,
                status,
                output_payload: persisted_node_output_payload(
                    &trace.output_payload,
                    &trace.metrics_payload,
                    trace.error_payload.as_ref(),
                    &trace.debug_payload,
                ),
                error_payload: trace.error_payload.clone(),
                metrics_payload: trace.metrics_payload.clone(),
                debug_payload,
                finished_at,
            })
            .await?;
        let node_finished_event = debug_stream_events::node_finished(&node_run);
        runtime_event_persister::persist_runtime_event_payload(
            repository,
            flow_run_id,
            &node_finished_event,
        )
        .await?;
        stream_events.push(node_finished_event);
        append_provider_stream_events(
            repository,
            flow_run_id,
            Some(node_run.id),
            Some(node_span.id),
            &trace.provider_events,
        )
        .await?;
        persist_visible_internal_llm_tool_route_events(
            repository,
            flow_run_id,
            node_run.id,
            &trace.node_id,
            &trace.debug_payload,
        )
        .await?;

        if finished_at.is_none() && status != domain::NodeRunStatus::Failed {
            waiting_node_run = Some(node_run);
        }
    }

    Ok(PersistedNodeTraces {
        waiting_node_run,
        stream_events,
    })
}

fn trace_finished_at(
    trace: &orchestration_runtime::execution_state::NodeExecutionTrace,
) -> Option<OffsetDateTime> {
    trace
        .metrics_payload
        .get("attempts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|attempt| {
            attempt
                .get("finished_at")
                .and_then(Value::as_str)
                .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_trace_uses_the_last_real_attempt_finish_time() {
        let trace = orchestration_runtime::execution_state::NodeExecutionTrace {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            input_payload: json!({}),
            output_payload: json!({}),
            error_payload: None,
            metrics_payload: json!({
                "attempts": [
                    { "finished_at": "2026-08-06T10:00:04Z" },
                    { "finished_at": "2026-08-06T10:00:09Z" }
                ]
            }),
            debug_payload: json!({}),
            provider_events: Vec::new(),
        };

        assert_eq!(
            trace_finished_at(&trace),
            Some(OffsetDateTime::parse("2026-08-06T10:00:09Z", &Rfc3339).unwrap())
        );
    }
}

async fn persist_visible_internal_llm_tool_route_events<R>(
    repository: &R,
    flow_run_id: Uuid,
    node_run_id: Uuid,
    node_id: &str,
    debug_payload: &Value,
) -> Result<()>
where
    R: OrchestrationRuntimeRepository,
{
    let Some(route_events) = debug_payload
        .get("visible_internal_llm_tool_events")
        .and_then(Value::as_array)
    else {
        return Ok(());
    };

    for route_event in route_events {
        runtime_event_persister::persist_runtime_event_payload(
            repository,
            flow_run_id,
            &debug_stream_events::visible_internal_llm_tool_route(
                flow_run_id,
                node_run_id,
                node_id,
                route_event,
            ),
        )
        .await?;
    }

    Ok(())
}
