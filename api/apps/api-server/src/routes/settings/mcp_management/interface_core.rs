use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

#[derive(Clone)]
pub(crate) struct McpCoreDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) provider_secret_master_key: String,
}

pub(crate) enum McpCoreInput {
    GetCredential(String),
    SaveCredential(String, SaveMcpClientCredentialBody),
    DeleteCredential(String),
    ListInstances,
    CreateInstance(CreateMcpInstanceBody),
    CopyInstance(String, CopyMcpInstanceBody),
    UpdateInstance(String, CreateMcpInstanceBody),
    DeleteInstance(String),
    UpsertGroup(String, UpsertMcpGroupBody),
    MoveGroup(String, MoveMcpGroupBody),
    DeleteGroup(String, DeleteMcpGroupQuery),
    CreateBinding(String, CreateMcpToolBindingBody),
    UpdateBinding(String, UpdateMcpToolBindingBody),
    DeleteBinding(String),
    GetDiscoveryPolicy(String),
    UpdateDiscoveryPolicy(String, UpdateMcpInstanceDiscoveryPolicyBody),
}

impl InterfaceContract for McpCoreInput {
    const CONTRACT_ID: &'static str = "console-mcp-core-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpCoreOutput {
    Credential(McpClientCredentialResponse),
    Instances(Vec<McpInstanceResponse>),
    Instance(McpInstanceResponse),
    Group(McpGroupResponse),
    Binding(McpToolBindingResponse),
    DiscoveryPolicy(McpInstanceDiscoveryPolicyResponse),
    NoContent,
}

impl InterfaceContract for McpCoreOutput {
    const CONTRACT_ID: &'static str = "console-mcp-core-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpCoreAdapter(McpCoreDependencies);

impl McpCoreAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpCoreInput,
    ) -> Result<McpCoreOutput, ApiError> {
        let actor = principal.actor();
        let service = McpManagementService::new(self.0.store.clone());
        match input {
            McpCoreInput::GetCredential(instance_id) => {
                let api_key = service
                    .get_client_credential(
                        actor.user_id,
                        &instance_id,
                        &self.0.provider_secret_master_key,
                    )
                    .await?;
                Ok(McpCoreOutput::Credential(McpClientCredentialResponse {
                    saved: api_key.is_some(),
                    api_key,
                }))
            }
            McpCoreInput::SaveCredential(instance_id, body) => {
                service
                    .save_client_credential(SaveMcpClientCredentialCommand {
                        actor_user_id: actor.user_id,
                        instance_id,
                        api_key: body.api_key,
                        master_key: self.0.provider_secret_master_key.clone(),
                    })
                    .await?;
                Ok(McpCoreOutput::Credential(McpClientCredentialResponse {
                    saved: true,
                    api_key: None,
                }))
            }
            McpCoreInput::DeleteCredential(instance_id) => {
                service
                    .delete_client_credential(actor.user_id, &instance_id)
                    .await?;
                Ok(McpCoreOutput::NoContent)
            }
            McpCoreInput::ListInstances => {
                let snapshot = service.read_catalog_for_actor(actor).await?;
                Ok(McpCoreOutput::Instances(
                    snapshot
                        .instances
                        .into_iter()
                        .map(to_instance_response)
                        .collect(),
                ))
            }
            McpCoreInput::CreateInstance(body) => {
                let record = service
                    .create_instance_for_actor(actor, to_instance_command(actor.user_id, body)?)
                    .await?;
                Ok(McpCoreOutput::Instance(to_instance_response(record)))
            }
            McpCoreInput::CopyInstance(source_instance_id, body) => {
                let record = service
                    .copy_instance_for_actor(
                        actor,
                        CopyMcpInstanceCommand {
                            actor_user_id: actor.user_id,
                            source_instance_id,
                            instance_id: body.instance_id,
                            name: body.name,
                        },
                    )
                    .await?;
                Ok(McpCoreOutput::Instance(to_instance_response(record)))
            }
            McpCoreInput::UpdateInstance(instance_id, mut body) => {
                body.instance_id = instance_id;
                let record = service
                    .update_instance_for_actor(actor, to_instance_command(actor.user_id, body)?)
                    .await?;
                Ok(McpCoreOutput::Instance(to_instance_response(record)))
            }
            McpCoreInput::DeleteInstance(instance_id) => {
                service
                    .delete_instance_for_actor(actor, &instance_id)
                    .await?;
                Ok(McpCoreOutput::NoContent)
            }
            McpCoreInput::UpsertGroup(instance_id, body) => {
                let record = service
                    .upsert_group_for_actor(
                        actor,
                        UpsertMcpGroupCommand {
                            actor_user_id: actor.user_id,
                            instance_id,
                            path: body.path,
                            display_name: body.display_name.unwrap_or_default(),
                            description_short: body.description_short,
                            enabled: body.enabled,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(McpCoreOutput::Group(to_group_response(record)))
            }
            McpCoreInput::MoveGroup(instance_id, body) => {
                let record = service
                    .move_group_for_actor(
                        actor,
                        MoveMcpGroupCommand {
                            actor_user_id: actor.user_id,
                            instance_id,
                            source_path: body.source_path,
                            target_parent_path: body.target_parent_path,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(McpCoreOutput::Group(to_group_response(record)))
            }
            McpCoreInput::DeleteGroup(instance_id, query) => {
                service
                    .delete_group_for_actor(actor, &instance_id, &query.path)
                    .await?;
                Ok(McpCoreOutput::NoContent)
            }
            McpCoreInput::CreateBinding(instance_id, body) => {
                let record = service
                    .create_tool_binding_for_actor(
                        actor,
                        CreateMcpToolBindingCommand {
                            actor_user_id: actor.user_id,
                            instance_id,
                            group_path: body.group_path,
                            tool_id: body.tool_id,
                            display_alias: body.display_alias,
                            visible: body.visible,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(McpCoreOutput::Binding(to_binding_response(record)))
            }
            McpCoreInput::UpdateBinding(binding_id, body) => {
                let record = service
                    .update_tool_binding_for_actor(
                        actor,
                        UpdateMcpToolBindingCommand {
                            actor_user_id: actor.user_id,
                            binding_id: parse_uuid(&binding_id, "binding_id")?,
                            group_path: body.group_path,
                            display_alias: body.display_alias,
                            visible: body.visible,
                            sort_order: body.sort_order,
                        },
                    )
                    .await?;
                Ok(McpCoreOutput::Binding(to_binding_response(record)))
            }
            McpCoreInput::DeleteBinding(binding_id) => {
                service
                    .delete_tool_binding_for_actor(actor, parse_uuid(&binding_id, "binding_id")?)
                    .await?;
                Ok(McpCoreOutput::NoContent)
            }
            McpCoreInput::GetDiscoveryPolicy(instance_id) => {
                let record = service
                    .get_instance_discovery_policy(actor.user_id, &instance_id)
                    .await?;
                Ok(McpCoreOutput::DiscoveryPolicy(
                    to_discovery_policy_response(record, instance_id),
                ))
            }
            McpCoreInput::UpdateDiscoveryPolicy(instance_id, body) => {
                let record = service
                    .update_instance_discovery_policy_for_actor(
                        actor,
                        UpdateMcpInstanceDiscoveryPolicyCommand {
                            actor_user_id: actor.user_id,
                            instance_id: instance_id.clone(),
                            list_default_limit: body.list_default_limit,
                            list_max_depth: body.list_max_depth,
                            list_regex_enabled: body.list_regex_enabled,
                            list_regex_max_length: body.list_regex_max_length,
                            list_return_fields: body.list_return_fields,
                        },
                    )
                    .await?;
                Ok(McpCoreOutput::DiscoveryPolicy(
                    to_discovery_policy_response(record, instance_id),
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<McpCoreInput, McpCoreOutput> for McpCoreAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpCoreInput,
    ) -> ConsoleInterfaceFuture<'a, McpCoreOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.client_credential.reveal",
        binding_id: "http.console.mcp.client-credential.get.v1",
        method: "GET",
        path: "/api/console/mcp/instances/:instance_id/client-credential",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.client_credential.save",
        binding_id: "http.console.mcp.client-credential.put.v1",
        method: "PUT",
        path: "/api/console/mcp/instances/:instance_id/client-credential",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.client_credential.delete",
        binding_id: "http.console.mcp.client-credential.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/instances/:instance_id/client-credential",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.instances.view",
        binding_id: "http.console.mcp.instances.get.v1",
        method: "GET",
        path: "/api/console/mcp/instances",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.instances.create",
        binding_id: "http.console.mcp.instances.post.v1",
        method: "POST",
        path: "/api/console/mcp/instances",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.instances.copy",
        binding_id: "http.console.mcp.instances.copy.v1",
        method: "POST",
        path: "/api/console/mcp/instances/:instance_id/copy",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.instances.update",
        binding_id: "http.console.mcp.instances.put.v1",
        method: "PUT",
        path: "/api/console/mcp/instances/:instance_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.instances.delete",
        binding_id: "http.console.mcp.instances.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/instances/:instance_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.groups.upsert",
        binding_id: "http.console.mcp.groups.post.v1",
        method: "POST",
        path: "/api/console/mcp/instances/:instance_id/groups",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.groups.move",
        binding_id: "http.console.mcp.groups.move.v1",
        method: "POST",
        path: "/api/console/mcp/instances/:instance_id/groups/move",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.groups.delete",
        binding_id: "http.console.mcp.groups.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/instances/:instance_id/groups",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tool_bindings.create",
        binding_id: "http.console.mcp.bindings.post.v1",
        method: "POST",
        path: "/api/console/mcp/instances/:instance_id/tool-bindings",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tool_bindings.update",
        binding_id: "http.console.mcp.bindings.put.v1",
        method: "PUT",
        path: "/api/console/mcp/tool-bindings/:binding_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tool_bindings.delete",
        binding_id: "http.console.mcp.bindings.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/tool-bindings/:binding_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.discovery_policy.view",
        binding_id: "http.console.mcp.discovery-policy.get.v1",
        method: "GET",
        path: "/api/console/mcp/instances/:instance_id/discovery-policy",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.discovery_policy.update",
        binding_id: "http.console.mcp.discovery-policy.put.v1",
        method: "PUT",
        path: "/api/console/mcp/instances/:instance_id/discovery-policy",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: McpCoreDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-core",
        "graph:console-mcp-core-v1",
        DECLARATIONS,
        Arc::new(McpCoreAdapter(dependencies)),
    )
}
