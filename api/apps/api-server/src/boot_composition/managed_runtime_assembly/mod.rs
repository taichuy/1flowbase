use crate::{extension_bus::ManagedExtensionComposition, provider_runtime::ApiRuntimeServices};
use anyhow::Result;
use api_host_kernel::extension_bus::managed_activation::*;
use control_plane::ports::ContributionAuthorityLease;
use control_plane::{
    lifecycle_outbox_dispatcher::LifecycleSourceObserver,
    plugin_management::ManagedExecutionGovernancePort,
};
use plugin_framework::extension_bus::*;
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use std::collections::BTreeMap;
use std::sync::Arc;
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;
pub(crate) fn bind_managed_runtime(
    services: ApiRuntimeServices,
    store: MainDurableStore,
    node_id: String,
    modules: Vec<ModuleDescriptor>,
) -> (ApiRuntimeServices, Arc<ManagedExtensionComposition>) {
    let composition = Arc::new(ManagedExtensionComposition::new(
        store,
        node_id,
        services.runtime_backend().clone(),
        modules,
    ));
    (
        services.with_managed_composition(composition.clone()),
        composition,
    )
}
#[async_trait::async_trait]
impl ManagedCompositionPort for ManagedExtensionComposition {
    async fn snapshot(&self, workspace_id: Uuid) -> Option<Arc<ManagedWorkspaceSnapshot>> {
        Self::snapshot(self, workspace_id).await
    }
    async fn rebuild_installation(self: Arc<Self>, installation_id: Uuid) -> Result<()> {
        Self::rebuild_installation(&self, installation_id).await
    }
    async fn execute_hook(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        contribution_id: &ContributionId,
        principal: RuntimeExecutionPrincipal,
        invocation: extension_contracts::ManagedHookInvocation,
        input: runtime_core::runtime_backend::RuntimeManagedHookInput,
    ) -> Result<extension_contracts::ManagedHookOutcome> {
        Self::execute_hook(
            self,
            snapshot,
            contribution_id,
            principal,
            invocation,
            input,
        )
        .await
    }
    async fn authorize_snapshot_bindings(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_ids: &[ContributionId],
    ) -> Result<(Box<dyn ContributionAuthorityLease>, Vec<ContributionId>)> {
        Self::authorize_snapshot_bindings(self, snapshot, workspace_id, contribution_ids).await
    }
    async fn authorize_snapshot_binding(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
    ) -> Result<Box<dyn ContributionAuthorityLease>> {
        Self::authorize_snapshot_binding(self, snapshot, workspace_id, contribution_id).await
    }
    async fn reset_service_workers(
        &self,
        handle: &extension_contracts::ManagedExecutionHandle,
    ) -> Result<()> {
        Self::reset_service_workers(self, handle).await
    }
    async fn execute_snapshot(
        &self,
        snapshot: Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
        principal: RuntimeExecutionPrincipal,
        config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<serde_json::Value> {
        Self::execute_snapshot(
            self,
            snapshot,
            workspace_id,
            contribution_id,
            principal,
            config_payload,
            input_payload,
        )
        .await
    }
    async fn execute_snapshot_duplex(
        &self,
        snapshot: Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
        input: ManagedExecutionInput,
    ) -> Result<serde_json::Value> {
        Self::execute_snapshot_duplex(self, snapshot, workspace_id, contribution_id, input).await
    }
    fn attach_native_lifecycle_plan(&self, plan: EffectiveLifecycleSubscriberPlan) -> Result<()> {
        Self::attach_native_lifecycle_plan(self, plan)
    }
    async fn event_snapshot_for_graph(
        &self,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
    ) -> Result<Option<Arc<ManagedWorkspaceSnapshot>>> {
        Self::event_snapshot_for_graph(self, record).await
    }
    async fn deliver_event_record(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
    ) -> Result<bool> {
        Self::deliver_event_record(self, snapshot, record).await
    }
    fn attach_interface_module(&self, module: ModuleDescriptor) -> Result<()> {
        Self::attach_interface_module(self, module)
    }
    async fn switch_installation(
        self: Arc<Self>,
        workspace_id: Uuid,
        current: Uuid,
        target: Uuid,
        audit: domain::AuditLogRecord,
        task: domain::PluginTaskRecord,
    ) -> Result<domain::PluginTaskRecord> {
        Self::switch_installation(&self, workspace_id, current, target, audit, task).await
    }
    fn close_owned_admission(&self) {
        Self::close_owned_admission(self)
    }
    async fn wait_owned_shutdown(&self, budget: std::time::Duration) -> Result<()> {
        Self::wait_owned_shutdown(self, budget).await
    }
    async fn wait_for_shutdown(&self, budget: std::time::Duration) -> Result<()> {
        Self::wait_for_shutdown(self, budget).await
    }
    async fn cleanup_after_shutdown(&self) {
        Self::cleanup_after_shutdown(self).await
    }
    async fn current_snapshots(&self) -> BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>> {
        Self::current_snapshots(self).await
    }
    fn start_reclamation(self: Arc<Self>) {
        Self::start_reclamation(&self)
    }
    fn notify_reclamation(&self) {
        Self::notify_reclamation(self)
    }
    async fn attach_lifecycle_source_observer(
        &self,
        observer: Arc<LifecycleSourceObserver>,
    ) -> Result<()> {
        Self::attach_lifecycle_source_observer(self, observer).await
    }

    fn governance(self: Arc<Self>) -> Arc<dyn ManagedExecutionGovernancePort> {
        Self::governance(&self)
    }
    async fn terminate_command_delivery(
        &self,
        fact: &control_plane_contracts::ports::LifecycleOutboxRecord,
        termination: control_plane_contracts::ports::CommunicationCommandTermination,
    ) -> Result<()> {
        Self::terminate_command_delivery(self, fact, termination).await
    }
}
