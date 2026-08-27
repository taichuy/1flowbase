use std::sync::Arc;

use control_plane::host_infrastructure_config::HostInfrastructureConfigService;
use plugin_framework::extension_bus::{
    ContributionResolutionReceipt, EffectiveExtensionGraph, EffectiveExtensionPoint, ModuleId,
    ModuleResolutionReceipt, Provenance,
};
use serde::Serialize;
use storage_durable_postgres::MainDurableStore;

pub(crate) struct DurableHostInfrastructureProvidersViewQuery {
    store: MainDurableStore,
    node_id: String,
}

impl DurableHostInfrastructureProvidersViewQuery {
    pub(crate) fn new(store: MainDurableStore, node_id: String) -> Self {
        Self { store, node_id }
    }
}

impl crate::routes::host_infrastructure::interface_operation::HostInfrastructureProvidersViewQuery
    for DurableHostInfrastructureProvidersViewQuery
{
    fn list(
        &self,
    ) -> crate::routes::host_infrastructure::interface_operation::HostInfrastructureProvidersViewQueryFuture<'_>
    {
        Box::pin(async move {
            Ok(
                HostInfrastructureConfigService::new(self.store.clone(), self.node_id.clone())
                    .list_providers()
                    .await?,
            )
        })
    }
}

pub const EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1: &str = "1flowbase.effective-extension-plan/v1";

#[derive(Debug)]
pub struct ExtensionBootSnapshot {
    graph: Arc<EffectiveExtensionGraph>,
    interface_registry: Option<Arc<interface_runtime::DynamicInterfaceRegistry>>,
}

impl ExtensionBootSnapshot {
    #[cfg(test)]
    pub(crate) fn new(graph: Arc<EffectiveExtensionGraph>) -> Self {
        Self {
            graph,
            interface_registry: None,
        }
    }

    pub(crate) fn compile(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
        providers_view_query: Arc<
            dyn crate::routes::host_infrastructure::interface_operation::HostInfrastructureProvidersViewQuery,
        >,
    ) -> anyhow::Result<Self> {
        let interface_snapshot =
            crate::routes::host_infrastructure::interface_operation::compile_interface_registry(
                Arc::clone(&graph),
                descriptors,
                providers_view_query,
            )?;
        let interface_registry = Arc::new(interface_runtime::DynamicInterfaceRegistry::new(
            interface_snapshot,
        ));
        Ok(Self {
            graph,
            interface_registry: Some(interface_registry),
        })
    }

    #[cfg(test)]
    pub(crate) fn compile_for_test(
        graph: Arc<EffectiveExtensionGraph>,
        descriptors: &[plugin_framework::HostExtensionInterfaceOperationManifest],
    ) -> anyhow::Result<Self> {
        Self::compile(
            graph,
            descriptors,
            Arc::new(
                crate::routes::host_infrastructure::interface_operation::UnavailableHostInfrastructureProvidersViewQuery,
            ),
        )
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
