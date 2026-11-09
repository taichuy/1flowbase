use super::{authorized_actor, EventWorkflowStatuses, Snapshot};
use crate::{
    app_state::ApiState,
    routes::{
        application_public_api::native::api_provider_runtime,
        mcp_protocol::virtual_ui::ApiMcpRuntimeToolInvoker,
    },
    runtime_activity::{scope_application_activity, ApplicationActivityKind},
};
use axum::http::HeaderMap;
use control_plane::{
    application_public_api::workflow_event::{
        InvokeWorkflowEventCommand, WorkflowEventInvocationOutcome, WorkflowEventTriggerService,
    },
    event_center::{EventCenterDelivery, EventCenterSignalBody},
    orchestration_runtime::{OrchestrationRuntimeService, StartPublishedFlowRunCommand},
};
use std::sync::Arc;

pub(super) enum DeliveryOutcome {
    Processed,
    Retire,
}

/// Read current authority and target only after non-consuming queue readiness. The worker
/// follows this with synchronous try_recv, so an idle wait cannot freeze this decision.
/// invoke_event still validates again before durable writes after ownership has transferred.
pub(super) async fn validate_handoff(state: &ApiState, snapshot: &Snapshot) -> anyhow::Result<()> {
    use control_plane_contracts::{
        ports::EventCenterRepository, ControlPlaneContractError as Error,
    };
    let actor = authorized_actor(state, &snapshot.trigger).await?;
    let entries = state
        .event_sources
        .registered_events(&actor, &state.event_center);
    let store = state.store.for_actor(actor);
    let current = store
        .get_workflow_event_trigger(
            snapshot.trigger.workspace_id,
            snapshot.trigger.application_id,
        )
        .await?
        .ok_or(Error::Conflict("trigger_removed"))?;
    if current != snapshot.trigger {
        return Err(Error::Conflict("trigger_changed").into());
    }
    let publication = WorkflowEventTriggerService::new(store)
        .with_registered_events(entries)
        .prepare_subscription(&current)
        .await?;
    if publication != snapshot.publication {
        return Err(Error::Conflict("publication_changed").into());
    }
    Ok(())
}

/// The caller retains the EventCenterDelivery until this future returns, including projection,
/// invocation and synchronous execution. Ordinary run callbacks retain their existing run owner.
pub(super) async fn execute_delivery(
    state: &Arc<ApiState>,
    snapshot: &Snapshot,
    delivery: &EventCenterDelivery,
    statuses: &EventWorkflowStatuses,
) -> anyhow::Result<DeliveryOutcome> {
    let signal = delivery.signal();
    let event_id = match &signal.body {
        EventCenterSignalBody::Published { .. } => match &signal.source_context {
            Some(source) => source.original_event_id.clone(),
            None => Some(signal.event_id.to_string()),
        },
        EventCenterSignalBody::Subscribed { .. } => None,
        EventCenterSignalBody::Consumed {
            source_event_id, ..
        } => Some(source_event_id.clone()),
    };
    let actor = authorized_actor(state, &snapshot.trigger).await?;
    let entries = state
        .event_sources
        .registered_events(&actor, &state.event_center);
    let actor_store = state
        .store
        .clone()
        .with_workflow_ancestor_ids(signal.workflow_ancestor_ids.clone())
        .for_actor(actor.clone());
    let invoked = WorkflowEventTriggerService::new(actor_store.clone())
        .with_registered_events(entries)
        .invoke_event(InvokeWorkflowEventCommand {
            trigger: snapshot.trigger.clone(),
            publication: snapshot.publication.clone(),
            signal_id: signal.event_id,
            event_id,
            subscription_id: snapshot.trigger.id,
            context: serde_json::to_value(signal)?,
        })
        .await?;
    let invoked = match invoked {
        WorkflowEventInvocationOutcome::Skipped { reason } => {
            statuses.delivery(
                snapshot.trigger.application_id,
                signal.event_id,
                None,
                "skipped",
                Some(reason),
            );
            state.event_workflow_changes.notify_one();
            return Ok(DeliveryOutcome::Retire);
        }
        WorkflowEventInvocationOutcome::Invoked(invoked) => invoked,
    };
    let run_id = invoked.flow_run.id;
    if !invoked.created {
        statuses.delivery(
            snapshot.trigger.application_id,
            signal.event_id,
            Some(run_id),
            "duplicate",
            None,
        );
        return Ok(DeliveryOutcome::Processed);
    }
    statuses.delivery(
        snapshot.trigger.application_id,
        signal.event_id,
        Some(run_id),
        "executing",
        None,
    );
    let source_binding =
        control_plane::ports::RuntimeEventSourceBinding::workspace(snapshot.trigger.workspace_id)
            .with_workflow_ancestors(signal.workflow_ancestor_ids.clone());
    // Propagate the explicit store clone into tools and services before their owned spawns.
    let causal_state = Arc::new(ApiState {
        store: actor_store.clone(),
        ..(**state).clone()
    });
    let execution = control_plane_contracts::ports::with_lifecycle_workflow_ancestors(
        signal.workflow_ancestor_ids.clone(),
        async {
            let mcp_invoker = Arc::new(
                ApiMcpRuntimeToolInvoker::new(
                    crate::runtime_internal_tool_invoker_factory(&causal_state, &actor)
                        .await
                        .map_err(|error| error.0)?,
                    HeaderMap::new(),
                    actor,
                    Vec::new(),
                )
                .await
                .map_err(|error| error.0)?,
            );
            state.runtime_event_stream.open_run_scoped(run_id, control_plane::ports::RuntimeEventStreamPolicy::debug_default(), source_binding.clone()).await?;
            let persister = control_plane::orchestration_runtime::start_runtime_debug_event_persister(actor_store.clone(), state.runtime_event_stream.clone(), run_id).await;
            let runtime_service = OrchestrationRuntimeService::new(
                actor_store,
                api_provider_runtime(state),
                state.runtime_engine.clone(),
                state.provider_secret_master_key.clone(),
                state.infrastructure.provider_transport_store(),
                state.model_billing_require_provider_usage,
            )
            .with_published_plan_cache(state.infrastructure.published_plan_cache())
            .with_node_artifact_context(
                state.api_node_id.clone(),
                state.provider_install_root.clone(),
            )
            .with_file_storage_registry(state.file_storage_registry.clone())
            .with_notification_sender(state.notification_service.clone())
            .with_runtime_internal_tool_invoker(mcp_invoker)
            .with_llm_routing_counter_store(state.infrastructure.cache_store())
            .with_provider_request_log_queue(state.infrastructure.task_queue())
            .with_runtime_event_stream(state.runtime_event_stream.clone())
            .with_runtime_event_source_binding(source_binding);
            let _activity = state.runtime_activity.start(
                snapshot.trigger.application_id,
                ApplicationActivityKind::ApplicationExecution,
            );
            let result = scope_application_activity(
                snapshot.trigger.application_id,
                runtime_service.start_published_flow_run(StartPublishedFlowRunCommand {
                    transport_connection_scope: None,
                    observation_context: None,
                    application_id: snapshot.trigger.application_id,
                    flow_run_id: run_id,
                    provider_transport_slot: None,
                }),
            )
            .await;
            match &result {
                Ok(detail) => control_plane::orchestration_runtime::project_runtime_event_stream_terminal(state.runtime_event_stream.clone(), &detail.flow_run).await,
                Err(_) => {
                    if let Ok(Some(winner)) = control_plane::ports::OrchestrationRuntimeRepository::get_flow_run(&causal_state.store, snapshot.trigger.application_id, run_id).await {
                        control_plane::orchestration_runtime::project_runtime_event_stream_terminal(state.runtime_event_stream.clone(), &winner).await;
                    }
                }
            }
            control_plane::orchestration_runtime::wait_for_runtime_debug_event_persister(persister, snapshot.trigger.application_id, run_id).await;
            result
        },
    )
    .await;
    match execution {
        Ok(detail) => {
            statuses.delivery(
                snapshot.trigger.application_id,
                signal.event_id,
                Some(run_id),
                detail.flow_run.status.as_str(),
                None,
            );
            Ok(DeliveryOutcome::Processed)
        }
        Err(error) => {
            statuses.delivery(
                snapshot.trigger.application_id,
                signal.event_id,
                Some(run_id),
                "execution_failed",
                Some(super::status::public_error_code(&error, "consumer_failed")),
            );
            Err(error)
        }
    }
}
