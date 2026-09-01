use std::{collections::HashMap, sync::Arc};

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum McpToolsInput {
    List,
    Create(CreateMcpToolBody),
    Get(String),
    Update(String, UpdateMcpToolBody),
    Delete(String),
    RefreshDescription(String),
    CheckDescription(String, McpDescriptionCheckBody),
}

impl InterfaceContract for McpToolsInput {
    const CONTRACT_ID: &'static str = "console-mcp-tools-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpToolsOutput {
    Tools(Vec<McpToolResponse>),
    Tool(McpToolResponse),
    Check(McpDescriptionCheckResponse),
    NoContent,
}

impl InterfaceContract for McpToolsOutput {
    const CONTRACT_ID: &'static str = "console-mcp-tools-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpToolsAdapter(interface_catalog::McpInterfaceCatalogDependencies);

impl McpToolsAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpToolsInput,
    ) -> Result<McpToolsOutput, ApiError> {
        let actor = principal.actor();
        let service = McpManagementService::new(self.0.store.clone());
        match input {
            McpToolsInput::List => {
                let snapshot = service.read_catalog_for_actor(actor).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                let mut tools = Vec::with_capacity(snapshot.tools.len());
                for record in snapshot.tools {
                    tools.push(
                        to_tool_response_for_actor(&self.0.store, actor, record, &operations)
                            .await?,
                    );
                }
                Ok(McpToolsOutput::Tools(tools))
            }
            McpToolsInput::Create(body) => {
                let interface_id = interface_target_id(&body.execution_target)?;
                let interface_entry =
                    interface_catalog::bindable_mcp_interface_with(&self.0, actor, interface_id)
                        .await?;
                let operation = interface_operation(&interface_entry);
                let record = service
                    .create_tool_for_actor(
                        actor,
                        to_create_tool_command(actor.user_id, body, interface_entry)?,
                    )
                    .await?;
                Ok(McpToolsOutput::Tool(to_tool_response_with_operation(
                    record,
                    operation,
                    domain::McpToolAvailabilityStatus::Available,
                )))
            }
            McpToolsInput::Get(tool_id) => {
                let record = service.get_tool(actor.user_id, &tool_id).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpToolsOutput::Tool(
                    to_tool_response_for_actor(&self.0.store, actor, record, &operations).await?,
                ))
            }
            McpToolsInput::Update(tool_id, body) => match &body.execution_target {
                McpToolExecutionTargetDto::InterfaceWrapper { interface_id } => {
                    let interface_entry = interface_catalog::bindable_mcp_interface_with(
                        &self.0,
                        actor,
                        interface_id,
                    )
                    .await?;
                    let operation = interface_operation(&interface_entry);
                    let record = service
                        .update_tool_for_actor(
                            actor,
                            to_update_tool_command(actor.user_id, tool_id, body, interface_entry)?,
                        )
                        .await?;
                    Ok(McpToolsOutput::Tool(to_tool_response_with_operation(
                        record,
                        operation,
                        domain::McpToolAvailabilityStatus::Available,
                    )))
                }
                McpToolExecutionTargetDto::McpProxy { .. } => {
                    let execution_target = to_domain_execution_target(&body.execution_target)?;
                    let record = service
                        .update_proxy_tool_for_actor(
                            actor,
                            UpdateMcpProxyToolCommand {
                                actor_user_id: actor.user_id,
                                tool_id,
                                des_id: body.des_id,
                                name: body.name,
                                short_description: body.short_description,
                                full_description: body.full_description.unwrap_or_default(),
                                execution_target,
                                parameter_schema: body.parameter_schema,
                                result_schema: body.result_schema,
                                input_mapping: body.input_mapping,
                                output_mapping: body.output_mapping,
                                risk_level: parse_risk_level(&body.risk_level)?,
                                status: parse_tool_status(&body.status)?,
                            },
                        )
                        .await?;
                    let operations =
                        interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                    Ok(McpToolsOutput::Tool(
                        to_tool_response_for_actor(&self.0.store, actor, record, &operations)
                            .await?,
                    ))
                }
                McpToolExecutionTargetDto::AssistantClient { .. } => {
                    let execution_target = to_domain_execution_target(&body.execution_target)?;
                    let record = service
                        .update_proxy_tool_for_actor(
                            actor,
                            UpdateMcpProxyToolCommand {
                                actor_user_id: actor.user_id,
                                tool_id,
                                des_id: body.des_id,
                                name: body.name,
                                short_description: body.short_description,
                                full_description: body.full_description.unwrap_or_default(),
                                execution_target,
                                parameter_schema: body.parameter_schema,
                                result_schema: body.result_schema,
                                input_mapping: body.input_mapping,
                                output_mapping: body.output_mapping,
                                risk_level: parse_risk_level(&body.risk_level)?,
                                status: parse_tool_status(&body.status)?,
                            },
                        )
                        .await?;
                    Ok(McpToolsOutput::Tool(to_tool_response(
                        record,
                        &HashMap::new(),
                    )))
                }
            },
            McpToolsInput::Delete(tool_id) => {
                service.delete_tool_for_actor(actor, &tool_id).await?;
                Ok(McpToolsOutput::NoContent)
            }
            McpToolsInput::RefreshDescription(tool_id) => {
                let record = service
                    .refresh_tool_description_for_actor(
                        actor,
                        RefreshMcpToolDescriptionCommand {
                            actor_user_id: actor.user_id,
                            tool_id,
                        },
                    )
                    .await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpToolsOutput::Tool(to_tool_response(record, &operations)))
            }
            McpToolsInput::CheckDescription(tool_id, body) => {
                let result = service
                    .description_check(actor.user_id, &tool_id, body.des_id.as_deref())
                    .await?;
                Ok(McpToolsOutput::Check(McpDescriptionCheckResponse {
                    accepted: result.accepted,
                    current_des_id: result.current_des_id,
                }))
            }
        }
    }
}

impl ConsoleInterfacePort<McpToolsInput, McpToolsOutput> for McpToolsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpToolsInput,
    ) -> ConsoleInterfaceFuture<'a, McpToolsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.view",
        binding_id: "http.console.mcp.tools.get.v1",
        method: "GET",
        path: "/api/console/mcp/tools",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.create",
        binding_id: "http.console.mcp.tools.post.v1",
        method: "POST",
        path: "/api/console/mcp/tools",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.view",
        binding_id: "http.console.mcp.tool.get.v1",
        method: "GET",
        path: "/api/console/mcp/tools/:tool_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.update",
        binding_id: "http.console.mcp.tool.put.v1",
        method: "PUT",
        path: "/api/console/mcp/tools/:tool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.delete",
        binding_id: "http.console.mcp.tool.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/tools/:tool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.description.refresh",
        binding_id: "http.console.mcp.tool-description.refresh.v1",
        method: "POST",
        path: "/api/console/mcp/tools/:tool_id/description/refresh",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.description.check",
        binding_id: "http.console.mcp.tool-description.check.v1",
        method: "POST",
        path: "/api/console/mcp/tools/:tool_id/description-check",
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
        "api-server.console-mcp-tools",
        "graph:console-mcp-tools-v1",
        DECLARATIONS,
        Arc::new(McpToolsAdapter(dependencies)),
    )
}
