use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::callable_interfaces::FrontstageInterfaceCapabilityQuery;
use crate::{
    error_response::ApiError,
    openapi_interface::{
        get_openapi_capability_with, query_openapi_capability_catalog_with,
        OpenApiCapabilityCatalogDependencies, OpenApiCapabilityCatalogEntry,
        OpenApiCapabilityCatalogPage, OpenApiCapabilityCatalogQuery,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

const INTERFACE_CAPABILITY_PAGE_SIZE: usize = 20;

pub(crate) enum FrontstageCallableCatalogInput {
    List(FrontstageInterfaceCapabilityQuery),
    Get { interface_id: String },
}

impl InterfaceContract for FrontstageCallableCatalogInput {
    const CONTRACT_ID: &'static str = "console-frontstage-callable-catalog-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum FrontstageCallableCatalogOutput {
    Page(OpenApiCapabilityCatalogPage),
    Entry(OpenApiCapabilityCatalogEntry),
}

impl InterfaceContract for FrontstageCallableCatalogOutput {
    const CONTRACT_ID: &'static str = "console-frontstage-callable-catalog-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct FrontstageCallableCatalogDependencies {
    pub(crate) openapi: OpenApiCapabilityCatalogDependencies,
}

struct FrontstageCallableCatalogAdapter(FrontstageCallableCatalogDependencies);

impl FrontstageCallableCatalogAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontstageCallableCatalogInput,
    ) -> Result<FrontstageCallableCatalogOutput, ApiError> {
        let actor = principal.actor();
        if !actor.has_permission("frontstage.page.design") {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "frontstage.page.design",
            )
            .into());
        }
        match input {
            FrontstageCallableCatalogInput::List(query) => {
                Ok(FrontstageCallableCatalogOutput::Page(
                    query_openapi_capability_catalog_with(
                        &self.0.openapi,
                        actor.current_workspace_id,
                        OpenApiCapabilityCatalogQuery {
                            path_prefixes: query.path_prefixes,
                            path_query: query.path_query,
                            adapter_id: query.adapter_id,
                            method: query.method,
                            offset: query.offset.unwrap_or(0),
                            limit: query
                                .limit
                                .unwrap_or(INTERFACE_CAPABILITY_PAGE_SIZE)
                                .clamp(1, INTERFACE_CAPABILITY_PAGE_SIZE),
                        },
                    )
                    .await?,
                ))
            }
            FrontstageCallableCatalogInput::Get { interface_id } => {
                let entry = get_openapi_capability_with(
                    &self.0.openapi,
                    actor.current_workspace_id,
                    &interface_id,
                )
                .await?
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "frontstage_interface_capability",
                ))?;
                Ok(FrontstageCallableCatalogOutput::Entry(entry))
            }
        }
    }
}

impl ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>
    for FrontstageCallableCatalogAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstageCallableCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageCallableCatalogOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) fn port(
    dependencies: FrontstageCallableCatalogDependencies,
) -> Arc<dyn ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>>
{
    Arc::new(FrontstageCallableCatalogAdapter(dependencies))
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.callable_interfaces.list",
        binding_id: "http.console.frontstage.interface-capabilities.list.get.v1",
        method: "GET",
        path: "/api/console/frontstage/interface-capabilities",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.callable_interfaces.view",
        binding_id: "http.console.frontstage.interface-capabilities.detail.get.v1",
        method: "GET",
        path: "/api/console/frontstage/interface-capabilities/:interface_id",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<
        dyn ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-callable-catalog",
        "graph:console-frontstage-callable-catalog-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableFrontstageCallableCatalogPort;

#[cfg(test)]
impl ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>
    for UnavailableFrontstageCallableCatalogPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: FrontstageCallableCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageCallableCatalogOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("frontstage callable catalog fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f12c1_registry_freezes_callable_catalog_bindings() {
        let registry =
            compile_registry(Arc::new(UnavailableFrontstageCallableCatalogPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared callable catalog binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
