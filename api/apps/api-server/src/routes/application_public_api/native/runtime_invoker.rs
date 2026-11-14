use crate::routes::mcp_protocol::virtual_ui;
use api_public_gateway::routes::application_public_api::{
    native::service_error,
    native_read_interface::{NativeRuntimeInvokerFactory, RuntimeInvokerFuture},
};
use axum::http::HeaderMap;
use std::sync::Arc;
pub(crate) struct NativeRuntimeInvokerFactoryDependencies {
    pub(crate) generation: std::sync::Weak<crate::generation_dispatch::GenerationDispatch>,
    pub(crate) runtime_dependencies: virtual_ui::RuntimeInternalToolInvokerDependencies,
    pub(crate) interface_catalog:
        crate::routes::mcp_management::interface_catalog::McpInterfaceCatalogDependencies,
    pub(crate) interface_registry:
        Arc<dyn crate::routes::mcp_management::interface_catalog::McpInterfaceRegistrySnapshotPort>,
}

pub(super) struct NativeRuntimeInvokerFactoryAdapter(NativeRuntimeInvokerFactoryDependencies);

impl NativeRuntimeInvokerFactory for NativeRuntimeInvokerFactoryAdapter {
    fn without_mcp<'a>(&'a self, actor: &'a domain::ActorContext) -> RuntimeInvokerFuture<'a> {
        Box::pin(async move {
            let owner = self
                .0
                .generation
                .upgrade()
                .ok_or_else(|| service_error(crate::generation_dispatch::unavailable().0))?;
            let invoker = virtual_ui::RuntimeInternalToolInvokerFactory::new(
                self.0.runtime_dependencies.clone(),
                Arc::new(virtual_ui::McpInterfaceCatalogSnapshot::new(Vec::new())),
                owner,
            )
            .create_workflow_credential_host(actor.clone())
            .await
            .map_err(|error| service_error(error.0))?;
            Ok(Arc::new(invoker)
                as Arc<
                    dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
                >)
        })
    }

    fn for_actor<'a>(
        &'a self,
        actor: &'a control_plane::application_public_api::api_keys::ApplicationApiKeyActor,
    ) -> RuntimeInvokerFuture<'a> {
        let actor = actor.clone();
        Box::pin(async move {
            let owner = self
                .0
                .generation
                .upgrade()
                .ok_or_else(|| service_error(crate::generation_dispatch::unavailable().0))?;
            let catalog_dependencies = self.0.interface_catalog.with_interface_registry_snapshot(
                self.0
                    .interface_registry
                    .snapshot()
                    .map_err(|error| service_error(error.0))?,
            );
            let interface_catalog =
                crate::routes::mcp_management::interface_catalog::mcp_interface_catalog_entries_with(
                    &catalog_dependencies,
                    &actor.actor,
                )
                .await
                .map_err(|error| service_error(error.0))?;
            let invoker = virtual_ui::ApiMcpRuntimeToolInvoker::new(
                virtual_ui::RuntimeInternalToolInvokerFactory::new(
                    self.0.runtime_dependencies.clone(),
                    Arc::new(virtual_ui::McpInterfaceCatalogSnapshot::new(
                        interface_catalog,
                    )),
                    owner,
                ),
                HeaderMap::new(),
                actor.actor.clone(),
                // Public application runs select MCP only through compiled node bindings.
                Vec::new(),
            )
            .await
            .map_err(|error| service_error(error.0))?;
            Ok(Arc::new(invoker)
                as Arc<
                    dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker,
                >)
        })
    }
}

pub(crate) fn native_runtime_invoker_factory(
    dependencies: NativeRuntimeInvokerFactoryDependencies,
) -> Arc<dyn NativeRuntimeInvokerFactory> {
    Arc::new(NativeRuntimeInvokerFactoryAdapter(dependencies))
}
