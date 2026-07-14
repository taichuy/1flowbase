mod continuation;
mod observability;
mod plan;
mod preparation;
mod run_detail;
mod runtime_events;

use anyhow::Result;

use super::{
    ApplicationRunContext, CancelFlowRunCommand, ContinueFlowDebugRunCommand,
    LiveProviderStreamEventSender, OrchestrationRuntimeService, PrepareFlowDebugRunCommand,
    StartFlowDebugRunCommand,
};
use observability::{persist_llm_context_observability, run_live_event_persister};
use plan::{first_output_key, next_node_index};
use run_detail::{fail_flow_run, is_run_cancelled, load_run_detail};
use runtime_events::{
    append_runtime_event, close_runtime_event_stream, emit_flow_failed_and_close,
    update_node_run_and_emit,
};

pub(super) async fn start_flow_debug_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: StartFlowDebugRunCommand,
    context: &ApplicationRunContext,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::ApplicationJsDependencySelectionRepository
        + crate::ports::FlowRepository
        + crate::ports::OrchestrationRuntimeRepository
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
    preparation::start_flow_debug_run(service, command, context).await
}

pub(super) async fn open_flow_debug_run_shell<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: StartFlowDebugRunCommand,
    context: &ApplicationRunContext,
) -> Result<domain::FlowRunRecord>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FlowRepository
        + crate::ports::OrchestrationRuntimeRepository
        + Clone
        + Send
        + Sync
        + 'static,
    H: crate::ports::ProviderRuntimePort
        + crate::capability_plugin_runtime::CapabilityPluginRuntimePort
        + Clone,
{
    preparation::open_flow_debug_run_shell(service, command, context).await
}

pub(super) async fn prepare_flow_debug_run_from_shell<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: PrepareFlowDebugRunCommand,
    context: &ApplicationRunContext,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::ApplicationJsDependencySelectionRepository
        + crate::ports::FlowRepository
        + crate::ports::OrchestrationRuntimeRepository
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
    preparation::prepare_flow_debug_run_from_shell(service, command, context).await
}

pub(super) async fn continue_flow_debug_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: ContinueFlowDebugRunCommand,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + crate::ports::OrchestrationRuntimeRepository
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
    continuation::continue_flow_debug_run(service, command).await
}

pub(super) async fn continue_flow_debug_run_with_live_provider_events<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: ContinueFlowDebugRunCommand,
    live_provider_events: LiveProviderStreamEventSender,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FileManagementRepository
        + crate::ports::FlowRepository
        + crate::ports::OrchestrationRuntimeRepository
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
    continuation::continue_flow_debug_run_with_live_provider_events(
        service,
        command,
        live_provider_events,
    )
    .await
}

pub(super) async fn cancel_flow_run<R, H>(
    service: &OrchestrationRuntimeService<R, H>,
    command: CancelFlowRunCommand,
    context: &ApplicationRunContext,
) -> Result<domain::ApplicationRunDetail>
where
    R: crate::ports::ApplicationRepository
        + crate::ports::FlowRepository
        + crate::ports::OrchestrationRuntimeRepository
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
    continuation::cancel_flow_run(service, command, context).await
}
