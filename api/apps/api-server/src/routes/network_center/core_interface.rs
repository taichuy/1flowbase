use std::sync::Arc;

use control_plane::{
    network_egress::{
        CreateNetworkEgressProviderCommand, NetworkEgressProviderService,
        UpdateNetworkEgressProviderLifecycleCommand,
    },
    network_egress_route::{
        CreateNetworkEgressRouteCommand, NetworkEgressRouteService, UpdateNetworkEgressRouteCommand,
    },
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::{
    provider_runtime::ApiProviderRuntime,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum NetworkCenterInput {
    ListProviders,
    ListProviderTypes,
    CreateProvider(CreateNetworkEgressProviderBody),
    UpdateProviderLifecycle {
        id: String,
        body: UpdateNetworkEgressProviderLifecycleBody,
    },
    SyncProvider {
        id: String,
    },
    ListRoutes,
    CreateRoute(CreateNetworkEgressRouteBody),
    UpdateRoute {
        route_id: String,
        body: UpdateNetworkEgressRouteBody,
    },
    DeleteRoute {
        route_id: String,
    },
}

impl InterfaceContract for NetworkCenterInput {
    const CONTRACT_ID: &'static str = "console-network-center-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum NetworkCenterOutput {
    Providers(Vec<NetworkEgressProviderResponse>),
    ProviderTypes(Vec<NetworkEgressProviderTypeResponse>),
    Provider(NetworkEgressProviderResponse),
    Routes(Vec<NetworkEgressRouteResponse>),
    Route(NetworkEgressRouteResponse),
    Deleted,
}

impl InterfaceContract for NetworkCenterOutput {
    const CONTRACT_ID: &'static str = "console-network-center-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NetworkCenterDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
}

struct NetworkCenterAdapter(NetworkCenterDependencies);

impl NetworkCenterAdapter {
    fn provider_service(&self) -> crate::app_state::ApiNetworkEgressProviderService {
        NetworkEgressProviderService::new(
            self.0.store.clone(),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            ProviderRegistryNetworkEgressSecretResolver::new(
                self.0.store.clone(),
                self.0.secret_key.clone(),
            ),
            self.0.secret_key.clone(),
            self.0.api_node_id.clone(),
        )
    }

    fn route_service(&self) -> crate::app_state::ApiNetworkEgressRouteService {
        NetworkEgressRouteService::new(self.0.store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: NetworkCenterInput,
    ) -> Result<NetworkCenterOutput, ApiError> {
        let actor = principal.actor();
        match input {
            NetworkCenterInput::ListProviders => Ok(NetworkCenterOutput::Providers(
                self.provider_service()
                    .list()
                    .await?
                    .into_iter()
                    .map(response)
                    .collect(),
            )),
            NetworkCenterInput::ListProviderTypes => Ok(NetworkCenterOutput::ProviderTypes(
                self.provider_service()
                    .list_types()
                    .await?
                    .into_iter()
                    .map(type_response)
                    .collect(),
            )),
            NetworkCenterInput::CreateProvider(body) => {
                Ok(NetworkCenterOutput::Provider(response(
                    self.provider_service()
                        .create(CreateNetworkEgressProviderCommand {
                            actor_user_id: actor.user_id,
                            installation_id: parse_uuid(&body.installation_id, "installation_id")?,
                            display_name: body.display_name,
                            description: body.description,
                            secret_json: body.config,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::UpdateProviderLifecycle { id, body } => {
                Ok(NetworkCenterOutput::Provider(response(
                    self.provider_service()
                        .update_lifecycle(UpdateNetworkEgressProviderLifecycleCommand {
                            actor_user_id: actor.user_id,
                            provider_id: parse_uuid(&id, "provider_id")?,
                            lifecycle: lifecycle(&body.lifecycle)?,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::SyncProvider { id } => Ok(NetworkCenterOutput::Provider(response(
                self.provider_service()
                    .sync(actor.user_id, parse_uuid(&id, "provider_id")?)
                    .await?,
            ))),
            NetworkCenterInput::ListRoutes => Ok(NetworkCenterOutput::Routes(
                self.route_service()
                    .list(actor.current_workspace_id)
                    .await?
                    .into_iter()
                    .map(route_response)
                    .collect(),
            )),
            NetworkCenterInput::CreateRoute(body) => {
                Ok(NetworkCenterOutput::Route(route_response(
                    self.route_service()
                        .create(CreateNetworkEgressRouteCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            selector: consumer_selector(
                                body.consumer_kind,
                                body.consumer_reference,
                            )?,
                            pool_member_ids: parse_uuids(body.pool_member_ids, "pool_member_ids")?,
                            enabled: body.enabled,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::UpdateRoute { route_id, body } => {
                Ok(NetworkCenterOutput::Route(route_response(
                    self.route_service()
                        .update(UpdateNetworkEgressRouteCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            route_id: parse_uuid(&route_id, "route_id")?,
                            pool_member_ids: parse_uuids(body.pool_member_ids, "pool_member_ids")?,
                            enabled: body.enabled,
                        })
                        .await?,
                )))
            }
            NetworkCenterInput::DeleteRoute { route_id } => {
                self.route_service()
                    .delete(
                        actor.user_id,
                        actor.current_workspace_id,
                        parse_uuid(&route_id, "route_id")?,
                    )
                    .await?;
                Ok(NetworkCenterOutput::Deleted)
            }
        }
    }
}

impl ConsoleInterfacePort<NetworkCenterInput, NetworkCenterOutput> for NetworkCenterAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NetworkCenterInput,
    ) -> ConsoleInterfaceFuture<'a, NetworkCenterOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.list",
        binding_id: "http.console.network-egress-providers.list.v1",
        method: "GET",
        path: "/api/console/settings/network-center/providers",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_proxy_types.list",
        binding_id: "http.console.network-egress-provider-types.list.v1",
        method: "GET",
        path: "/api/console/settings/network-center/providers/types",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.create",
        binding_id: "http.console.network-egress-providers.create.v1",
        method: "POST",
        path: "/api/console/settings/network-center/providers",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.lifecycle.update",
        binding_id: "http.console.network-egress-providers.lifecycle.update.v1",
        method: "PATCH",
        path: "/api/console/settings/network-center/providers/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_providers.sync",
        binding_id: "http.console.network-egress-providers.sync.v1",
        method: "POST",
        path: "/api/console/settings/network-center/providers/:id/sync",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.list",
        binding_id: "http.console.network-egress-routes.list.v1",
        method: "GET",
        path: "/api/console/network-center/routes",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.create",
        binding_id: "http.console.network-egress-routes.create.v1",
        method: "POST",
        path: "/api/console/network-center/routes",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.update",
        binding_id: "http.console.network-egress-routes.update.v1",
        method: "PATCH",
        path: "/api/console/network-center/routes/:route_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_routes.delete",
        binding_id: "http.console.network-egress-routes.delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/routes/:route_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: NetworkCenterDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-network-center",
        "graph:console-network-center-v1",
        DECLARATIONS,
        Arc::new(NetworkCenterAdapter(dependencies)),
    )
}
