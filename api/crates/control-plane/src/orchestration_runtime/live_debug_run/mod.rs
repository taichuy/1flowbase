mod continuation;
mod preparation;
mod run_detail;
mod runtime_events;

use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use time::OffsetDateTime;

use crate::ports::{CreateNodeRunInput, OrchestrationRuntimeRepository};

use super::{
    debug_stream_events, ApplicationRunContext, CancelFlowRunCommand, ContinueFlowDebugRunCommand,
    LiveProviderStreamEventSender, OrchestrationRuntimeService, PrepareFlowDebugRunCommand,
    StartFlowDebugRunCommand,
};
use run_detail::{fail_flow_run, load_run_detail};
use runtime_events::{
    append_runtime_event, close_runtime_event_stream, emit_flow_failed_and_close,
};

use super::{persistence::PreparedNodeRuns, RuntimeActiveNode, RuntimeFlowExecutionContext};

pub(in crate::orchestration_runtime) struct PersistedNodeLifecycle<'a, R, H> {
    service: &'a OrchestrationRuntimeService<R, H>,
    flow_run_id: uuid::Uuid,
    prepared_node_runs: Arc<std::sync::Mutex<PreparedNodeRuns>>,
    flow_execution_context: Arc<RuntimeFlowExecutionContext>,
}

impl<'a, R, H> PersistedNodeLifecycle<'a, R, H> {
    pub(in crate::orchestration_runtime) fn new(
        service: &'a OrchestrationRuntimeService<R, H>,
        flow_run_id: uuid::Uuid,
        flow_execution_context: Arc<RuntimeFlowExecutionContext>,
    ) -> Self {
        Self {
            service,
            flow_run_id,
            prepared_node_runs: Arc::new(std::sync::Mutex::new(BTreeMap::new())),
            flow_execution_context,
        }
    }

    pub(in crate::orchestration_runtime) fn prepared_node_runs(&self) -> Result<PreparedNodeRuns> {
        Ok(self
            .prepared_node_runs
            .lock()
            .map_err(|_| anyhow::anyhow!("prepared node run lock is poisoned"))?
            .clone())
    }
}

#[async_trait]
impl<R, H> orchestration_runtime::execution_engine::ExecutionLifecycle
    for PersistedNodeLifecycle<'_, R, H>
where
    R: OrchestrationRuntimeRepository + Send + Sync,
    H: Send + Sync,
{
    async fn begin_node(
        &self,
        node: &orchestration_runtime::compiled_plan::CompiledNode,
        input_payload: &Value,
    ) -> Result<()> {
        if self
            .prepared_node_runs
            .lock()
            .map_err(|_| anyhow::anyhow!("prepared node run lock is poisoned"))?
            .contains_key(&node.node_id)
        {
            return Err(anyhow::anyhow!(
                "node {} started more than once in one execution segment",
                node.node_id
            ));
        }

        let node_run = self
            .service
            .repository
            .create_node_run(&CreateNodeRunInput {
                flow_run_id: self.flow_run_id,
                node_id: node.node_id.clone(),
                node_type: node.node_type.clone(),
                node_alias: node.alias.clone(),
                status: domain::NodeRunStatus::Running,
                input_payload: input_payload.clone(),
                debug_payload: json!({}),
                started_at: OffsetDateTime::now_utc(),
            })
            .await?;

        append_runtime_event(
            self.service,
            self.flow_run_id,
            debug_stream_events::node_started(&node_run),
        )
        .await;
        self.prepared_node_runs
            .lock()
            .map_err(|_| anyhow::anyhow!("prepared node run lock is poisoned"))?
            .insert(node.node_id.clone(), node_run.clone());
        *self
            .flow_execution_context
            .active_node
            .lock()
            .map_err(|_| anyhow::anyhow!("active runtime node lock is poisoned"))? =
            Some(RuntimeActiveNode {
                node_id: node.node_id.clone(),
                node_run_id: node_run.id,
            });

        Ok(())
    }
}

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
