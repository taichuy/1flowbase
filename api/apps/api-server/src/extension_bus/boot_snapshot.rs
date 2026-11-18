use super::authentication_activation::AuthenticationFactoryAssembly;
use super::InterfaceContributionCollectorAssembly;
use crate::{
    extension_bus::{
        input_assembly::ExtensionGraphInputAssembly,
        production_host_extension_authentication_factories, production_interface_contributions,
        InterfaceContributionCollector,
    },
    generation_dispatch::GenerationDispatchAssembly,
};
pub use api_host_kernel::extension_bus::boot_snapshot::*;
use plugin_framework::extension_bus::EffectiveExtensionGraph;
use std::sync::Arc;
pub(crate) trait SnapshotAssembly {
    #[cfg_attr(test, api_test_selection::group(contracts, routes))]
    #[cfg(test)]
    fn compile(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
        host_authentication_factories: Vec<super::AuthenticationAdapterFactoryBinding>,
    ) -> anyhow::Result<Self>
    where
        Self: Sized;

    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    fn new(graph: Arc<EffectiveExtensionGraph>) -> Self;
    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    fn compile_for_test(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
    ) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn publish_complete_catalog(
        &self,
        state: &Arc<crate::app_state::ApiState>,
    ) -> anyhow::Result<Arc<crate::generation_dispatch::GenerationDispatch>>;
    fn publish_complete_catalog_with_route_assembly(
        &self,
        state: &Arc<crate::app_state::ApiState>,
        assembly: crate::routes::console_route_assembly::ConsoleRouteAssembly<
            Arc<crate::app_state::ApiState>,
        >,
    ) -> anyhow::Result<Arc<crate::generation_dispatch::GenerationDispatch>>;
    fn publish_complete_catalog_inner(
        &self,
        state: &Arc<crate::app_state::ApiState>,
        assembly: Option<
            crate::routes::console_route_assembly::ConsoleRouteAssembly<
                Arc<crate::app_state::ApiState>,
            >,
        >,
    ) -> anyhow::Result<Arc<crate::generation_dispatch::GenerationDispatch>>;
}
impl SnapshotAssembly for ExtensionBootSnapshot {
    #[cfg_attr(test, api_test_selection::group(contracts, routes))]
    #[cfg(test)]
    fn compile(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
        host_authentication_factories: Vec<super::AuthenticationAdapterFactoryBinding>,
    ) -> anyhow::Result<Self> {
        Self::compile_ready(
            graph,
            descriptors,
            host_authentication_factories,
            super::AuthenticationAdapterFactoryRegistry::built_in()?,
        )
    }

    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    fn new(graph: Arc<EffectiveExtensionGraph>) -> Self {
        Self::new_ready(
            graph,
            super::AuthenticationAdapterFactoryRegistry::built_in()
                .expect("built-in authentication factories must be valid"),
        )
    }
    #[cfg_attr(test, api_test_selection::group(contracts))]
    #[cfg(test)]
    fn compile_for_test(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
    ) -> anyhow::Result<Self> {
        Self::compile_ready(
            graph,
            descriptors,
            Vec::new(),
            super::AuthenticationAdapterFactoryRegistry::built_in()?,
        )
    }

    /// Transfer the returned strong owner into an external serving/candidate root.
    /// This snapshot only stores Weak; retaining state or metadata alone is not execution admission.
    fn publish_complete_catalog(
        &self,
        state: &Arc<crate::app_state::ApiState>,
    ) -> anyhow::Result<Arc<crate::generation_dispatch::GenerationDispatch>> {
        self.publish_complete_catalog_inner(state, None)
    }
    fn publish_complete_catalog_with_route_assembly(
        &self,
        state: &Arc<crate::app_state::ApiState>,
        assembly: crate::routes::console_route_assembly::ConsoleRouteAssembly<
            Arc<crate::app_state::ApiState>,
        >,
    ) -> anyhow::Result<Arc<crate::generation_dispatch::GenerationDispatch>> {
        self.publish_complete_catalog_inner(state, Some(assembly))
    }
    fn publish_complete_catalog_inner(
        &self,
        state: &Arc<crate::app_state::ApiState>,
        assembly: Option<
            crate::routes::console_route_assembly::ConsoleRouteAssembly<
                Arc<crate::app_state::ApiState>,
            >,
        >,
    ) -> anyhow::Result<Arc<crate::generation_dispatch::GenerationDispatch>> {
        let _publication = self.begin_publication(state)?;
        let owner = crate::generation_dispatch::GenerationDispatch::new(self.generation_id());
        let _registry = self
            .interface_registry()
            .ok_or_else(|| anyhow::anyhow!("interface registry is absent"))?;
        let managed = crate::extension_bus::managed_interface::ManagedInterfaceFactory::new(
            state,
            self.graph_arc().clone(),
        );
        let mut collector = InterfaceContributionCollector::new(
            interface_runtime::GraphFingerprint::new(self.graph().fingerprint().as_str())?,
        )
        .with_managed_invocations(managed.clone())
        .with_api_event_observation(
            state.api_docs.clone(),
            crate::event_center::api_observer::ApiOperationRuleObserver::new(state),
        )
        .with_event_observer(crate::event_center::interface::InterfaceRuleObserver::new(
            state,
        ));
        for contribution in production_interface_contributions(state, &owner)? {
            collector.add(contribution)?;
        }
        for service in state.console_surface_registry.managed_services() {
            collector.add(crate::managed_services::registry_contribution(
                state, service,
            )?)?;
        }
        let (candidate, console_operation_snapshot) = collector
            .compile_complete_console_snapshot(state.console_operation_registry.inventory())?;
        crate::event_center::api_catalog::validate_static_inventory(&candidate, &state.api_docs)?;
        managed.bind_registry(&candidate)?;
        let module = plugin_framework::extension_bus::compile_managed_interface_module(
            candidate
                .definitions()
                .map(|definition| definition.interface_id().as_str()),
        )?;
        if !self.has_pinned_managed_snapshots() {
            if let Ok(composition) = state.provider_runtime.managed_composition() {
                composition.attach_interface_module(module)?;
            }
        }
        self.authentication_factories()
            .validate_registry(&candidate)?;
        owner
            .compile_router(state, &candidate, assembly)
            .map_err(|error| error.0)?;
        self.publish_ready(console_operation_snapshot, &owner, candidate)?;
        Ok(owner)
    }
}
pub fn compile_extension_boot_snapshot(
    graph: Arc<EffectiveExtensionGraph>,
    assembly: &ExtensionGraphInputAssembly,
) -> anyhow::Result<ExtensionBootSnapshot> {
    let host_authentication_factories = production_host_extension_authentication_factories()
        .activate(assembly.host_extension_manifests())?;
    ExtensionBootSnapshot::compile_ready(
        graph,
        assembly.interface_operations(),
        host_authentication_factories,
        super::AuthenticationAdapterFactoryRegistry::built_in()?,
    )
}
