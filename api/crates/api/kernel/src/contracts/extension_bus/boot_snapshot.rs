use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock, Weak},
};

use plugin_framework::extension_bus::{
    ContributionResolutionReceipt, EffectiveExtensionGraph, EffectiveExtensionPoint, ModuleId,
    ModuleResolutionReceipt, Provenance,
};
use serde::Serialize;

use super::{AuthenticationAdapterFactoryBinding, AuthenticationAdapterFactoryRegistry};

pub const EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1: &str = "1flowbase.effective-extension-plan/v1";

pub struct ExtensionBootSnapshot {
    generation_id: uuid::Uuid,
    #[cfg(any(test, feature = "test-support"))]
    dispatch_assemblies: std::sync::atomic::AtomicUsize,
    dispatch_owner: OnceLock<Weak<crate::generation_dispatch::GenerationDispatch>>,
    publication: Mutex<()>,
    managed_composition: OnceLock<Arc<dyn super::managed_activation::ManagedCompositionPort>>,
    graph: Arc<EffectiveExtensionGraph>,
    interface_registry: Option<Arc<interface_runtime::DynamicInterfaceRegistry>>,
    authentication_factories: Arc<AuthenticationAdapterFactoryRegistry>,
    managed_snapshots: OnceLock<BTreeMap<uuid::Uuid, Arc<super::ManagedWorkspaceSnapshot>>>,
    external_endpoint_catalog:
        OnceLock<Arc<crate::external_endpoint_catalog::ExternalEndpointCatalog>>,
    console_operation_snapshot:
        OnceLock<Arc<crate::console_operation_compilation::CompiledConsoleOperationSnapshot>>,
}

impl std::fmt::Debug for ExtensionBootSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExtensionBootSnapshot")
            .field("graph_fingerprint", &self.graph.fingerprint().as_str())
            .field("has_interface_registry", &self.interface_registry.is_some())
            .finish()
    }
}

impl ExtensionBootSnapshot {
    pub fn begin_publication(
        &self,
        state: &Arc<crate::app_state::ApiState>,
    ) -> anyhow::Result<std::sync::MutexGuard<'_, ()>> {
        let guard = self
            .publication
            .lock()
            .map_err(|_| anyhow::anyhow!("catalog publication lock poisoned"))?;
        if self.console_operation_snapshot.get().is_some() || self.dispatch_owner.get().is_some() {
            anyhow::bail!("complete catalog is already published");
        }
        if state
            .extension_boot_snapshot
            .as_ref()
            .map(|boot| boot.generation_id())
            != Some(self.generation_id)
        {
            anyhow::bail!("catalog state belongs to another generation");
        }
        Ok(guard)
    }
    pub fn publish_ready(
        &self,
        console: crate::console_operation_compilation::CompiledConsoleOperationSnapshot,
        owner: &Arc<crate::generation_dispatch::GenerationDispatch>,
        candidate: Arc<interface_runtime::CompiledInterfaceRegistry>,
    ) -> anyhow::Result<()> {
        self.console_operation_snapshot
            .set(Arc::new(console))
            .map_err(|_| {
                anyhow::anyhow!("compiled Console operation snapshot is already published")
            })?;
        self.dispatch_owner
            .set(Arc::downgrade(owner))
            .map_err(|_| anyhow::anyhow!("generation dispatch already published"))?;
        self.interface_registry
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("interface registry is absent"))?
            .publish(candidate);
        Ok(())
    }
    pub fn authentication_factories(&self) -> &AuthenticationAdapterFactoryRegistry {
        &self.authentication_factories
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn record_dispatch_assembly(&self) {
        self.dispatch_assemblies
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn dispatch_assembly_count(&self) -> usize {
        self.dispatch_assemblies
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn generation_id(&self) -> uuid::Uuid {
        self.generation_id
    }
    pub fn attach_managed_composition(
        &self,
        composition: Arc<dyn super::managed_activation::ManagedCompositionPort>,
    ) -> anyhow::Result<()> {
        self.managed_composition
            .set(composition)
            .map_err(|_| anyhow::anyhow!("managed composition already attached"))
    }
    /// A host generation owns a separate registry and projections, but keeps the boot-scoped
    /// native graph and activated authentication factories unchanged.
    pub fn fork_generation(
        &self,
        snapshots: BTreeMap<uuid::Uuid, Arc<super::ManagedWorkspaceSnapshot>>,
    ) -> anyhow::Result<Self> {
        let registry = interface_runtime::RegistryCompiler::new(
            interface_runtime::GraphFingerprint::new(self.graph.fingerprint().as_str())?,
            [],
            [],
        )
        .compile()?;
        let fork = Self {
            generation_id: uuid::Uuid::now_v7(),
            #[cfg(any(test, feature = "test-support"))]
            dispatch_assemblies: std::sync::atomic::AtomicUsize::new(0),
            dispatch_owner: OnceLock::new(),
            publication: Mutex::new(()),
            managed_composition: OnceLock::new(),
            graph: self.graph.clone(),
            interface_registry: Some(Arc::new(interface_runtime::DynamicInterfaceRegistry::new(
                registry,
            ))),
            authentication_factories: self.authentication_factories.clone(),
            managed_snapshots: OnceLock::new(),
            external_endpoint_catalog: OnceLock::new(),
            console_operation_snapshot: OnceLock::new(),
        };
        fork.set_pinned_managed_snapshots(snapshots)?;
        Ok(fork)
    }

    pub fn set_pinned_managed_snapshots(
        &self,
        snapshots: BTreeMap<uuid::Uuid, Arc<super::ManagedWorkspaceSnapshot>>,
    ) -> anyhow::Result<()> {
        self.managed_snapshots
            .set(snapshots)
            .map_err(|_| anyhow::anyhow!("managed snapshots already pinned"))
    }

    pub fn has_pinned_managed_snapshots(&self) -> bool {
        self.managed_snapshots.get().is_some()
    }

    pub fn managed_snapshot(
        &self,
        workspace_id: uuid::Uuid,
    ) -> Option<Arc<super::ManagedWorkspaceSnapshot>> {
        self.managed_snapshots.get()?.get(&workspace_id).cloned()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn new_ready(
        graph: Arc<EffectiveExtensionGraph>,
        authentication_factories: AuthenticationAdapterFactoryRegistry,
    ) -> Self {
        Self {
            generation_id: uuid::Uuid::now_v7(),
            #[cfg(any(test, feature = "test-support"))]
            dispatch_assemblies: std::sync::atomic::AtomicUsize::new(0),
            dispatch_owner: OnceLock::new(),
            publication: Mutex::new(()),
            graph,
            interface_registry: None,
            authentication_factories: Arc::new(authentication_factories),
            managed_snapshots: OnceLock::new(),
            managed_composition: OnceLock::new(),
            external_endpoint_catalog: OnceLock::new(),
            console_operation_snapshot: OnceLock::new(),
        }
    }

    pub fn compile_ready(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
        host_authentication_factories: Vec<AuthenticationAdapterFactoryBinding>,
        mut authentication_factories: AuthenticationAdapterFactoryRegistry,
    ) -> anyhow::Result<Self> {
        if !descriptors.is_empty() {
            anyhow::bail!("host interface operation descriptors have no registered native handler");
        }
        let interface_snapshot = interface_runtime::RegistryCompiler::new(
            interface_runtime::GraphFingerprint::new(graph.fingerprint().as_str())?,
            [],
            [],
        )
        .compile()?;
        let interface_registry = Arc::new(interface_runtime::DynamicInterfaceRegistry::new(
            interface_snapshot,
        ));
        authentication_factories.activate_host_extensions(host_authentication_factories)?;
        Ok(Self {
            generation_id: uuid::Uuid::now_v7(),
            #[cfg(any(test, feature = "test-support"))]
            dispatch_assemblies: std::sync::atomic::AtomicUsize::new(0),
            dispatch_owner: OnceLock::new(),
            publication: Mutex::new(()),
            graph,
            interface_registry: Some(interface_registry),
            authentication_factories: Arc::new(authentication_factories),
            managed_snapshots: OnceLock::new(),
            managed_composition: OnceLock::new(),
            external_endpoint_catalog: OnceLock::new(),
            console_operation_snapshot: OnceLock::new(),
        })
    }

    pub fn graph(&self) -> &EffectiveExtensionGraph {
        self.graph.as_ref()
    }

    pub fn graph_arc(&self) -> &Arc<EffectiveExtensionGraph> {
        &self.graph
    }

    pub fn fingerprint(&self) -> &str {
        self.graph.fingerprint().as_str()
    }

    pub fn interface_registry(&self) -> Option<&Arc<interface_runtime::DynamicInterfaceRegistry>> {
        self.interface_registry.as_ref()
    }

    pub fn publish_external_endpoint_catalog(
        &self,
        catalog: crate::external_endpoint_catalog::ExternalEndpointCatalog,
    ) -> &Arc<crate::external_endpoint_catalog::ExternalEndpointCatalog> {
        self.external_endpoint_catalog
            .get_or_init(|| Arc::new(catalog))
    }

    pub fn external_endpoint_catalog(
        &self,
    ) -> Option<&Arc<crate::external_endpoint_catalog::ExternalEndpointCatalog>> {
        self.external_endpoint_catalog.get()
    }

    #[cfg(feature = "test-support")]
    pub fn console_operation_snapshot_for_test_support(
        &self,
    ) -> Option<&crate::console_operation_compilation::CompiledConsoleOperationSnapshot> {
        self.console_operation_snapshot.get().map(AsRef::as_ref)
    }

    pub fn dispatch_owner(
        &self,
    ) -> Result<Arc<crate::generation_dispatch::GenerationDispatch>, crate::error_response::ApiError>
    {
        self.dispatch_owner
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(crate::generation_dispatch::unavailable)
    }

    pub async fn authenticate_invocation<C, P>(
        &self,
        snapshot: Arc<interface_runtime::CompiledInterfaceRegistry>,
        binding: &interface_runtime::BindingId,
        protocol: interface_runtime::InterfaceProtocol,
        credential: C,
    ) -> anyhow::Result<super::AuthenticatedInvocation<P>>
    where
        C: std::any::Any + Send + 'static,
        P: interface_runtime::InvocationPrincipal,
    {
        self.authentication_factories
            .authenticate_invocation(snapshot, binding, protocol, credential)
            .await
    }

    pub fn effective_plan(&self) -> EffectiveExtensionPlan<'_> {
        EffectiveExtensionPlan {
            schema_version: EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1,
            graph_fingerprint: self.graph.fingerprint().as_str(),
            bus_version: self.graph.bus_version().as_str(),
            module_order: self.graph.module_order(),
            module_provenance: self.graph.module_provenance(),
            module_receipts: self.graph.module_receipts(),
            points: self.graph.points(),
            contribution_receipts: self.graph.contribution_receipts(),
        }
    }

    pub fn render_effective_plan(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.effective_plan())
    }
}

#[derive(Debug, Serialize)]
pub struct EffectiveExtensionPlan<'a> {
    pub schema_version: &'static str,
    pub graph_fingerprint: &'a str,
    pub bus_version: &'static str,
    pub module_order: &'a [ModuleId],
    pub module_provenance: &'a [Provenance],
    pub module_receipts: &'a [ModuleResolutionReceipt],
    pub points: &'a [EffectiveExtensionPoint],
    pub contribution_receipts: &'a [ContributionResolutionReceipt],
}
