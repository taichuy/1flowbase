use std::sync::Arc;

use control_plane::{
    network_egress::{CreateNetworkEgressProxyCommand, NetworkEgressProviderService},
    network_egress_pool::{
        AddProviderEgressesToPoolCommand, AddStaticHttpProxyToPoolCommand,
        CreateNetworkEgressPoolCommand, CreateNetworkEgressPoolMemberCommand,
        NetworkEgressPoolService, RecordNetworkEgressPoolMemberProbeCommand,
        UpdateNetworkEgressPoolCommand, UpdateNetworkEgressPoolMemberCommand,
    },
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::pools::*;
use crate::{
    error_response::ApiError,
    network_egress_probe::test_network_egress_connection,
    provider_runtime::ApiProviderRuntime,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum NetworkPoolsInput {
    List,
    CreateProxy(CreateNetworkEgressProxyBody),
    TestMember {
        pool_id: String,
        member_id: String,
    },
    Create(CreateNetworkEgressPoolBody),
    Update {
        pool_id: String,
        body: UpdateNetworkEgressPoolBody,
    },
    Delete {
        pool_id: String,
    },
    CreateMember {
        pool_id: String,
        body: CreateNetworkEgressPoolMemberBody,
    },
    AddStatic {
        pool_id: String,
        body: AddStaticHttpProxyToPoolBody,
    },
    AddProvider {
        pool_id: String,
        body: AddProviderEgressesToPoolBody,
    },
    UpdateMember {
        pool_id: String,
        member_id: String,
        body: UpdateNetworkEgressPoolMemberBody,
    },
    DeleteMember {
        pool_id: String,
        member_id: String,
    },
}

impl InterfaceContract for NetworkPoolsInput {
    const CONTRACT_ID: &'static str = "console-network-pools-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum NetworkPoolsOutput {
    Pools(Vec<NetworkEgressPoolResponse>),
    Pool(NetworkEgressPoolResponse),
    Provider(super::NetworkEgressProviderResponse),
    Member(NetworkEgressPoolMemberResponse),
    Members(Vec<NetworkEgressPoolMemberResponse>),
    Deleted,
}

impl InterfaceContract for NetworkPoolsOutput {
    const CONTRACT_ID: &'static str = "console-network-pools-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NetworkPoolsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
}

struct NetworkPoolsAdapter(NetworkPoolsDependencies);

impl NetworkPoolsAdapter {
    fn service(&self) -> crate::app_state::ApiNetworkEgressPoolService {
        NetworkEgressPoolService::with_secret_master_key(
            self.0.store.clone(),
            self.0.secret_key.clone(),
        )
    }

    fn proxy_service(&self) -> crate::app_state::ApiNetworkEgressProviderService {
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

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: NetworkPoolsInput,
    ) -> Result<NetworkPoolsOutput, ApiError> {
        let user_id = principal.actor().user_id;
        match input {
            NetworkPoolsInput::List => Ok(NetworkPoolsOutput::Pools(
                self.service()
                    .list_global(user_id)
                    .await?
                    .into_iter()
                    .map(response)
                    .collect(),
            )),
            NetworkPoolsInput::CreateProxy(body) => {
                Ok(NetworkPoolsOutput::Provider(super::response(
                    self.proxy_service()
                        .create_proxy(CreateNetworkEgressProxyCommand {
                            actor_user_id: user_id,
                            provider_code: body.provider_code,
                            display_name: body.display_name,
                            description: body.description,
                            config: body.config,
                        })
                        .await?,
                )))
            }
            NetworkPoolsInput::TestMember { pool_id, member_id } => {
                let pool_id = parse_uuid(&pool_id, "pool_id")?;
                let member_id = parse_uuid(&member_id, "member_id")?;
                let member = self.service().member(pool_id, member_id).await?;
                let probe = test_network_egress_connection(
                    self.0.store.clone(),
                    self.0.provider_runtime.clone(),
                    self.0.secret_key.clone(),
                    self.0.api_node_id.clone(),
                    control_plane::network_egress_pool::NetworkEgressPoolSelection {
                        pool_id,
                        member_id,
                        provider_id: member.provider_id,
                        provider_egress_key: member.provider_egress_key,
                    },
                )
                .await;
                let member = self
                    .service()
                    .record_probe(RecordNetworkEgressPoolMemberProbeCommand {
                        actor_user_id: user_id,
                        pool_id,
                        member_id,
                        status: probe.status,
                        http_status: probe.http_status,
                        https_status: probe.https_status,
                        latency_ms: probe.latency_ms,
                        exit_ip: probe.exit_ip,
                        exit_region: probe.exit_region,
                        error_code: probe.error_code,
                    })
                    .await?;
                Ok(NetworkPoolsOutput::Member(member_response(member)))
            }
            NetworkPoolsInput::Create(body) => Ok(NetworkPoolsOutput::Pool(response(
                self.service()
                    .create(CreateNetworkEgressPoolCommand {
                        actor_user_id: user_id,
                        display_name: body.display_name,
                    })
                    .await?,
            ))),
            NetworkPoolsInput::Update { pool_id, body } => Ok(NetworkPoolsOutput::Pool(response(
                self.service()
                    .update(UpdateNetworkEgressPoolCommand {
                        actor_user_id: user_id,
                        pool_id: parse_uuid(&pool_id, "pool_id")?,
                        display_name: body.display_name,
                    })
                    .await?,
            ))),
            NetworkPoolsInput::Delete { pool_id } => {
                self.service()
                    .delete(user_id, parse_uuid(&pool_id, "pool_id")?)
                    .await?;
                Ok(NetworkPoolsOutput::Deleted)
            }
            NetworkPoolsInput::CreateMember { pool_id, body } => {
                Ok(NetworkPoolsOutput::Member(member_response(
                    self.service()
                        .add_member(CreateNetworkEgressPoolMemberCommand {
                            actor_user_id: user_id,
                            pool_id: parse_uuid(&pool_id, "pool_id")?,
                            provider_id: parse_uuid(&body.provider_id, "provider_id")?,
                            provider_egress_key: body.provider_egress_key,
                            enabled: body.enabled,
                            sequence: body.sequence,
                        })
                        .await?,
                )))
            }
            NetworkPoolsInput::AddStatic { pool_id, body } => {
                Ok(NetworkPoolsOutput::Member(member_response(
                    self.service()
                        .add_static_http_proxy(AddStaticHttpProxyToPoolCommand {
                            actor_user_id: user_id,
                            pool_id: parse_uuid(&pool_id, "pool_id")?,
                            display_name: body.display_name,
                            host: body.host,
                            port: body.port,
                            username: body.username,
                            password: body.password,
                            enabled: body.enabled,
                            sequence: body.sequence,
                        })
                        .await?,
                )))
            }
            NetworkPoolsInput::AddProvider { pool_id, body } => Ok(NetworkPoolsOutput::Members(
                self.service()
                    .add_provider_egresses(AddProviderEgressesToPoolCommand {
                        actor_user_id: user_id,
                        pool_id: parse_uuid(&pool_id, "pool_id")?,
                        provider_id: parse_uuid(&body.provider_id, "provider_id")?,
                        enabled: body.enabled,
                        sequence: body.sequence,
                    })
                    .await?
                    .into_iter()
                    .map(member_response)
                    .collect(),
            )),
            NetworkPoolsInput::UpdateMember {
                pool_id,
                member_id,
                body,
            } => Ok(NetworkPoolsOutput::Member(member_response(
                self.service()
                    .update_member(UpdateNetworkEgressPoolMemberCommand {
                        actor_user_id: user_id,
                        pool_id: parse_uuid(&pool_id, "pool_id")?,
                        member_id: parse_uuid(&member_id, "member_id")?,
                        enabled: body.enabled,
                        sequence: body.sequence,
                    })
                    .await?,
            ))),
            NetworkPoolsInput::DeleteMember { pool_id, member_id } => {
                self.service()
                    .delete_member(
                        user_id,
                        parse_uuid(&pool_id, "pool_id")?,
                        parse_uuid(&member_id, "member_id")?,
                    )
                    .await?;
                Ok(NetworkPoolsOutput::Deleted)
            }
        }
    }
}

impl ConsoleInterfacePort<NetworkPoolsInput, NetworkPoolsOutput> for NetworkPoolsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NetworkPoolsInput,
    ) -> ConsoleInterfaceFuture<'a, NetworkPoolsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.list",
        binding_id: "http.console.network-egress-pools.list.v1",
        method: "GET",
        path: "/api/console/network-center/pools",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_proxies.create",
        binding_id: "http.console.network-egress-proxies.create.v1",
        method: "POST",
        path: "/api/console/network-center/pools/proxies",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.test_connection",
        binding_id: "http.console.network-egress-pool-members.test-connection.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members/:member_id/test-connection",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.create",
        binding_id: "http.console.network-egress-pools.create.v1",
        method: "POST",
        path: "/api/console/network-center/pools",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.update",
        binding_id: "http.console.network-egress-pools.update.v1",
        method: "PATCH",
        path: "/api/console/network-center/pools/:pool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pools.delete",
        binding_id: "http.console.network-egress-pools.delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/pools/:pool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.create",
        binding_id: "http.console.network-egress-pool-members.create.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.create_static_http",
        binding_id: "http.console.network-egress-pool-members.create-static-http.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members/static-http",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.add_provider_egresses",
        binding_id: "http.console.network-egress-pool-members.add-provider-egresses.v1",
        method: "POST",
        path: "/api/console/network-center/pools/:pool_id/members/provider",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.update",
        binding_id: "http.console.network-egress-pool-members.update.v1",
        method: "PATCH",
        path: "/api/console/network-center/pools/:pool_id/members/:member_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_pool_members.delete",
        binding_id: "http.console.network-egress-pool-members.delete.v1",
        method: "DELETE",
        path: "/api/console/network-center/pools/:pool_id/members/:member_id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: NetworkPoolsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-network-pools",
        "graph:console-network-pools-v1",
        DECLARATIONS,
        Arc::new(NetworkPoolsAdapter(dependencies)),
    )
}
