use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_post, console_put, ConsoleRouteAssembly,
    },
};

use super::{upstream_interface, McpToolResponse};

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct SaveMcpUpstreamConnectionBody {
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub(crate) transport: String,
    pub(crate) auth_type: String,
    pub(crate) custom_header_name: Option<String>,
    pub(crate) status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct McpUpstreamConnectionResponse {
    pub(crate) connection_id: String,
    pub(crate) workspace_id: String,
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub(crate) transport: String,
    pub(crate) auth_type: String,
    pub(crate) custom_header_name: Option<String>,
    pub(crate) status: String,
    pub(crate) credentials_status: String,
    pub(crate) last_connected_at: Option<String>,
    pub(crate) last_discovered_at: Option<String>,
    pub(crate) last_error: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SaveMcpUpstreamCredentialBody {
    Bearer {
        token: String,
    },
    CustomHeader {
        header_name: String,
        header_value: String,
    },
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct McpUpstreamTestResponse {
    pub(crate) connection_id: String,
    pub(crate) ok: bool,
    pub(crate) server_name: Option<String>,
    pub(crate) server_version: Option<String>,
    pub(crate) protocol_version: Option<String>,
    pub(crate) tested_at: String,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct TestMcpUpstreamConnectionDraftBody {
    pub(crate) connection_id: Option<String>,
    pub(crate) endpoint: String,
    pub(crate) transport: String,
    pub(crate) auth_type: String,
    pub(crate) custom_header_name: Option<String>,
    pub(crate) credential: Option<SaveMcpUpstreamCredentialBody>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct McpUpstreamDraftTestResponse {
    pub(crate) ok: bool,
    pub(crate) server_name: Option<String>,
    pub(crate) server_version: Option<String>,
    pub(crate) protocol_version: Option<String>,
    pub(crate) tested_at: String,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct McpUpstreamToolResponse {
    pub(crate) remote_tool_name: String,
    pub(crate) description: Option<String>,
    #[schema(value_type = Object)]
    pub(crate) input_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub(crate) output_schema: serde_json::Value,
    pub(crate) source_status: String,
    pub(crate) imported_tool_id: Option<String>,
    pub(crate) schema_hash: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct McpUpstreamDiscoverResponse {
    pub(crate) connection_id: String,
    pub(crate) server_name: Option<String>,
    pub(crate) server_version: Option<String>,
    pub(crate) protocol_version: String,
    pub(crate) discovered_at: String,
    pub(crate) items: Vec<McpUpstreamToolResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct ImportMcpUpstreamToolsBody {
    pub(crate) remote_tool_names: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(crate) struct DebugMcpProxyToolBody {
    #[schema(value_type = Object)]
    pub(crate) arguments: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct DebugMcpProxyToolResponse {
    #[schema(value_type = Object)]
    pub(crate) local_arguments: serde_json::Value,
    #[schema(value_type = Object)]
    pub(crate) remote_arguments: serde_json::Value,
    #[schema(value_type = Object)]
    pub(crate) upstream_result: serde_json::Value,
    #[schema(value_type = Object)]
    pub(crate) mapped_result: serde_json::Value,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/mcp/upstream-connections",
            console_get(
                list_connections,
                ConsoleOperation("mcp.upstream_connections.view".to_string()),
            )
            .post(
                create_connection,
                ConsoleOperation("mcp.upstream_connections.create".to_string()),
            ),
        )
        .route(
            "/mcp/upstream-connections/:connection_id",
            console_put(
                update_connection,
                ConsoleOperation("mcp.upstream_connections.update".to_string()),
            )
            .delete(
                delete_connection,
                ConsoleOperation("mcp.upstream_connections.delete".to_string()),
            ),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/credentials",
            console_put(
                save_credentials,
                ConsoleOperation("mcp.upstream_credentials.update".to_string()),
            )
            .delete(
                delete_credentials,
                ConsoleOperation("mcp.upstream_credentials.delete".to_string()),
            ),
        )
        .route(
            "/mcp/upstream-connections/test",
            console_post(
                test_draft_connection,
                ConsoleOperation("mcp.upstream_connections.test".to_string()),
            ),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/test",
            console_post(
                test_connection,
                ConsoleOperation("mcp.upstream_connections.test".to_string()),
            ),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/discover",
            console_post(
                discover_tools,
                ConsoleOperation("mcp.upstream_connections.discover".to_string()),
            ),
        )
        .route(
            "/mcp/upstream-connections/:connection_id/imports",
            console_post(
                import_tools,
                ConsoleOperation("mcp.upstream_tools.import".to_string()),
            ),
        )
        .route(
            "/mcp/tools/:tool_id/debug",
            console_post(
                debug_proxy_tool,
                ConsoleOperation("mcp.upstream_tools.debug".to_string()),
            ),
        )
}

async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: upstream_interface::McpUpstreamInput,
    mutating: bool,
) -> Result<upstream_interface::McpUpstreamOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}

#[utoipa::path(get, path = "/api/console/mcp/upstream-connections", responses((status = 200, body = [McpUpstreamConnectionResponse])))]
pub async fn list_connections(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpUpstreamConnectionResponse>>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::Connections(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.list.v1",
        upstream_interface::McpUpstreamInput::List,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections", request_body = SaveMcpUpstreamConnectionBody, responses((status = 201, body = McpUpstreamConnectionResponse)))]
pub async fn create_connection(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<SaveMcpUpstreamConnectionBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpUpstreamConnectionResponse>>), ApiError> {
    let upstream_interface::McpUpstreamOutput::Created(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.create.v1",
        upstream_interface::McpUpstreamInput::Create(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(response))))
}

#[utoipa::path(put, path = "/api/console/mcp/upstream-connections/{connection_id}", request_body = SaveMcpUpstreamConnectionBody, responses((status = 200, body = McpUpstreamConnectionResponse)))]
pub async fn update_connection(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SaveMcpUpstreamConnectionBody>,
) -> Result<Json<ApiSuccess<McpUpstreamConnectionResponse>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::Connection(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.update.v1",
        upstream_interface::McpUpstreamInput::Update {
            connection_id,
            body,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(delete, path = "/api/console/mcp/upstream-connections/{connection_id}", responses((status = 204)))]
pub async fn delete_connection(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let upstream_interface::McpUpstreamOutput::NoContent = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.delete.v1",
        upstream_interface::McpUpstreamInput::Delete(connection_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/console/mcp/upstream-connections/{connection_id}/credentials", request_body = SaveMcpUpstreamCredentialBody, responses((status = 204)))]
pub async fn save_credentials(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SaveMcpUpstreamCredentialBody>,
) -> Result<StatusCode, ApiError> {
    let upstream_interface::McpUpstreamOutput::NoContent = invoke(
        state,
        headers,
        "http.console.mcp.upstream-credentials.save.v1",
        upstream_interface::McpUpstreamInput::SaveCredentials {
            connection_id,
            body,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/console/mcp/upstream-connections/{connection_id}/credentials", responses((status = 204)))]
pub async fn delete_credentials(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let upstream_interface::McpUpstreamOutput::NoContent = invoke(
        state,
        headers,
        "http.console.mcp.upstream-credentials.delete.v1",
        upstream_interface::McpUpstreamInput::DeleteCredentials(connection_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/test", request_body = TestMcpUpstreamConnectionDraftBody, responses((status = 200, body = McpUpstreamDraftTestResponse)))]
pub async fn test_draft_connection(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<TestMcpUpstreamConnectionDraftBody>,
) -> Result<Json<ApiSuccess<McpUpstreamDraftTestResponse>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::DraftTest(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.test-draft.v1",
        upstream_interface::McpUpstreamInput::TestDraft(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/{connection_id}/test", responses((status = 200, body = McpUpstreamTestResponse)))]
pub async fn test_connection(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpUpstreamTestResponse>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::Test(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.test.v1",
        upstream_interface::McpUpstreamInput::Test(connection_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/{connection_id}/discover", responses((status = 200, body = McpUpstreamDiscoverResponse)))]
pub async fn discover_tools(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpUpstreamDiscoverResponse>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::Discovery(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.discover.v1",
        upstream_interface::McpUpstreamInput::Discover(connection_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(post, path = "/api/console/mcp/upstream-connections/{connection_id}/imports", request_body = ImportMcpUpstreamToolsBody, responses((status = 200, body = [McpToolResponse])))]
pub async fn import_tools(
    State(state): State<Arc<ApiState>>,
    Path(connection_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<ImportMcpUpstreamToolsBody>,
) -> Result<Json<ApiSuccess<Vec<McpToolResponse>>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::Imported(response) = invoke(
        state,
        headers,
        "http.console.mcp.upstream-connections.import.v1",
        upstream_interface::McpUpstreamInput::Import {
            connection_id,
            body,
        },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(post, path = "/api/console/mcp/tools/{tool_id}/debug", request_body = DebugMcpProxyToolBody, responses((status = 200, body = DebugMcpProxyToolResponse)))]
pub async fn debug_proxy_tool(
    State(state): State<Arc<ApiState>>,
    Path(tool_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<DebugMcpProxyToolBody>,
) -> Result<Json<ApiSuccess<DebugMcpProxyToolResponse>>, ApiError> {
    let upstream_interface::McpUpstreamOutput::Debug(response) = invoke(
        state,
        headers,
        "http.console.mcp.tools.debug.v1",
        upstream_interface::McpUpstreamInput::Debug { tool_id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(response)))
}
