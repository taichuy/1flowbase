use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use control_plane::mcp_management::{
    McpManagementService, McpRemoteToolDefinition, McpUpstreamCredential,
    RecordMcpUpstreamDiscoveryCommand, SaveMcpUpstreamConnectionCommand,
    SaveMcpUpstreamCredentialCommand,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
};

use super::{
    to_tool_response_with_operation,
    upstream_client::{execute_proxy_call, McpStreamableHttpClient},
    McpToolResponse,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveMcpUpstreamConnectionBody {
    pub name: String,
    pub endpoint: String,
    pub transport: String,
    pub auth_type: String,
    pub custom_header_name: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpUpstreamConnectionResponse {
    pub connection_id: String,
    pub workspace_id: String,
    pub name: String,
    pub endpoint: String,
    pub transport: String,
    pub auth_type: String,
    pub custom_header_name: Option<String>,
    pub status: String,
    pub credentials_status: String,
    pub last_connected_at: Option<String>,
    pub last_discovered_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SaveMcpUpstreamCredentialBody {
    Bearer {
        token: String,
    },
    CustomHeader {
        header_name: String,
        header_value: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpUpstreamTestResponse {
    pub connection_id: String,
    pub ok: bool,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub protocol_version: Option<String>,
    pub tested_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpUpstreamToolResponse {
    pub remote_tool_name: String,
    pub description: Option<String>,
    #[schema(value_type = Object)]
    pub input_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_schema: serde_json::Value,
    pub source_status: String,
    pub imported_tool_id: Option<String>,
    pub schema_hash: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpUpstreamDiscoverResponse {
    pub connection_id: String,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub protocol_version: String,
    pub discovered_at: String,
    pub items: Vec<McpUpstreamToolResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportMcpUpstreamToolsBody {
    pub remote_tool_names: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DebugMcpProxyToolBody {
    #[schema(value_type = Object)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DebugMcpProxyToolResponse {
    #[schema(value_type = Object)]
    pub local_arguments: serde_json::Value,
    #[schema(value_type = Object)]
    pub remote_arguments: serde_json::Value,
    #[schema(value_type = Object)]
    pub upstream_result: serde_json::Value,
    #[schema(value_type = Object)]
    pub mapped_result: serde_json::Value,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/mcp/upstream-connections",
            get(list_connections).post(create_connection),
        )
        .route(
            "/mcp/upstream-connections/:connection_id",
            put(update_connection).delete(delete_connection),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/credentials",
            put(save_credentials).delete(delete_credentials),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/test",
            post(test_connection),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/discover",
            post(discover_tools),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/imports",
            post(import_tools),
        )
        .route("/mcp/tools/:tool_id/debug", post(debug_proxy_tool))
}

#[utoipa::path(get, path = "/api/console/mcp/upstream-connections", responses((status = 200, body = [McpUpstreamConnectionResponse])))]
pub async fn list_connections(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpUpstreamConnectionResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let records = McpManagementService::new(state.store.clone())
        .list_upstream_connections(context.user.id)
        .await?;
    Ok(Json(ApiSuccess::new(
        records.into_iter().map(to_connection_response).collect(),
    )))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections", request_body = SaveMcpUpstreamConnectionBody, responses((status = 201, body = McpUpstreamConnectionResponse)))]
pub async fn create_connection(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<SaveMcpUpstreamConnectionBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpUpstreamConnectionResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .save_upstream_connection(connection_command(context.user.id, None, body)?)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_connection_response(record))),
    ))
}

#[utoipa::path(put, path = "/api/console/mcp/upstream-connections/{connection_id}", request_body = SaveMcpUpstreamConnectionBody, responses((status = 200, body = McpUpstreamConnectionResponse)))]
pub async fn update_connection(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SaveMcpUpstreamConnectionBody>,
) -> Result<Json<ApiSuccess<McpUpstreamConnectionResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let connection_id = parse_connection_id(&connection_id)?;
    let record = McpManagementService::new(state.store.clone())
        .save_upstream_connection(connection_command(
            context.user.id,
            Some(connection_id),
            body,
        )?)
        .await?;
    Ok(Json(ApiSuccess::new(to_connection_response(record))))
}

#[utoipa::path(delete, path = "/api/console/mcp/upstream-connections/{connection_id}", responses((status = 204)))]
pub async fn delete_connection(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_upstream_connection(context.user.id, parse_connection_id(&connection_id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/console/mcp/upstream-connections/{connection_id}/credentials", request_body = SaveMcpUpstreamCredentialBody, responses((status = 204)))]
pub async fn save_credentials(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SaveMcpUpstreamCredentialBody>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let credential = match body {
        SaveMcpUpstreamCredentialBody::Bearer { token } => McpUpstreamCredential::Bearer { token },
        SaveMcpUpstreamCredentialBody::CustomHeader {
            header_name,
            header_value,
        } => McpUpstreamCredential::CustomHeader {
            header_name,
            header_value,
        },
    };
    McpManagementService::new(state.store.clone())
        .save_upstream_credential(SaveMcpUpstreamCredentialCommand {
            actor_user_id: context.user.id,
            connection_id: parse_connection_id(&connection_id)?,
            credential,
            master_key: state.provider_secret_master_key.clone(),
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/console/mcp/upstream-connections/{connection_id}/credentials", responses((status = 204)))]
pub async fn delete_credentials(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_upstream_credential(context.user.id, parse_connection_id(&connection_id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/{connection_id}/test", responses((status = 200, body = McpUpstreamTestResponse)))]
pub async fn test_connection(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpUpstreamTestResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let connection_id = parse_connection_id(&connection_id)?;
    let service = McpManagementService::new(state.store.clone());
    let (connection, secret) = service
        .prepare_upstream_management_action(
            context.user.id,
            connection_id,
            &state.provider_secret_master_key,
        )
        .await?;
    let tested_at = OffsetDateTime::now_utc();
    let result = match McpStreamableHttpClient::connect(&connection, secret.as_ref()).await {
        Ok(client) => client.initialize().await,
        Err(error) => Err(error),
    };
    let (response, last_error) = match result {
        Ok(server) => (
            McpUpstreamTestResponse {
                connection_id: connection_id.to_string(),
                ok: true,
                server_name: server.name,
                server_version: server.version,
                protocol_version: Some(server.protocol_version),
                tested_at: tested_at.to_string(),
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
                    tested_at: tested_at.to_string(),
                    error: Some(error.clone()),
                },
                Some(error),
            )
        }
    };
    service
        .record_upstream_result(
            context.user.id,
            connection_id,
            response.ok.then_some(tested_at),
            None,
            last_error.as_deref(),
        )
        .await?;
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/{connection_id}/discover", responses((status = 200, body = McpUpstreamDiscoverResponse)))]
pub async fn discover_tools(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpUpstreamDiscoverResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let connection_id = parse_connection_id(&connection_id)?;
    let service = McpManagementService::new(state.store.clone());
    let (connection, secret) = service
        .prepare_upstream_management_action(
            context.user.id,
            connection_id,
            &state.provider_secret_master_key,
        )
        .await?;
    let client = McpStreamableHttpClient::connect(&connection, secret.as_ref())
        .await
        .map_err(|_| {
            control_plane::errors::ControlPlaneError::UpstreamUnavailable("mcp_connection")
        })?;
    let discovery = client.discover_tools().await.map_err(|_| {
        control_plane::errors::ControlPlaneError::UpstreamUnavailable("mcp_discovery")
    })?;
    let discovered_at = OffsetDateTime::now_utc();
    let sources = service
        .record_upstream_discovery(RecordMcpUpstreamDiscoveryCommand {
            actor_user_id: context.user.id,
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
    Ok(Json(ApiSuccess::new(McpUpstreamDiscoverResponse {
        connection_id: connection_id.to_string(),
        server_name: discovery.server.name,
        server_version: discovery.server.version,
        protocol_version: discovery.server.protocol_version,
        discovered_at: discovered_at.to_string(),
        items: sources.into_iter().map(to_tool_source_response).collect(),
    })))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/{connection_id}/imports", request_body = ImportMcpUpstreamToolsBody, responses((status = 200, body = [McpToolResponse])))]
pub async fn import_tools(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ImportMcpUpstreamToolsBody>,
) -> Result<Json<ApiSuccess<Vec<McpToolResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let records = McpManagementService::new(state.store.clone())
        .import_upstream_tools(
            context.user.id,
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
            return Err(
                control_plane::errors::ControlPlaneError::InvalidInput("execution_target").into(),
            );
        };
        let availability = McpManagementService::new(state.store.clone())
            .upstream_proxy_availability(context.user.id, *upstream_connection_id, remote_tool_name)
            .await?;
        let operation = format!("MCP tools/call {remote_tool_name}");
        responses.push(to_tool_response_with_operation(
            record,
            operation,
            availability,
        ));
    }
    Ok(Json(ApiSuccess::new(responses)))
}

#[utoipa::path(post, path = "/api/console/mcp/tools/{tool_id}/debug", request_body = DebugMcpProxyToolBody, responses((status = 200, body = DebugMcpProxyToolResponse)))]
pub async fn debug_proxy_tool(
    State(state): State<Arc<ApiState>>,
    Path(tool_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<DebugMcpProxyToolBody>,
) -> Result<Json<ApiSuccess<DebugMcpProxyToolResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let service = McpManagementService::new(state.store.clone());
    let tool = service.get_tool(context.user.id, &tool_id).await?;
    let domain::McpToolExecutionTarget::McpProxy {
        upstream_connection_id,
        remote_tool_name,
        ..
    } = &tool.execution_target
    else {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("execution_target").into(),
        );
    };
    let (connection, secret) = service
        .prepare_upstream_management_action(
            context.user.id,
            *upstream_connection_id,
            &state.provider_secret_master_key,
        )
        .await?;
    if service
        .upstream_proxy_availability(context.user.id, *upstream_connection_id, remote_tool_name)
        .await?
        != domain::McpToolAvailabilityStatus::Available
    {
        return Err(
            control_plane::errors::ControlPlaneError::UpstreamUnavailable("mcp_proxy_unavailable")
                .into(),
        );
    }
    let client = McpStreamableHttpClient::connect(&connection, secret.as_ref())
        .await
        .map_err(|_| {
            control_plane::errors::ControlPlaneError::UpstreamUnavailable("mcp_connection")
        })?;
    let trace = execute_proxy_call(
        &client,
        remote_tool_name,
        body.arguments,
        &tool.input_mapping,
        &tool.output_mapping,
    )
    .await
    .map_err(|_| control_plane::errors::ControlPlaneError::UpstreamUnavailable("mcp_tools_call"))?;
    Ok(Json(ApiSuccess::new(DebugMcpProxyToolResponse {
        local_arguments: trace.local_arguments,
        remote_arguments: trace.remote_arguments,
        upstream_result: serde_json::to_value(trace.upstream_result).map_err(ApiError::from)?,
        mapped_result: serde_json::to_value(trace.mapped_result).map_err(ApiError::from)?,
    })))
}

fn connection_command(
    actor_user_id: Uuid,
    connection_id: Option<Uuid>,
    body: SaveMcpUpstreamConnectionBody,
) -> Result<SaveMcpUpstreamConnectionCommand, ApiError> {
    let transport = match body.transport.as_str() {
        "streamable_http" => domain::McpUpstreamTransport::StreamableHttp,
        _ => {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput("transport").into())
        }
    };
    let auth_type = match body.auth_type.as_str() {
        "none" => domain::McpUpstreamAuthType::None,
        "bearer" => domain::McpUpstreamAuthType::Bearer,
        "custom_header" => domain::McpUpstreamAuthType::CustomHeader,
        _ => {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput("auth_type").into())
        }
    };
    let status = match body.status.as_str() {
        "enabled" => domain::McpUpstreamConnectionStatus::Enabled,
        "disabled" => domain::McpUpstreamConnectionStatus::Disabled,
        _ => return Err(control_plane::errors::ControlPlaneError::InvalidInput("status").into()),
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

fn parse_connection_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("connection_id").into())
}

fn to_connection_response(
    record: domain::McpUpstreamConnectionRecord,
) -> McpUpstreamConnectionResponse {
    let credentials_status = match record.auth_type {
        domain::McpUpstreamAuthType::None => "not_required",
        _ if record.credentials_configured => "configured",
        _ => "missing",
    };
    McpUpstreamConnectionResponse {
        connection_id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        name: record.name,
        endpoint: record.endpoint,
        transport: record.transport.as_str().into(),
        auth_type: record.auth_type.as_str().into(),
        custom_header_name: record.custom_header_name,
        status: record.status.as_str().into(),
        credentials_status: credentials_status.into(),
        last_connected_at: record.last_connected_at.map(|value| value.to_string()),
        last_discovered_at: record.last_discovered_at.map(|value| value.to_string()),
        last_error: record.last_error,
        created_at: record.created_at.to_string(),
        updated_at: record.updated_at.to_string(),
    }
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
