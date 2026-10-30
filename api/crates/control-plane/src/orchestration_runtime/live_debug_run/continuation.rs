use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use time::OffsetDateTime;

use crate::{
    errors::ControlPlaneError,
    ports::{
        CommitFlowRunTerminalInput, CommitFlowRunTerminalResult, OrchestrationRuntimeRepository,
    },
    state_transition::ensure_flow_run_transition,
};

use super::super::stream_terminal_recovery::{resolve_terminal_commit, TerminalCommitResolution};
use super::super::{
    debug_stream_events, ApplicationRunContext, CancelFlowRunCommand, ContinueFlowDebugRunCommand,
    LiveProviderStreamEventSender, OrchestrationRuntimeService,
};

mod engine;
mod helpers;
pub(in crate::orchestration_runtime) use helpers::{
    compiled_plan_start_node_id, inject_system_variables,
};
mod native_recovery;

use super::{fail_flow_run, load_run_detail, project_committed_terminal};
use engine::continue_flow_debug_run_inner;

pub(super) async fn continue_flow_debug_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: ContinueFlowDebugRunCommand,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::BillingRepository
        + crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    continue_flow_debug_run_with_optional_live_provider_events(
        service, command, None, None, None, None,
    )
    .await
}

pub(in crate::orchestration_runtime) async fn continue_flow_debug_run_with_provider_transport<
    R,
    H,
>(
    service: &OrchestrationRuntimeService<R, H>,
    command: ContinueFlowDebugRunCommand,
    provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    transport_connection_scope: Option<String>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::BillingRepository
        + crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    continue_flow_debug_run_with_optional_live_provider_events(
        service,
        command,
        None,
        provider_transport_payload,
        transport_connection_scope,
        observation_context,
    )
    .await
}

pub(super) async fn continue_flow_debug_run_with_live_provider_events<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: ContinueFlowDebugRunCommand,
    live_provider_events: LiveProviderStreamEventSender,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::BillingRepository
        + crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    continue_flow_debug_run_with_optional_live_provider_events(
        service,
        command,
        Some(live_provider_events),
        None,
        None,
        None,
    )
    .await
}

async fn continue_flow_debug_run_with_optional_live_provider_events<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: ContinueFlowDebugRunCommand,
    live_provider_events: Option<LiveProviderStreamEventSender>,
    provider_transport_payload: Option<crate::ports::ProviderTransportPayload>,
    transport_connection_scope: Option<String>,
    observation_context: Option<control_plane_contracts::ports::WorkflowObservationContext>,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::BillingRepository
        + crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    let flow_run = match service
        .repository
        .get_flow_run(command.application_id, command.flow_run_id)
        .await
    {
        Ok(Some(run)) => run,
        Ok(None) => return Err(anyhow!("flow run not found")),
        Err(error) => {
            // Preserve pre-execution read-failure cleanup. This retry supplies
            // payload metadata only: the atomic initial-execution fence below
            // cannot acquire a checkpoint/resume owner's failure authority.
            if let Ok(Some(initial)) = service
                .repository
                .get_flow_run(command.application_id, command.flow_run_id)
                .await
            {
                if initial.status == domain::FlowRunStatus::Running {
                    return fail_flow_run(service, &initial, &error)
                        .await
                        .or(Err(error));
                }
            }
            return Err(error);
        }
    };
    if flow_run.status != domain::FlowRunStatus::Running {
        if !service
            .repository
            .list_pending_container_observations(flow_run.id, None, 1)
            .await?
            .is_empty()
        {
            let application = service
                .repository
                .get_application(command.workspace_id, command.application_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("application"))?;
            crate::orchestration_runtime::persistence::scoped_observations::drain_pending(
                &service.repository,
                crate::orchestration_runtime::persistence::scoped_observations::ObservationContext {
                    scope_id:application.workspace_id,application_name:&application.name,
                    task_queue:service.provider_request_log_queue.as_ref(),flow_run:&flow_run,
                    conversation_id:flow_run.external_conversation_id.as_deref(),flow_span_id:None,
                },
            ).await?;
        }
        return load_run_detail(&service.repository, command.application_id, flow_run.id).await;
    }
    let result = continue_flow_debug_run_inner(
        service,
        &command,
        &flow_run,
        live_provider_events,
        provider_transport_payload,
        transport_connection_scope,
        observation_context,
    )
    .await;

    match result {
        Ok(detail) => Ok(detail),
        Err(error) => fail_flow_run(service, &flow_run, &error)
            .await
            .or(Err(error)),
    }
}

pub(super) async fn cancel_flow_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: CancelFlowRunCommand,
    context: &ApplicationRunContext,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    let flow_run = service
        .repository
        .get_flow_run(command.application_id, command.flow_run_id)
        .await?
        .ok_or_else(|| anyhow!("flow run not found"))?;
    ensure_flow_run_transition(
        flow_run.status,
        domain::FlowRunStatus::Cancelled,
        "cancel_flow_run",
    )?;
    if let Err(error) =
        crate::orchestration_runtime::persistence::scoped_observations::drain_pending(
            &service.repository,
            crate::orchestration_runtime::persistence::scoped_observations::ObservationContext {
                scope_id: context.application.workspace_id,
                application_name: &context.application.name,
                task_queue: service.provider_request_log_queue.as_ref(),
                flow_run: &flow_run,
                conversation_id: flow_run.external_conversation_id.as_deref(),
                flow_span_id: None,
            },
        )
        .await
    {
        // Cancellation retains authority even when a diagnostic sink is down.
        // The durable pending trace remains available for reconciliation.
        tracing::warn!(flow_run_id=%flow_run.id, %error, "scoped observations remain pending during cancellation");
    }
    let terminal_event = debug_stream_events::flow_cancelled(flow_run.id);
    let output_payload = cancellation_output_payload(service, &flow_run).await;
    let receipt = service
        .repository
        .commit_flow_run_terminal(&CommitFlowRunTerminalInput {
            flow_run_id: flow_run.id,
            expected_status: flow_run.status,
            require_initial_execution: false,
            result: CommitFlowRunTerminalResult::Cancelled {
                output_payload,
                error_payload: flow_run.error_payload.clone(),
            },
            flow_run_event_payload: json!({ "reason": "manual_stop" }),
            terminal_event_payload: terminal_event.payload,
            finished_at: OffsetDateTime::now_utc(),
        })
        .await?;
    let flow_run = match resolve_terminal_commit(
        &service.repository,
        command.application_id,
        flow_run.id,
        receipt,
    )
    .await?
    {
        TerminalCommitResolution::Winner(flow_run) | TerminalCommitResolution::Loser(flow_run) => {
            flow_run
        }
    };
    project_committed_terminal(service, &flow_run).await;

    load_run_detail(&service.repository, command.application_id, flow_run.id).await
}

async fn cancellation_output_payload<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    flow_run: &domain::FlowRunRecord,
) -> Value
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FlowRepository
        + OrchestrationRuntimeRepository
        + crate::ports::ModelDefinitionRepository
        + crate::ports::ModelProviderRepository
        + crate::ports::NodeContributionRepository
        + crate::ports::PluginRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    let empty_or_existing = || {
        if flow_run.run_mode == domain::FlowRunMode::AssistantExecution {
            json!({})
        } else {
            flow_run.output_payload.clone()
        }
    };
    let Some(stream) = &service.runtime_event_stream else {
        return empty_or_existing();
    };
    let Ok(events) = stream.replay(flow_run.id, None, usize::MAX).await else {
        return empty_or_existing();
    };
    let answer = events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| debug_stream_events::is_answer_presentation_delta_payload(&event.payload))
        .filter_map(|event| event.payload.get("text").and_then(Value::as_str))
        .collect::<String>();
    if answer.is_empty() {
        return empty_or_existing();
    }

    let mut output_payload = flow_run.output_payload.clone();
    if !output_payload.is_object() {
        output_payload = json!({});
    }
    super::super::answer_presentation::mark_canonical_answer_presentation_output(
        &mut output_payload,
    );
    if let Some(output) = output_payload.as_object_mut() {
        output.insert("answer".to_string(), Value::String(answer));
    }
    output_payload
}
