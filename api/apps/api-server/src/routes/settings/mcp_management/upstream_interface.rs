use std::sync::Arc;

use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    mcp_management::{
        McpManagementService, McpRemoteToolDefinition, McpUpstreamCredential,
        RecordMcpUpstreamDiscoveryCommand, SaveMcpUpstreamConnectionCommand,
        SaveMcpUpstreamCredentialCommand,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use super::{
    to_tool_response_with_operation,
    upstream::{
        DebugMcpProxyToolBody, DebugMcpProxyToolResponse, ImportMcpUpstreamToolsBody,
        McpUpstreamConnectionResponse, McpUpstreamDiscoverResponse, McpUpstreamDraftTestResponse,
        McpUpstreamTestResponse, McpUpstreamToolResponse, SaveMcpUpstreamConnectionBody,
        SaveMcpUpstreamCredentialBody, TestMcpUpstreamConnectionDraftBody,
    },
    upstream_client::{McpDiscoveryResult, McpProxyExecutionTrace, McpUpstreamServerInfo},
    McpToolResponse,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

#[async_trait]
pub(crate) trait McpUpstreamTransportPort: Send + Sync {
    async fn test_configuration(
        &self,
        endpoint: &str,
        auth_type: domain::McpUpstreamAuthType,
        custom_header_name: Option<&str>,
        secret: Option<&Value>,
    ) -> Result<McpUpstreamServerInfo, McpUpstreamTransportError>;

    async fn test_connection(
        &self,
        connection: &domain::McpUpstreamConnectionRecord,
        secret: Option<&Value>,
    ) -> Result<McpUpstreamServerInfo, McpUpstreamTransportError>;

    async fn discover(
        &self,
        connection: &domain::McpUpstreamConnectionRecord,
        secret: Option<&Value>,
    ) -> Result<McpDiscoveryResult, McpUpstreamTransportError>;

    async fn execute_proxy(
        &self,
        connection: &domain::McpUpstreamConnectionRecord,
        secret: Option<&Value>,
        remote_tool_name: &str,
        arguments: Value,
        input_mapping: &Value,
        output_mapping: &Value,
    ) -> Result<McpProxyExecutionTrace, McpProxyTransportError>;
}

#[derive(Debug)]
pub(crate) struct McpUpstreamTransportError(String);

impl McpUpstreamTransportError {
    pub(crate) fn new(message: String) -> Self {
        Self(message)
    }
}

impl std::fmt::Display for McpUpstreamTransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub(crate) enum McpProxyTransportError {
    Connection,
    Execution,
}

#[derive(Clone)]
pub(crate) struct McpUpstreamDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) provider_secret_master_key: String,
    pub(crate) transport: Arc<dyn McpUpstreamTransportPort>,
}

pub(crate) enum McpUpstreamInput {
    List,
    Create(SaveMcpUpstreamConnectionBody),
    Update {
        connection_id: String,
        body: SaveMcpUpstreamConnectionBody,
    },
    Delete(String),
    SaveCredentials {
        connection_id: String,
        body: SaveMcpUpstreamCredentialBody,
    },
    DeleteCredentials(String),
    TestDraft(TestMcpUpstreamConnectionDraftBody),
    Test(String),
    Discover(String),
    Import {
        connection_id: String,
        body: ImportMcpUpstreamToolsBody,
    },
    Debug {
        tool_id: String,
        body: DebugMcpProxyToolBody,
    },
}

impl InterfaceContract for McpUpstreamInput {
    const CONTRACT_ID: &'static str = "console-mcp-upstream-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum McpUpstreamOutput {
    Connections(Vec<McpUpstreamConnectionResponse>),
    Created(McpUpstreamConnectionResponse),
    Connection(McpUpstreamConnectionResponse),
    DraftTest(McpUpstreamDraftTestResponse),
    Test(McpUpstreamTestResponse),
    Discovery(McpUpstreamDiscoverResponse),
    Imported(Vec<McpToolResponse>),
    Debug(DebugMcpProxyToolResponse),
    NoContent,
}

impl InterfaceContract for McpUpstreamOutput {
    const CONTRACT_ID: &'static str = "console-mcp-upstream-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpUpstreamAdapter(McpUpstreamDependencies);

pub(crate) fn port(
    dependencies: McpUpstreamDependencies,
) -> Arc<dyn ConsoleInterfacePort<McpUpstreamInput, McpUpstreamOutput>> {
    Arc::new(McpUpstreamAdapter(dependencies))
}

impl McpUpstreamAdapter {
    fn service(&self) -> McpManagementService<storage_durable_postgres::MainDurableStore> {
        McpManagementService::new(self.0.store.clone())
    }

    async fn test_draft(
        &self,
        principal: &UserPrincipal,
        body: TestMcpUpstreamConnectionDraftBody,
    ) -> Result<McpUpstreamDraftTestResponse, ApiError> {
        parse_upstream_transport(&body.transport)?;
        let auth_type = parse_upstream_auth_type(&body.auth_type)?;
        let service = self.service();
        let stored_secret = if let Some(connection_id) = body.connection_id.as_deref() {
            let (_, secret) = service
                .prepare_upstream_management_action(
                    principal.actor().user_id,
                    parse_connection_id(connection_id)?,
                    &self.0.provider_secret_master_key,
                )
                .await?;
            secret
        } else {
            None
        };
        let provided_secret = match (auth_type, body.credential) {
            (domain::McpUpstreamAuthType::None, None) => None,
            (
                domain::McpUpstreamAuthType::Bearer,
                Some(SaveMcpUpstreamCredentialBody::Bearer { token }),
            ) if !token.trim().is_empty() => Some(serde_json::json!({ "token": token })),
            (
                domain::McpUpstreamAuthType::CustomHeader,
                Some(SaveMcpUpstreamCredentialBody::CustomHeader {
                    header_name,
                    header_value,
                }),
            ) if body.custom_header_name.as_deref() == Some(header_name.as_str())
                && !header_value.is_empty() =>
            {
                Some(serde_json::json!({
                    "header_name": header_name,
                    "header_value": header_value,
                }))
            }
            (_, None) => None,
            _ => return Err(ControlPlaneError::InvalidInput("credential").into()),
        };
        let secret = provided_secret.or(stored_secret);
        let tested_at = OffsetDateTime::now_utc();
        let tested_at_response = format_timestamp(tested_at)?;
        let response = match self
            .0
            .transport
            .test_configuration(
                &body.endpoint,
                auth_type,
                body.custom_header_name.as_deref(),
                secret.as_ref(),
            )
            .await
        {
            Ok(server) => McpUpstreamDraftTestResponse {
                ok: true,
                server_name: server.name,
                server_version: server.version,
                protocol_version: Some(server.protocol_version),
                tested_at: tested_at_response,
                error: None,
            },
            Err(error) => McpUpstreamDraftTestResponse {
                ok: false,
                server_name: None,
                server_version: None,
                protocol_version: None,
                tested_at: tested_at_response,
                error: Some(error.to_string()),
            },
        };
        Ok(response)
    }

    async fn test_connection(
        &self,
        principal: &UserPrincipal,
        connection_id: Uuid,
    ) -> Result<McpUpstreamTestResponse, ApiError> {
        let service = self.service();
        let (connection, secret) = service
            .prepare_upstream_management_action(
                principal.actor().user_id,
                connection_id,
                &self.0.provider_secret_master_key,
            )
            .await?;
        let tested_at = OffsetDateTime::now_utc();
        let tested_at_response = format_timestamp(tested_at)?;
        let (response, last_error) = match self
            .0
            .transport
            .test_connection(&connection, secret.as_ref())
            .await
        {
            Ok(server) => (
                McpUpstreamTestResponse {
                    connection_id: connection_id.to_string(),
                    ok: true,
                    server_name: server.name,
                    server_version: server.version,
                    protocol_version: Some(server.protocol_version),
                    tested_at: tested_at_response.clone(),
                    error: None,
                },
                None,
            ),
            Err(error) => {
                let error = error.to_string();
                (
                    McpUpstreamTestResponse {
                        connection_id: connection_id.to_string(),
                        ok: false,
                        server_name: None,
                        server_version: None,
                        protocol_version: None,
                        tested_at: tested_at_response,
                        error: Some(error.clone()),
                    },
                    Some(error),
                )
            }
        };
        service
            .record_upstream_result(
                principal.actor().user_id,
                connection_id,
                response.ok.then_some(tested_at),
                None,
                last_error.as_deref(),
            )
            .await?;
        Ok(response)
    }

    async fn discover(
        &self,
        principal: &UserPrincipal,
        connection_id: Uuid,
    ) -> Result<McpUpstreamDiscoverResponse, ApiError> {
        let service = self.service();
        let (connection, secret) = service
            .prepare_upstream_management_action(
                principal.actor().user_id,
                connection_id,
                &self.0.provider_secret_master_key,
            )
            .await?;
        let discovery = self
            .0
            .transport
            .discover(&connection, secret.as_ref())
            .await
            .map_err(|_| ControlPlaneError::UpstreamUnavailable("mcp_discovery"))?;
        let discovered_at = OffsetDateTime::now_utc();
        let sources = service
            .record_upstream_discovery(RecordMcpUpstreamDiscoveryCommand {
                actor_user_id: principal.actor().user_id,
                connection_id,
                discovered_at,
                tools: discovery
                    .tools
                    .into_iter()
                    .map(|tool| McpRemoteToolDefinition {
                        remote_tool_name: tool.name,
                        description: tool.description,
                        input_schema: tool.input_schema,
                        output_schema: tool.output_schema,
                        schema_hash: tool.schema_hash,
                    })
                    .collect(),
            })
            .await?;
        Ok(McpUpstreamDiscoverResponse {
            connection_id: connection_id.to_string(),
            server_name: discovery.server.name,
            server_version: discovery.server.version,
            protocol_version: discovery.server.protocol_version,
            discovered_at: format_timestamp(discovered_at)?,
            items: sources.into_iter().map(to_tool_source_response).collect(),
        })
    }

    async fn debug(
        &self,
        principal: &UserPrincipal,
        tool_id: String,
        body: DebugMcpProxyToolBody,
    ) -> Result<DebugMcpProxyToolResponse, ApiError> {
        let service = self.service();
        let tool = service
            .get_tool(principal.actor().user_id, &tool_id)
            .await?;
        let domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } = &tool.execution_target
        else {
            return Err(ControlPlaneError::InvalidInput("execution_target").into());
        };
        let (connection, secret) = service
            .prepare_upstream_management_action(
                principal.actor().user_id,
                *upstream_connection_id,
                &self.0.provider_secret_master_key,
            )
            .await?;
        if service
            .upstream_proxy_availability(
                principal.actor().user_id,
                *upstream_connection_id,
                remote_tool_name,
            )
            .await?
            != domain::McpToolAvailabilityStatus::Available
        {
            return Err(ControlPlaneError::UpstreamUnavailable("mcp_proxy_unavailable").into());
        }
        let trace = self
            .0
            .transport
            .execute_proxy(
                &connection,
                secret.as_ref(),
                remote_tool_name,
                body.arguments,
                &tool.input_mapping,
                &tool.output_mapping,
            )
            .await
            .map_err(|error| match error {
                McpProxyTransportError::Connection => {
                    ControlPlaneError::UpstreamUnavailable("mcp_connection")
                }
                McpProxyTransportError::Execution => {
                    ControlPlaneError::UpstreamUnavailable("mcp_tools_call")
                }
            })?;
        Ok(DebugMcpProxyToolResponse {
            local_arguments: trace.local_arguments,
            remote_arguments: trace.remote_arguments,
            upstream_result: serde_json::to_value(trace.upstream_result).map_err(ApiError::from)?,
            mapped_result: serde_json::to_value(trace.mapped_result).map_err(ApiError::from)?,
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpUpstreamInput,
    ) -> Result<McpUpstreamOutput, ApiError> {
        let actor_user_id = principal.actor().user_id;
        match input {
            McpUpstreamInput::List => Ok(McpUpstreamOutput::Connections(
                self.service()
                    .list_upstream_connections(actor_user_id)
                    .await?
                    .into_iter()
                    .map(to_connection_response)
                    .collect::<Result<_, _>>()?,
            )),
            McpUpstreamInput::Create(body) => {
                Ok(McpUpstreamOutput::Created(to_connection_response(
                    self.service()
                        .save_upstream_connection(connection_command(actor_user_id, None, body)?)
                        .await?,
                )?))
            }
            McpUpstreamInput::Update {
                connection_id,
                body,
            } => Ok(McpUpstreamOutput::Connection(to_connection_response(
                self.service()
                    .save_upstream_connection(connection_command(
                        actor_user_id,
                        Some(parse_connection_id(&connection_id)?),
                        body,
                    )?)
                    .await?,
            )?)),
            McpUpstreamInput::Delete(connection_id) => {
                self.service()
                    .delete_upstream_connection(actor_user_id, parse_connection_id(&connection_id)?)
                    .await?;
                Ok(McpUpstreamOutput::NoContent)
            }
            McpUpstreamInput::SaveCredentials {
                connection_id,
                body,
            } => {
                let credential = match body {
                    SaveMcpUpstreamCredentialBody::Bearer { token } => {
                        McpUpstreamCredential::Bearer { token }
                    }
                    SaveMcpUpstreamCredentialBody::CustomHeader {
                        header_name,
                        header_value,
                    } => McpUpstreamCredential::CustomHeader {
                        header_name,
                        header_value,
                    },
                };
                self.service()
                    .save_upstream_credential(SaveMcpUpstreamCredentialCommand {
                        actor_user_id,
                        connection_id: parse_connection_id(&connection_id)?,
                        credential,
                        master_key: self.0.provider_secret_master_key.clone(),
                    })
                    .await?;
                Ok(McpUpstreamOutput::NoContent)
            }
            McpUpstreamInput::DeleteCredentials(connection_id) => {
                self.service()
                    .delete_upstream_credential(actor_user_id, parse_connection_id(&connection_id)?)
                    .await?;
                Ok(McpUpstreamOutput::NoContent)
            }
            McpUpstreamInput::TestDraft(body) => Ok(McpUpstreamOutput::DraftTest(
                self.test_draft(principal, body).await?,
            )),
            McpUpstreamInput::Test(connection_id) => Ok(McpUpstreamOutput::Test(
                self.test_connection(principal, parse_connection_id(&connection_id)?)
                    .await?,
            )),
            McpUpstreamInput::Discover(connection_id) => Ok(McpUpstreamOutput::Discovery(
                self.discover(principal, parse_connection_id(&connection_id)?)
                    .await?,
            )),
            McpUpstreamInput::Import {
                connection_id,
                body,
            } => {
                let records = self
                    .service()
                    .import_upstream_tools(
                        actor_user_id,
                        parse_connection_id(&connection_id)?,
                        &body.remote_tool_names,
                    )
                    .await?;
                let mut responses = Vec::with_capacity(records.len());
                for record in records {
                    let domain::McpToolExecutionTarget::McpProxy {
                        upstream_connection_id,
                        remote_tool_name,
                        ..
                    } = &record.execution_target
                    else {
                        return Err(ControlPlaneError::InvalidInput("execution_target").into());
                    };
                    let upstream_connection_id = *upstream_connection_id;
                    let remote_tool_name = remote_tool_name.clone();
                    let availability = self
                        .service()
                        .upstream_proxy_availability(
                            actor_user_id,
                            upstream_connection_id,
                            &remote_tool_name,
                        )
                        .await?;
                    responses.push(to_tool_response_with_operation(
                        record,
                        format!("MCP tools/call {remote_tool_name}"),
                        availability,
                    ));
                }
                Ok(McpUpstreamOutput::Imported(responses))
            }
            McpUpstreamInput::Debug { tool_id, body } => Ok(McpUpstreamOutput::Debug(
                self.debug(principal, tool_id, body).await?,
            )),
        }
    }
}

impl ConsoleInterfacePort<McpUpstreamInput, McpUpstreamOutput> for McpUpstreamAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpUpstreamInput,
    ) -> ConsoleInterfaceFuture<'a, McpUpstreamOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

fn connection_command(
    actor_user_id: Uuid,
    connection_id: Option<Uuid>,
    body: SaveMcpUpstreamConnectionBody,
) -> Result<SaveMcpUpstreamConnectionCommand, ApiError> {
    let transport = parse_upstream_transport(&body.transport)?;
    let auth_type = parse_upstream_auth_type(&body.auth_type)?;
    let status = match body.status.as_str() {
        "enabled" => domain::McpUpstreamConnectionStatus::Enabled,
        "disabled" => domain::McpUpstreamConnectionStatus::Disabled,
        _ => return Err(ControlPlaneError::InvalidInput("status").into()),
    };
    Ok(SaveMcpUpstreamConnectionCommand {
        actor_user_id,
        connection_id,
        name: body.name,
        endpoint: body.endpoint,
        transport,
        auth_type,
        custom_header_name: body.custom_header_name,
        status,
    })
}

fn parse_upstream_transport(value: &str) -> Result<domain::McpUpstreamTransport, ApiError> {
    match value {
        "streamable_http" => Ok(domain::McpUpstreamTransport::StreamableHttp),
        _ => Err(ControlPlaneError::InvalidInput("transport").into()),
    }
}

fn parse_upstream_auth_type(value: &str) -> Result<domain::McpUpstreamAuthType, ApiError> {
    match value {
        "none" => Ok(domain::McpUpstreamAuthType::None),
        "bearer" => Ok(domain::McpUpstreamAuthType::Bearer),
        "custom_header" => Ok(domain::McpUpstreamAuthType::CustomHeader),
        _ => Err(ControlPlaneError::InvalidInput("auth_type").into()),
    }
}

fn parse_connection_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| ControlPlaneError::InvalidInput("connection_id").into())
}

fn to_connection_response(
    record: domain::McpUpstreamConnectionRecord,
) -> Result<McpUpstreamConnectionResponse, time::error::Format> {
    let credentials_status = match record.auth_type {
        domain::McpUpstreamAuthType::None => "not_required",
        _ if record.credentials_configured => "configured",
        _ => "missing",
    };
    Ok(McpUpstreamConnectionResponse {
        connection_id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        name: record.name,
        endpoint: record.endpoint,
        transport: record.transport.as_str().into(),
        auth_type: record.auth_type.as_str().into(),
        custom_header_name: record.custom_header_name,
        status: record.status.as_str().into(),
        credentials_status: credentials_status.into(),
        last_connected_at: record.last_connected_at.map(format_timestamp).transpose()?,
        last_discovered_at: record
            .last_discovered_at
            .map(format_timestamp)
            .transpose()?,
        last_error: record.last_error,
        created_at: format_timestamp(record.created_at)?,
        updated_at: format_timestamp(record.updated_at)?,
    })
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, time::error::Format> {
    value.format(&Rfc3339)
}

fn to_tool_source_response(record: domain::McpUpstreamToolSourceRecord) -> McpUpstreamToolResponse {
    McpUpstreamToolResponse {
        remote_tool_name: record.remote_tool_name,
        description: record.description,
        input_schema: record.input_schema,
        output_schema: record.output_schema,
        source_status: record.source_status.as_str().into(),
        imported_tool_id: record.imported_tool_id,
        schema_hash: record.schema_hash,
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.view",
        binding_id: "http.console.mcp.upstream-connections.list.v1",
        method: "GET",
        path: "/api/console/mcp/upstream-connections",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.create",
        binding_id: "http.console.mcp.upstream-connections.create.v1",
        method: "POST",
        path: "/api/console/mcp/upstream-connections",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.update",
        binding_id: "http.console.mcp.upstream-connections.update.v1",
        method: "PUT",
        path: "/api/console/mcp/upstream-connections/:connection_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.delete",
        binding_id: "http.console.mcp.upstream-connections.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/upstream-connections/:connection_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_credentials.update",
        binding_id: "http.console.mcp.upstream-credentials.save.v1",
        method: "PUT",
        path: "/api/console/mcp/upstream-connections/:connection_id/credentials",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_credentials.delete",
        binding_id: "http.console.mcp.upstream-credentials.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/upstream-connections/:connection_id/credentials",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.test",
        binding_id: "http.console.mcp.upstream-connections.test-draft.v1",
        method: "POST",
        path: "/api/console/mcp/upstream-connections/test",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.test",
        binding_id: "http.console.mcp.upstream-connections.test.v1",
        method: "POST",
        path: "/api/console/mcp/upstream-connections/:connection_id/test",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_connections.discover",
        binding_id: "http.console.mcp.upstream-connections.discover.v1",
        method: "POST",
        path: "/api/console/mcp/upstream-connections/:connection_id/discover",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_tools.import",
        binding_id: "http.console.mcp.upstream-connections.import.v1",
        method: "POST",
        path: "/api/console/mcp/upstream-connections/:connection_id/imports",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.upstream_tools.debug",
        binding_id: "http.console.mcp.tools.debug.v1",
        method: "POST",
        path: "/api/console/mcp/tools/:tool_id/debug",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<McpUpstreamInput, McpUpstreamOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-upstream",
        "graph:console-mcp-upstream-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableMcpUpstreamPort;

#[cfg(test)]
impl ConsoleInterfacePort<McpUpstreamInput, McpUpstreamOutput> for UnavailableMcpUpstreamPort {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        _: McpUpstreamInput,
    ) -> ConsoleInterfaceFuture<'a, McpUpstreamOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("MCP upstream fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f11b_registry_freezes_mcp_upstream_bindings() {
        let registry = compile_registry(Arc::new(UnavailableMcpUpstreamPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared MCP upstream binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
