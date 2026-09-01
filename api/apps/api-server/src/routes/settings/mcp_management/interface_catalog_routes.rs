use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum McpCatalogInput {
    Catalog,
    Interfaces(McpInterfaceCatalogQuery),
    List(McpListQuery),
    Export,
}

impl InterfaceContract for McpCatalogInput {
    const CONTRACT_ID: &'static str = "console-mcp-catalog-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpCatalogOutput {
    Catalog(McpCatalogResponse),
    Interfaces(Vec<McpInterfaceCatalogEntryResponse>),
    List(Vec<McpListItemSummaryResponse>),
    Export(McpExportPackageResponse),
}

impl InterfaceContract for McpCatalogOutput {
    const CONTRACT_ID: &'static str = "console-mcp-catalog-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpCatalogAdapter(interface_catalog::McpInterfaceCatalogDependencies);

impl McpCatalogAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpCatalogInput,
    ) -> Result<McpCatalogOutput, ApiError> {
        let actor = principal.actor();
        let service = McpManagementService::new(self.0.store.clone());
        match input {
            McpCatalogInput::Catalog => {
                let snapshot = service.read_catalog_for_actor(actor).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpCatalogOutput::Catalog(to_catalog_response(
                    snapshot,
                    &operations,
                )?))
            }
            McpCatalogInput::Interfaces(query) => {
                service
                    .authorize_interface_catalog_view(actor.user_id)
                    .await?;
                let mut entries =
                    interface_catalog::mcp_interface_catalog_entries_with(&self.0, actor).await?;
                if query.bindable_only.unwrap_or(false) {
                    entries.retain(|entry| entry.bindable);
                }
                Ok(McpCatalogOutput::Interfaces(
                    entries.into_iter().map(to_interface_response).collect(),
                ))
            }
            McpCatalogInput::List(query) => {
                let items = service
                    .list_items_for_actor(
                        actor,
                        query.instance_id.as_deref(),
                        query.path.as_deref(),
                        query.path_regex.as_deref(),
                        query.keywords.as_deref(),
                        query.depth,
                        query.limit,
                    )
                    .await?;
                let instance_id = query.instance_id.as_deref().ok_or(
                    control_plane::errors::ControlPlaneError::InvalidInput("instance_id"),
                )?;
                let discovery_policy = service
                    .get_instance_discovery_policy_for_actor(actor, instance_id)
                    .await?;
                let return_fields = list_response_field_set(&discovery_policy.list_return_fields)?;
                Ok(McpCatalogOutput::List(
                    items
                        .into_iter()
                        .map(|item| to_list_item_response(item, &return_fields))
                        .collect(),
                ))
            }
            McpCatalogInput::Export => {
                let export = service.export_catalog_for_actor(actor).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpCatalogOutput::Export(to_export_response(
                    export,
                    &operations,
                )?))
            }
        }
    }
}

impl ConsoleInterfacePort<McpCatalogInput, McpCatalogOutput> for McpCatalogAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, McpCatalogOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.view",
        binding_id: "http.console.mcp.catalog.get.v1",
        method: "GET",
        path: "/api/console/mcp/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.view",
        binding_id: "http.console.mcp.interfaces.get.v1",
        method: "GET",
        path: "/api/console/mcp/interface-capabilities",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.view",
        binding_id: "http.console.mcp.list.get.v1",
        method: "GET",
        path: "/api/console/mcp/list",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.catalog.export",
        binding_id: "http.console.mcp.export.get.v1",
        method: "GET",
        path: "/api/console/mcp/export",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: interface_catalog::McpInterfaceCatalogDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-catalog",
        "graph:console-mcp-catalog-v1",
        DECLARATIONS,
        Arc::new(McpCatalogAdapter(dependencies)),
    )
}
