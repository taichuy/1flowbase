use super::*;
use std::sync::Arc;

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
    base_started_at: OffsetDateTime,
) -> Result<Option<domain::NodeRunRecord>>
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
        orchestration_runtime::execution_state::ExecutionStopReason::Completed => None,
    };
    let mut waiting_node_run = None;

    for (index, trace) in outcome.node_traces.iter().enumerate() {
        let started_at = base_started_at + Duration::seconds(index as i64);
        let node_run = repository
            .create_node_run(&CreateNodeRunInput {
                flow_run_id,
                node_id: trace.node_id.clone(),
                node_type: trace.node_type.clone(),
                node_alias: trace.node_alias.clone(),
                status: domain::NodeRunStatus::Running,
                input_payload: trace.input_payload.clone(),
                debug_payload: json!({}),
                started_at,
            })
            .await?;
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
        let (status, finished_at) = match waiting_node_id {
            Some((waiting_id, waiting_status)) if waiting_id == trace.node_id => {
                if waiting_status == domain::NodeRunStatus::Failed {
                    (waiting_status, Some(started_at))
                } else {
                    (waiting_status, None)
                }
            }
            _ => (domain::NodeRunStatus::Succeeded, Some(started_at)),
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

    Ok(waiting_node_run)
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
