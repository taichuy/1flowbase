//! Stateless composition adapters for MCP-owned Host services.
pub(crate) struct ApplicationLogExchange;
#[async_trait::async_trait]
impl api_host_kernel::resource_import_ports::ApplicationLogExchangePort for ApplicationLogExchange {
    async fn execute(
        &self,
        state: &std::sync::Arc<crate::app_state::ApiState>,
        actor: &domain::ActorContext,
        request: extension_contracts::ApplicationLogsRequest,
    ) -> Result<extension_contracts::ApplicationLogsResponse, crate::error_response::ApiError> {
        crate::application_log_exchange::execute(state, actor, request).await
    }
}

pub(crate) struct InterfaceCatalog;
#[async_trait::async_trait]
impl api_host_kernel::mcp_catalog::McpInterfaceCatalogProvider for InterfaceCatalog {
    async fn entries(
        &self,
        dependencies: &api_host_kernel::mcp_catalog::McpInterfaceCatalogDependencies,
        actor: &domain::ActorContext,
    ) -> Result<Vec<domain::McpInterfaceCatalogEntry>, crate::error_response::ApiError> {
        api_mcp::routes::mcp_management::mcp_interface_catalog_entries_with(dependencies, actor)
            .await
    }
}
