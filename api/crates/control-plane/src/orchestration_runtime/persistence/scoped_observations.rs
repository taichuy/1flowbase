use super::scoped_observation_writer::ScopedObservationWriter;
use super::*;
use crate::runtime_observability::RuntimeObservationRepository;

pub(in crate::orchestration_runtime) struct ObservationContext<'a> {
    pub scope_id: Uuid,
    pub application_name: &'a str,
    pub task_queue: Option<&'a Arc<dyn crate::ports::TaskQueue>>,
    pub flow_run: &'a domain::FlowRunRecord,
    pub conversation_id: Option<&'a str>,
    pub flow_span_id: Option<Uuid>,
}

/// Node terminal state and this trace were committed together. The bounded
/// cursor reads only pending payloads; every downstream write has a stable ID,
/// so interruption at any await can be resumed without duplicate facts.
pub(in crate::orchestration_runtime) async fn drain_pending<R: OrchestrationRuntimeRepository>(
    repository: &R,
    context: ObservationContext<'_>,
) -> Result<()> {
    let mut after_id = None;
    let mut conversation_id_owned = None;
    let mut looked_up_conversation = false;
    loop {
        let pending = repository
            .list_pending_container_observations(context.flow_run.id, after_id, 1)
            .await?;
        if pending.is_empty() {
            break;
        }
        if context.conversation_id.is_none() && !looked_up_conversation {
            conversation_id_owned = repository
                .get_flow_run_assistant_conversation_id(context.flow_run.id)
                .await?
                .map(|id| id.to_string());
            looked_up_conversation = true;
        }
        for observation in pending {
            let trace: orchestration_runtime::execution_state::NodeExecutionTrace =
                serde_json::from_value(observation.trace_payload)?;
            let writer = ScopedObservationWriter::new(repository, observation.id);
            let span=append_host_span(&writer,AppendHostSpanInput {
                flow_run_id:context.flow_run.id,node_run_id:Some(observation.node_run_id),parent_span_id:context.flow_span_id,
                kind:if trace.node_type=="llm" {domain::RuntimeSpanKind::LlmTurn} else {domain::RuntimeSpanKind::Node},
                name:trace.node_alias.clone(),started_at:node_traces::trace_started_at(&trace).unwrap_or(observation.started_at),
                metadata:json!({"node_id":trace.node_id,"node_type":trace.node_type,"execution_identity":trace.execution_identity,"native_response":trace.output_payload.pointer("/provider_metadata/native_response")}),
            }).await?;
            let mut refs_payload = json!({});
            if trace.node_type == "llm" {
                let refs = persist_llm_context_observability(
                    &writer,
                    context.scope_id,
                    context.application_name,
                    context.task_queue,
                    context.flow_run.id,
                    context.flow_run.created_by,
                    context.flow_run.authorized_account.as_deref(),
                    context.flow_run.application_id,
                    context.conversation_id.or(conversation_id_owned.as_deref()),
                    observation.node_run_id,
                    span.id,
                    &trace,
                )
                .await?;
                apply_llm_debug_observability_refs(&mut refs_payload, &refs);
            }
            append_provider_stream_events(
                &writer,
                context.flow_run.id,
                Some(observation.node_run_id),
                Some(span.id),
                &trace.provider_events,
            )
            .await?;
            if let Some(events) = trace
                .debug_payload
                .get("visible_internal_llm_tool_events")
                .and_then(Value::as_array)
            {
                for event in events {
                    let event = debug_stream_events::visible_internal_llm_tool_route(
                        context.flow_run.id,
                        observation.node_run_id,
                        &trace.node_id,
                        event,
                    );
                    if event.persist_required {
                        writer
                            .append_runtime_event(
                                &runtime_event_persister::build_runtime_event_input(
                                    context.flow_run.id,
                                    Some(observation.node_run_id),
                                    event.event_type,
                                    event.source,
                                    event.payload,
                                ),
                            )
                            .await?;
                    }
                }
            }
            repository
                .finish_container_observation(observation.id, &refs_payload)
                .await?;
            after_id = Some(observation.id);
        }
    }
    Ok(())
}
