use std::sync::{Arc, OnceLock};

use plugin_framework::extension_bus::{
    ContributionResolutionReceipt, EffectiveExtensionGraph, EffectiveExtensionPoint, ModuleId,
    ModuleResolutionReceipt, Provenance,
};
use serde::Serialize;

use super::{
    input_assembly::ExtensionGraphInputAssembly,
    production_host_extension_authentication_factories, production_interface_contributions,
    AuthenticationAdapterFactoryBinding, AuthenticationAdapterFactoryRegistry,
    InterfaceContributionCollector,
};

pub const EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1: &str = "1flowbase.effective-extension-plan/v1";

pub struct ExtensionBootSnapshot {
    managed_composition: OnceLock<Arc<super::ManagedExtensionComposition>>,
    graph: Arc<EffectiveExtensionGraph>,
    interface_registry: Option<Arc<interface_runtime::DynamicInterfaceRegistry>>,
    authentication_factories: AuthenticationAdapterFactoryRegistry,
    external_endpoint_catalog:
        OnceLock<Arc<crate::external_endpoint_catalog::ExternalEndpointCatalog>>,
    console_operation_snapshot:
        OnceLock<Arc<crate::console_operation_compilation::CompiledConsoleOperationSnapshot>>,
}

pub fn compile_extension_boot_snapshot(
    graph: Arc<EffectiveExtensionGraph>,
    assembly: &ExtensionGraphInputAssembly,
) -> anyhow::Result<ExtensionBootSnapshot> {
    let host_authentication_factories = production_host_extension_authentication_factories()
        .activate(assembly.host_extension_manifests())?;
    ExtensionBootSnapshot::compile(
        graph,
        assembly.interface_operations(),
        host_authentication_factories,
    )
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
    pub(crate) fn attach_managed_composition(
        &self,
        composition: Arc<super::ManagedExtensionComposition>,
    ) -> anyhow::Result<()> {
        self.managed_composition
            .set(composition)
            .map_err(|_| anyhow::anyhow!("managed composition already attached"))
    }
    #[cfg(test)]
    pub(crate) fn new(graph: Arc<EffectiveExtensionGraph>) -> Self {
        Self {
            graph,
            interface_registry: None,
            authentication_factories: AuthenticationAdapterFactoryRegistry::built_in()
                .expect("built-in authentication factories must be valid"),
            managed_composition: OnceLock::new(),
            external_endpoint_catalog: OnceLock::new(),
            console_operation_snapshot: OnceLock::new(),
        }
    }

    pub(crate) fn compile(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
        host_authentication_factories: Vec<AuthenticationAdapterFactoryBinding>,
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
        let mut authentication_factories = AuthenticationAdapterFactoryRegistry::built_in()?;
        authentication_factories.activate_host_extensions(host_authentication_factories)?;
        Ok(Self {
            graph,
            interface_registry: Some(interface_registry),
            authentication_factories,
            managed_composition: OnceLock::new(),
            external_endpoint_catalog: OnceLock::new(),
            console_operation_snapshot: OnceLock::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn compile_for_test(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
    ) -> anyhow::Result<Self> {
        Self::compile(graph, descriptors, Vec::new())
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

    pub(crate) fn publish_external_endpoint_catalog(
        &self,
        catalog: crate::external_endpoint_catalog::ExternalEndpointCatalog,
    ) -> &Arc<crate::external_endpoint_catalog::ExternalEndpointCatalog> {
        self.external_endpoint_catalog
            .get_or_init(|| Arc::new(catalog))
    }

    #[cfg(test)]
    pub(crate) fn external_endpoint_catalog(
        &self,
    ) -> Option<&Arc<crate::external_endpoint_catalog::ExternalEndpointCatalog>> {
        self.external_endpoint_catalog.get()
    }

    pub(crate) fn publish_complete_catalog(
        &self,
        state: &Arc<crate::app_state::ApiState>,
    ) -> anyhow::Result<()> {
        let registry = self
            .interface_registry
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("interface registry is absent"))?;
        let managed =
            super::managed_interface::ManagedInterfaceFactory::new(state, self.graph.clone());
        let mut collector = InterfaceContributionCollector::new(
            interface_runtime::GraphFingerprint::new(self.graph.fingerprint().as_str())?,
        )
        .with_managed_invocations(managed.clone());
        for contribution in production_interface_contributions(state)? {
            collector.add(contribution)?;
        }
        let (candidate, console_operation_snapshot) = collector
            .compile_complete_console_snapshot(state.console_operation_registry.inventory())?;
        managed.bind_registry(&candidate)?;
        let module = plugin_framework::extension_bus::compile_managed_interface_module(
            candidate
                .definitions()
                .map(|definition| definition.interface_id().as_str()),
        )?;
        if let Ok(composition) = state.provider_runtime.managed_composition() {
            composition.attach_interface_module(module)?;
        }
        self.authentication_factories
            .validate_registry(&candidate)?;
        registry.publish(candidate);
        self.console_operation_snapshot
            .set(Arc::new(console_operation_snapshot))
            .map_err(|_| {
                anyhow::anyhow!("compiled Console operation snapshot is already published")
            })?;
        Ok(())
    }

    pub(crate) async fn authenticate_invocation<C, P>(
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
