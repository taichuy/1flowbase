//! Concrete execution entry points shared by HTTP domains and host workers.
//! Keep execution monomorphization at the host type owner; callers retain service lifecycle.

use control_plane::orchestration_runtime::{
    CompleteCallbackTaskCommand, ContinueFlowDebugRunCommand, OrchestrationRuntimeService,
    ResumeFlowRunCommand, StartFlowDebugRunCommand, StartNodeDebugPreviewCommand,
    StartPublishedFlowRunCommand,
};
use storage_durable_postgres::MainDurableStore;

use super::provider_runtime::ApiProviderRuntime;

type RuntimeService = OrchestrationRuntimeService<MainDurableStore, ApiProviderRuntime>;

pub fn start_published_flow_run(
    service: &RuntimeService,
    command: StartPublishedFlowRunCommand,
) -> impl std::future::Future<Output = anyhow::Result<domain::ApplicationRunDetail>> + '_ {
    let future = service.start_published_flow_run(command);
    async move { future.await }
}

pub async fn start_flow_debug_run(
    service: &RuntimeService,
    command: StartFlowDebugRunCommand,
) -> anyhow::Result<domain::ApplicationRunDetail> {
    service.start_flow_debug_run(command).await
}

pub async fn continue_flow_debug_run(
    service: &RuntimeService,
    command: ContinueFlowDebugRunCommand,
) -> anyhow::Result<domain::ApplicationRunDetail> {
    service.continue_flow_debug_run(command).await
}

pub async fn resume_flow_run(
    service: &RuntimeService,
    command: ResumeFlowRunCommand,
) -> anyhow::Result<domain::ApplicationRunDetail> {
    service.resume_flow_run(command).await
}

pub async fn complete_callback_task(
    service: &RuntimeService,
    command: CompleteCallbackTaskCommand,
) -> anyhow::Result<domain::ApplicationRunDetail> {
    service.complete_callback_task(command).await
}

pub async fn start_node_debug_preview(
    service: &RuntimeService,
    command: StartNodeDebugPreviewCommand,
) -> anyhow::Result<domain::NodeDebugPreviewResult> {
    service.start_node_debug_preview(command).await
}

pub async fn dispatch_communication_run(
    service: &RuntimeService,
    command: StartPublishedFlowRunCommand,
) -> anyhow::Result<domain::ApplicationRunDetail> {
    service.dispatch_communication_run(command).await
}
