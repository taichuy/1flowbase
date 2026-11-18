//! Read-only MCP catalog inputs; projection remains MCP-owned.
use std::sync::Arc;
#[derive(Clone)]
pub struct McpInterfaceCatalogDependencies {
    pub store: storage_durable_postgres::MainDurableStore,
    pub openapi: crate::openapi_interface::OpenApiCapabilityCatalogDependencies,
}

impl McpInterfaceCatalogDependencies {
    pub fn with_interface_registry_snapshot(
        &self,
        interface_registry: Arc<interface_runtime::CompiledInterfaceRegistry>,
    ) -> Self {
        let mut openapi = self.openapi.clone();
        openapi.interface_registry = Some(interface_registry);
        Self {
            store: self.store.clone(),
            openapi,
        }
    }
}

/// Catalog projection is injected by the composition root and consumes the exact compiled snapshot.
#[async_trait::async_trait]
pub trait McpInterfaceCatalogProvider: Send + Sync + 'static {
    async fn entries(
        &self,
        dependencies: &McpInterfaceCatalogDependencies,
        actor: &domain::ActorContext,
    ) -> Result<Vec<domain::McpInterfaceCatalogEntry>, crate::error_response::ApiError>;
}
