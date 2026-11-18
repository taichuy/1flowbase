use anyhow::{bail, Context, Result};
use control_plane::lifecycle_outbox_dispatcher::LifecycleSourceObserver;
use control_plane::plugin_management::ManagedExecutionGovernancePort;
use control_plane::ports::ContributionAuthorityLease;
use plugin_framework::extension_bus::*;
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;
#[path = "managed_activation/lifetime.rs"]
pub mod lifetime;
pub use lifetime::*;
#[derive(Clone)]
pub struct ManagedContributionBinding {
    pub handle: ManagedExecutionHandle,
    pub descriptor: ContributionDescriptor,
    pub interface_protocol: Option<extension_contracts::ManagedInterfaceProtocol>,
    pub installation: domain::PluginInstallationRecord,
}

pub struct ManagedWorkspaceSnapshot {
    pub lifetime: Arc<SnapshotLifetime>,
    pub graph: Arc<EffectiveExtensionGraph>,
    pub authority: ManagedGraphAuthority,
    pub bindings: BTreeMap<ContributionId, ManagedContributionBinding>,
    pub lifecycle_plan: Option<EffectiveLifecycleSubscriberPlan>,
}

pub struct ManagedExecutionInput {
    pub principal: RuntimeExecutionPrincipal,
    pub config_payload: serde_json::Value,
    pub input_payload: serde_json::Value,
    pub duplex: Option<runtime_core::runtime_backend::RuntimeManagedDuplex>,
}

impl ManagedWorkspaceSnapshot {
    pub fn same_execution_snapshot(&self, other: &Self) -> bool {
        self.graph.fingerprint() == other.graph.fingerprint()
            && self.bindings.len() == other.bindings.len()
            && self.bindings.iter().all(|(id, binding)| {
                other
                    .bindings
                    .get(id)
                    .is_some_and(|old| old.handle == binding.handle)
            })
    }

    pub fn publication_plan(
        &self,
        contract_id: &str,
        contract_version: &str,
    ) -> Option<control_plane_contracts::ports::LifecyclePublicationPlan> {
        // Commands require a selected declaration; generic event publication cannot broadcast them.
        if contract_id == extension_contracts::COMMUNICATION_COMMAND_POINT {
            return None;
        }
        let plan = self.lifecycle_plan.as_ref()?;
        let subscribers = plan
            .subscribers()
            .iter()
            .filter(|s| {
                s.fact_contract_id == contract_id && s.fact_contract_version == contract_version
            })
            .map(
                |s| control_plane_contracts::ports::LifecycleSubscriberTarget {
                    subscriber_id: s.subscriber_id.clone(),
                    handler_id: s.handler_id.clone(),
                    handler_version: s.handler_version.clone(),
                },
            )
            .collect::<Vec<_>>();
        if subscribers.is_empty() {
            return None;
        }
        Some(control_plane_contracts::ports::LifecyclePublicationPlan {
            graph_fingerprint: self.graph.fingerprint().as_str().into(),
            subscribers,
        })
    }
}

#[async_trait::async_trait]
pub trait ManagedCompositionPort:
    Send + Sync + control_plane_contracts::ports::ManagedArtifactRemovalGuard
{
    async fn snapshot(&self, workspace_id: Uuid) -> Option<Arc<ManagedWorkspaceSnapshot>>;
    async fn rebuild_installation(self: Arc<Self>, installation_id: Uuid) -> Result<()>;
    async fn execute_hook(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        contribution_id: &ContributionId,
        principal: RuntimeExecutionPrincipal,
        invocation: extension_contracts::ManagedHookInvocation,
        input: runtime_core::runtime_backend::RuntimeManagedHookInput,
    ) -> Result<extension_contracts::ManagedHookOutcome>;
    async fn authorize_snapshot_bindings(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_ids: &[ContributionId],
    ) -> Result<(Box<dyn ContributionAuthorityLease>, Vec<ContributionId>)>;
    async fn authorize_snapshot_binding(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
    ) -> Result<Box<dyn ContributionAuthorityLease>>;
    async fn reset_service_workers(
        &self,
        handle: &extension_contracts::ManagedExecutionHandle,
    ) -> Result<()>;
    async fn execute_snapshot(
        &self,
        snapshot: Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
        principal: RuntimeExecutionPrincipal,
        config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<serde_json::Value>;
    async fn execute_snapshot_duplex(
        &self,
        snapshot: Arc<ManagedWorkspaceSnapshot>,
        workspace_id: Uuid,
        contribution_id: &ContributionId,
        input: ManagedExecutionInput,
    ) -> Result<serde_json::Value>;
    fn attach_native_lifecycle_plan(&self, plan: EffectiveLifecycleSubscriberPlan) -> Result<()>;
    async fn event_snapshot_for_graph(
        &self,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
    ) -> Result<Option<Arc<ManagedWorkspaceSnapshot>>>;
    async fn deliver_event_record(
        &self,
        snapshot: &Arc<ManagedWorkspaceSnapshot>,
        record: &control_plane_contracts::ports::LifecycleOutboxRecord,
    ) -> Result<bool>;
    fn attach_interface_module(&self, module: ModuleDescriptor) -> Result<()>;
    async fn switch_installation(
        self: Arc<Self>,
        workspace_id: Uuid,
        current: Uuid,
        target: Uuid,
        audit: domain::AuditLogRecord,
        task: domain::PluginTaskRecord,
    ) -> Result<domain::PluginTaskRecord>;
    fn close_owned_admission(&self);
    async fn wait_owned_shutdown(&self, budget: std::time::Duration) -> Result<()>;
    async fn wait_for_shutdown(&self, budget: std::time::Duration) -> Result<()>;
    async fn cleanup_after_shutdown(&self);
    async fn current_snapshots(&self) -> BTreeMap<Uuid, Arc<ManagedWorkspaceSnapshot>>;
    async fn refresh_boot_snapshots(self: Arc<Self>) -> Result<()>;
    fn start_reclamation(self: Arc<Self>);
    fn notify_reclamation(&self);
    async fn attach_lifecycle_source_observer(
        &self,
        observer: Arc<LifecycleSourceObserver>,
    ) -> Result<()>;

    fn governance(self: Arc<Self>) -> Arc<dyn ManagedExecutionGovernancePort>;
    async fn terminate_command_delivery(
        &self,
        fact: &control_plane_contracts::ports::LifecycleOutboxRecord,
        termination: control_plane_contracts::ports::CommunicationCommandTermination,
    ) -> Result<()>;
}
