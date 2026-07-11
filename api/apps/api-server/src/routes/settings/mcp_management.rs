pub(crate) mod debug_execute;

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use control_plane::mcp_management::{
    CreateMcpInstanceCommand, CreateMcpToolBindingCommand, CreateMcpToolCommand,
    McpManagementService, RefreshMcpToolDescriptionCommand, SaveMcpClientCredentialCommand,
    UpdateMcpInstanceDiscoveryPolicyCommand, UpdateMcpToolBindingCommand, UpdateMcpToolCommand,
    UpsertMcpGroupCommand,
};
use domain::mcp_management::{McpParameterDescriptor, McpParameterType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    openapi_docs::{ApiDocsRegistry, DocsCatalogOperation},
    response::ApiSuccess,
    runtime_data_model_docs,
};

pub use debug_execute::{
    McpDebugExecuteBody, McpDebugExecuteDetailsResponse, McpDebugResponseMode,
};

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInstanceResponse {
    pub id: String,
    pub workspace_id: String,
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: String,
    pub default_entry_path: String,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveMcpClientCredentialBody {
    pub api_key: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpClientCredentialResponse {
    pub saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpGroupResponse {
    pub id: String,
    pub instance_record_id: String,
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpToolResponse {
    pub id: String,
    pub workspace_id: String,
    pub tool_id: String,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_id: String,
    pub operation: String,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub result_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub input_mapping: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: String,
    pub des_id: String,
    pub des_id_required: bool,
    pub status: String,
    pub revision: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpToolBindingResponse {
    pub id: String,
    pub instance_record_id: String,
    pub tool_record_id: String,
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInstanceDiscoveryPolicyResponse {
    pub id: String,
    pub workspace_id: String,
    pub instance_record_id: String,
    pub instance_id: String,
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    #[schema(value_type = Object)]
    pub list_return_fields: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpCatalogResponse {
    pub instances: Vec<McpInstanceResponse>,
    pub groups: Vec<McpGroupResponse>,
    pub tools: Vec<McpToolResponse>,
    pub bindings: Vec<McpToolBindingResponse>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpParameterDescriptorResponse {
    pub name: String,
    pub field_type: String,
    pub parameter_type: String,
    pub description: Option<String>,
    pub required: bool,
    #[schema(value_type = Object)]
    pub schema: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInterfaceCatalogEntryResponse {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub short_description: String,
    pub parameter_descriptors: Vec<McpParameterDescriptorResponse>,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub result_schema: serde_json::Value,
    pub permission_code: Option<String>,
    #[schema(value_type = [Object])]
    pub security: serde_json::Value,
    pub risk_level: String,
    pub bindable: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpDescriptionCheckResponse {
    pub accepted: bool,
    pub current_des_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpListItemSummaryResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpExportPackageResponse {
    pub instances: Vec<McpInstanceResponse>,
    pub groups: Vec<McpGroupResponse>,
    pub tools: Vec<McpToolResponse>,
    pub bindings: Vec<McpToolBindingResponse>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct McpInstanceDirectoryExportPackageResponse {
    pub instances: Vec<McpInstanceResponse>,
    pub groups: Vec<McpGroupResponse>,
    pub bindings: Vec<McpToolBindingResponse>,
    pub discovery_policies: Vec<McpInstanceDiscoveryPolicyResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpInstanceBody {
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: String,
    pub default_entry_path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertMcpGroupBody {
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteMcpGroupQuery {
    pub path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpToolBody {
    pub tool_id: String,
    pub des_id: Option<String>,
    pub name: String,
    pub short_description: String,
    pub full_description: String,
    pub interface_id: String,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub result_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub input_mapping: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: String,
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMcpToolBody {
    pub name: String,
    pub des_id: Option<String>,
    pub short_description: String,
    pub full_description: String,
    pub interface_id: String,
    #[schema(value_type = Object)]
    pub parameter_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub result_schema: serde_json::Value,
    #[schema(value_type = Object)]
    pub input_mapping: serde_json::Value,
    #[schema(value_type = Object)]
    pub output_mapping: serde_json::Value,
    pub permission_code: Option<String>,
    pub risk_level: String,
    pub status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpToolBindingBody {
    pub group_path: String,
    pub tool_id: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMcpToolBindingBody {
    pub group_path: String,
    pub display_alias: Option<String>,
    pub visible: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMcpInstanceDiscoveryPolicyBody {
    pub list_default_limit: i32,
    pub list_max_depth: i32,
    pub list_regex_enabled: bool,
    pub list_regex_max_length: i32,
    #[schema(value_type = Object)]
    pub list_return_fields: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct McpDescriptionCheckBody {
    pub des_id: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct McpInterfaceCatalogQuery {
    pub bindable_only: Option<bool>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct McpListQuery {
    pub instance_id: Option<String>,
    pub path: Option<String>,
    pub path_regex: Option<String>,
    pub limit: Option<usize>,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/mcp/catalog", get(get_mcp_catalog))
        .route(
            "/mcp/interface-capabilities",
            get(list_mcp_interface_capabilities),
        )
        .route("/mcp/list", get(list_mcp_items))
        .route("/mcp/export", get(export_mcp_catalog))
        .route(
            "/mcp/instances",
            get(list_mcp_instances).post(create_mcp_instance),
        )
        .route("/mcp/instances/export", get(export_mcp_instance_directory))
        .route(
            "/mcp/instances/:instance_id",
            put(update_mcp_instance).delete(delete_mcp_instance),
        )
        .route(
            "/mcp/instances/:instance_id/client-credential",
            get(get_mcp_client_credential)
                .put(save_mcp_client_credential)
                .delete(delete_mcp_client_credential),
        )
        .route(
            "/mcp/instances/:instance_id/groups",
            post(upsert_mcp_group).delete(delete_mcp_group),
        )
        .route(
            "/mcp/instances/:instance_id/tool-bindings",
            post(create_mcp_tool_binding),
        )
        .route(
            "/mcp/tool-bindings/:binding_id",
            put(update_mcp_tool_binding).delete(delete_mcp_tool_binding),
        )
        .route("/mcp/tools", get(list_mcp_tools).post(create_mcp_tool))
        .route(
            "/mcp/tools/:tool_id",
            get(get_mcp_tool)
                .put(update_mcp_tool)
                .delete(delete_mcp_tool),
        )
        .route(
            "/mcp/tools/:tool_id/description/refresh",
            post(refresh_mcp_tool_description),
        )
        .route(
            "/mcp/tools/:tool_id/description-check",
            post(check_mcp_tool_description),
        )
        .route("/mcp/debug/execute", post(execute_mcp_debug))
        .route(
            "/mcp/instances/:instance_id/discovery-policy",
            get(get_mcp_instance_discovery_policy).put(update_mcp_instance_discovery_policy),
        )
}

pub async fn get_mcp_client_credential(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
) -> Result<Json<ApiSuccess<McpClientCredentialResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let api_key = McpManagementService::new(state.store.clone())
        .get_client_credential(
            context.user.id,
            &instance_id,
            &state.provider_secret_master_key,
        )
        .await?;
    Ok(Json(ApiSuccess::new(McpClientCredentialResponse {
        saved: api_key.is_some(),
        api_key,
    })))
}

pub async fn save_mcp_client_credential(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<SaveMcpClientCredentialBody>,
) -> Result<Json<ApiSuccess<McpClientCredentialResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .save_client_credential(SaveMcpClientCredentialCommand {
            actor_user_id: context.user.id,
            instance_id,
            api_key: body.api_key,
            master_key: state.provider_secret_master_key.clone(),
        })
        .await?;
    Ok(Json(ApiSuccess::new(McpClientCredentialResponse {
        saved: true,
        api_key: None,
    })))
}

pub async fn delete_mcp_client_credential(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_client_credential(context.user.id, &instance_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/mcp/catalog", responses((status = 200, body = McpCatalogResponse)))]
pub async fn get_mcp_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let service = McpManagementService::new(state.store.clone());
    let snapshot = service.read_workspace_catalog(context.user.id).await?;
    let operations = mcp_interface_operation_map(state.as_ref(), context.user.id).await?;
    Ok(Json(ApiSuccess::new(to_catalog_response(
        snapshot,
        &operations,
    )?)))
}

#[utoipa::path(get, path = "/api/console/mcp/interface-capabilities", params(McpInterfaceCatalogQuery), responses((status = 200, body = [McpInterfaceCatalogEntryResponse])))]
pub async fn list_mcp_interface_capabilities(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpInterfaceCatalogQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpInterfaceCatalogEntryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    McpManagementService::new(state.store.clone())
        .authorize_interface_catalog_view(context.user.id)
        .await?;
    let mut entries = mcp_interface_catalog_entries(state.as_ref(), context.user.id).await?;
    if query.bindable_only.unwrap_or(false) {
        entries.retain(|entry| entry.bindable);
    }
    Ok(Json(ApiSuccess::new(
        entries.into_iter().map(to_interface_response).collect(),
    )))
}

#[utoipa::path(get, path = "/api/console/mcp/list", params(McpListQuery), responses((status = 200, body = [McpListItemSummaryResponse])))]
pub async fn list_mcp_items(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpListQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpListItemSummaryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let service = McpManagementService::new(state.store.clone());
    let items = service
        .list_items(
            context.user.id,
            query.instance_id.as_deref(),
            query.path.as_deref(),
            query.path_regex.as_deref(),
            query.limit,
        )
        .await?;
    let instance_id = query.instance_id.as_deref().ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("instance_id"),
    )?;
    let discovery_policy = service
        .get_instance_discovery_policy(context.user.id, instance_id)
        .await?;
    let return_fields = list_response_field_set(&discovery_policy.list_return_fields)?;
    Ok(Json(ApiSuccess::new(
        items
            .into_iter()
            .map(|item| to_list_item_response(item, &return_fields))
            .collect(),
    )))
}

#[utoipa::path(get, path = "/api/console/mcp/export", responses((status = 200, body = McpExportPackageResponse)))]
pub async fn export_mcp_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpExportPackageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let export = McpManagementService::new(state.store.clone())
        .export_workspace_catalog(context.user.id)
        .await?;
    let operations = mcp_interface_operation_map(state.as_ref(), context.user.id).await?;
    Ok(Json(ApiSuccess::new(to_export_response(
        export,
        &operations,
    )?)))
}

#[utoipa::path(get, path = "/api/console/mcp/instances/export", responses((status = 200, body = McpInstanceDirectoryExportPackageResponse)))]
pub async fn export_mcp_instance_directory(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpInstanceDirectoryExportPackageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let export = McpManagementService::new(state.store.clone())
        .export_instance_directory(context.user.id)
        .await?;
    Ok(Json(ApiSuccess::new(
        to_instance_directory_export_response(export)?,
    )))
}

#[utoipa::path(get, path = "/api/console/mcp/instances", responses((status = 200, body = [McpInstanceResponse])))]
pub async fn list_mcp_instances(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpInstanceResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let snapshot = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(context.user.id)
        .await?;
    Ok(Json(ApiSuccess::new(
        snapshot
            .instances
            .into_iter()
            .map(to_instance_response)
            .collect(),
    )))
}

#[utoipa::path(post, path = "/api/console/mcp/instances", request_body = CreateMcpInstanceBody, responses((status = 201, body = McpInstanceResponse)))]
pub async fn create_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateMcpInstanceBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpInstanceResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .create_instance(to_instance_command(context.user.id, body)?)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_instance_response(record))),
    ))
}

#[utoipa::path(put, path = "/api/console/mcp/instances/{instance_id}", request_body = CreateMcpInstanceBody, responses((status = 200, body = McpInstanceResponse)))]
pub async fn update_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(mut body): Json<CreateMcpInstanceBody>,
) -> Result<Json<ApiSuccess<McpInstanceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    body.instance_id = instance_id;
    let record = McpManagementService::new(state.store.clone())
        .update_instance(to_instance_command(context.user.id, body)?)
        .await?;
    Ok(Json(ApiSuccess::new(to_instance_response(record))))
}

#[utoipa::path(delete, path = "/api/console/mcp/instances/{instance_id}", responses((status = 204)))]
pub async fn delete_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_instance(context.user.id, &instance_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/groups", request_body = UpsertMcpGroupBody, responses((status = 200, body = McpGroupResponse)))]
pub async fn upsert_mcp_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<UpsertMcpGroupBody>,
) -> Result<Json<ApiSuccess<McpGroupResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .upsert_group(UpsertMcpGroupCommand {
            actor_user_id: context.user.id,
            instance_id,
            path: body.path,
            display_name: body.display_name,
            description_short: body.description_short,
            enabled: body.enabled,
            sort_order: body.sort_order,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_group_response(record))))
}

#[utoipa::path(delete, path = "/api/console/mcp/instances/{instance_id}/groups", params(DeleteMcpGroupQuery), responses((status = 204)))]
pub async fn delete_mcp_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Query(query): Query<DeleteMcpGroupQuery>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_group(context.user.id, &instance_id, &query.path)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/mcp/tools", responses((status = 200, body = [McpToolResponse])))]
pub async fn list_mcp_tools(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpToolResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let snapshot = McpManagementService::new(state.store.clone())
        .read_workspace_catalog(context.user.id)
        .await?;
    let operations = mcp_interface_operation_map(state.as_ref(), context.user.id).await?;
    Ok(Json(ApiSuccess::new(
        snapshot
            .tools
            .into_iter()
            .map(|record| to_tool_response(record, &operations))
            .collect(),
    )))
}

#[utoipa::path(post, path = "/api/console/mcp/tools", request_body = CreateMcpToolBody, responses((status = 201, body = McpToolResponse)))]
pub async fn create_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateMcpToolBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpToolResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let interface_entry =
        bindable_mcp_interface(state.as_ref(), context.user.id, &body.interface_id).await?;
    let operation = interface_operation(&interface_entry);
    let record = McpManagementService::new(state.store.clone())
        .create_tool(to_create_tool_command(
            context.user.id,
            body,
            interface_entry,
        )?)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_tool_response_with_operation(
            record, operation,
        ))),
    ))
}

#[utoipa::path(get, path = "/api/console/mcp/tools/{tool_id}", responses((status = 200, body = McpToolResponse)))]
pub async fn get_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<Json<ApiSuccess<McpToolResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let record = McpManagementService::new(state.store.clone())
        .get_tool(context.user.id, &tool_id)
        .await?;
    let operations = mcp_interface_operation_map(state.as_ref(), context.user.id).await?;
    Ok(Json(ApiSuccess::new(to_tool_response(record, &operations))))
}

#[utoipa::path(put, path = "/api/console/mcp/tools/{tool_id}", request_body = UpdateMcpToolBody, responses((status = 200, body = McpToolResponse)))]
pub async fn update_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
    Json(body): Json<UpdateMcpToolBody>,
) -> Result<Json<ApiSuccess<McpToolResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let interface_entry =
        bindable_mcp_interface(state.as_ref(), context.user.id, &body.interface_id).await?;
    let operation = interface_operation(&interface_entry);
    let record = McpManagementService::new(state.store.clone())
        .update_tool(to_update_tool_command(
            context.user.id,
            tool_id,
            body,
            interface_entry,
        )?)
        .await?;
    Ok(Json(ApiSuccess::new(to_tool_response_with_operation(
        record, operation,
    ))))
}

#[utoipa::path(delete, path = "/api/console/mcp/tools/{tool_id}", responses((status = 204)))]
pub async fn delete_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_tool(context.user.id, &tool_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/mcp/tools/{tool_id}/description/refresh", responses((status = 200, body = McpToolResponse)))]
pub async fn refresh_mcp_tool_description(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<Json<ApiSuccess<McpToolResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .refresh_tool_description(RefreshMcpToolDescriptionCommand {
            actor_user_id: context.user.id,
            tool_id,
        })
        .await?;
    let operations = mcp_interface_operation_map(state.as_ref(), context.user.id).await?;
    Ok(Json(ApiSuccess::new(to_tool_response(record, &operations))))
}

#[utoipa::path(post, path = "/api/console/mcp/tools/{tool_id}/description-check", request_body = McpDescriptionCheckBody, responses((status = 200, body = McpDescriptionCheckResponse)))]
pub async fn check_mcp_tool_description(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
    Json(body): Json<McpDescriptionCheckBody>,
) -> Result<Json<ApiSuccess<McpDescriptionCheckResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let result = McpManagementService::new(state.store.clone())
        .description_check(context.user.id, &tool_id, body.des_id.as_deref())
        .await?;
    Ok(Json(ApiSuccess::new(McpDescriptionCheckResponse {
        accepted: result.accepted,
        current_des_id: result.current_des_id,
    })))
}

#[utoipa::path(post, path = "/api/console/mcp/debug/execute", request_body = McpDebugExecuteBody, responses((status = 200, body = Value)))]
pub async fn execute_mcp_debug(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<McpDebugExecuteBody>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .authorize_debug_execute(context.user.id)
        .await?;
    let interface_entry =
        bindable_mcp_interface(state.as_ref(), context.user.id, &body.interface_id).await?;

    match debug_execute::execute(state, headers, interface_entry, body).await {
        Ok(result) => Ok(Json(ApiSuccess::new(result)).into_response()),
        Err(debug_execute::McpDebugExecuteError::Api(error)) => Err(ApiError(error)),
        Err(debug_execute::McpDebugExecuteError::TargetResponse(response)) => Ok(response),
    }
}

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/tool-bindings", request_body = CreateMcpToolBindingBody, responses((status = 201, body = McpToolBindingResponse)))]
pub async fn create_mcp_tool_binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<CreateMcpToolBindingBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpToolBindingResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: context.user.id,
            instance_id,
            group_path: body.group_path,
            tool_id: body.tool_id,
            display_alias: body.display_alias,
            visible: body.visible,
            sort_order: body.sort_order,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_binding_response(record))),
    ))
}

#[utoipa::path(put, path = "/api/console/mcp/tool-bindings/{binding_id}", request_body = UpdateMcpToolBindingBody, responses((status = 200, body = McpToolBindingResponse)))]
pub async fn update_mcp_tool_binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(binding_id): Path<String>,
    Json(body): Json<UpdateMcpToolBindingBody>,
) -> Result<Json<ApiSuccess<McpToolBindingResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .update_tool_binding(UpdateMcpToolBindingCommand {
            actor_user_id: context.user.id,
            binding_id: parse_uuid(&binding_id, "binding_id")?,
            group_path: body.group_path,
            display_alias: body.display_alias,
            visible: body.visible,
            sort_order: body.sort_order,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_binding_response(record))))
}

#[utoipa::path(delete, path = "/api/console/mcp/tool-bindings/{binding_id}", responses((status = 204)))]
pub async fn delete_mcp_tool_binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(binding_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    McpManagementService::new(state.store.clone())
        .delete_tool_binding(context.user.id, parse_uuid(&binding_id, "binding_id")?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/mcp/instances/{instance_id}/discovery-policy", responses((status = 200, body = McpInstanceDiscoveryPolicyResponse)))]
pub async fn get_mcp_instance_discovery_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpInstanceDiscoveryPolicyResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let record = McpManagementService::new(state.store.clone())
        .get_instance_discovery_policy(context.user.id, &instance_id)
        .await?;
    Ok(Json(ApiSuccess::new(to_discovery_policy_response(
        record,
        instance_id,
    ))))
}

#[utoipa::path(put, path = "/api/console/mcp/instances/{instance_id}/discovery-policy", request_body = UpdateMcpInstanceDiscoveryPolicyBody, responses((status = 200, body = McpInstanceDiscoveryPolicyResponse)))]
pub async fn update_mcp_instance_discovery_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateMcpInstanceDiscoveryPolicyBody>,
) -> Result<Json<ApiSuccess<McpInstanceDiscoveryPolicyResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .update_instance_discovery_policy(UpdateMcpInstanceDiscoveryPolicyCommand {
            actor_user_id: context.user.id,
            instance_id: instance_id.clone(),
            list_default_limit: body.list_default_limit,
            list_max_depth: body.list_max_depth,
            list_regex_enabled: body.list_regex_enabled,
            list_regex_max_length: body.list_regex_max_length,
            list_return_fields: body.list_return_fields,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_discovery_policy_response(
        record,
        instance_id,
    ))))
}

fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn parse_instance_status(value: &str) -> Result<domain::McpInstanceStatus, ApiError> {
    match value {
        "draft" => Ok(domain::McpInstanceStatus::Draft),
        "enabled" => Ok(domain::McpInstanceStatus::Enabled),
        "disabled" => Ok(domain::McpInstanceStatus::Disabled),
        "archived" => Ok(domain::McpInstanceStatus::Archived),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("status").into()),
    }
}

fn parse_tool_status(value: &str) -> Result<domain::McpToolStatus, ApiError> {
    match value {
        "draft" => Ok(domain::McpToolStatus::Draft),
        "enabled" => Ok(domain::McpToolStatus::Enabled),
        "disabled" => Ok(domain::McpToolStatus::Disabled),
        "archived" => Ok(domain::McpToolStatus::Archived),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("status").into()),
    }
}

#[derive(Clone, Copy)]
enum McpInterfaceCapabilitySource {
    StaticApiDocs,
    RuntimeDataModelCrud,
}

struct McpInterfaceScopePolicy {
    bindable: bool,
    disabled_reason: Option<&'static str>,
}

async fn mcp_interface_catalog_entries(
    state: &ApiState,
    actor_user_id: Uuid,
) -> Result<Vec<domain::McpInterfaceCatalogEntry>, ApiError> {
    let mut entries = static_mcp_interface_catalog_entries(&state.api_docs);
    let models = runtime_data_model_docs::ready_models(state, actor_user_id).await?;
    entries.extend(runtime_data_model_mcp_interface_catalog_entries(&models));
    Ok(entries)
}

fn static_mcp_interface_catalog_entries(
    api_docs: &ApiDocsRegistry,
) -> Vec<domain::McpInterfaceCatalogEntry> {
    let mut entries = Vec::new();

    for category in &api_docs.catalog().categories {
        let Some(category_operations) = api_docs.category_operations(&category.id) else {
            continue;
        };

        for operation in &category_operations.operations {
            let Some(spec) = api_docs.operation_spec(&operation.id) else {
                continue;
            };
            let Some(entry) = mcp_interface_entry_from_operation(
                operation,
                spec,
                McpInterfaceCapabilitySource::StaticApiDocs,
            ) else {
                continue;
            };
            entries.push(entry);
        }
    }

    entries
}

fn runtime_data_model_mcp_interface_catalog_entries(
    models: &[domain::ModelDefinitionRecord],
) -> Vec<domain::McpInterfaceCatalogEntry> {
    let category_operations = runtime_data_model_docs::build_category_operations(models);
    let mut entries = Vec::new();

    for operation in &category_operations.operations {
        let Ok(Some((model_id, kind))) = runtime_data_model_docs::parse_operation_id(&operation.id)
        else {
            continue;
        };
        let Some(model) = models.iter().find(|model| model.id == model_id) else {
            continue;
        };
        let spec = runtime_data_model_docs::build_operation_openapi(model, kind);
        let Some(entry) = mcp_interface_entry_from_operation(
            operation,
            &spec,
            McpInterfaceCapabilitySource::RuntimeDataModelCrud,
        ) else {
            continue;
        };
        entries.push(entry);
    }

    entries
}

pub(crate) async fn bindable_mcp_interface(
    state: &ApiState,
    actor_user_id: Uuid,
    interface_id: &str,
) -> Result<domain::McpInterfaceCatalogEntry, ApiError> {
    let entry = mcp_interface_catalog_entries(state, actor_user_id)
        .await?
        .into_iter()
        .find(|entry| entry.interface_id == interface_id)
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "mcp_interface",
        ))?;

    if !entry.bindable {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("interface_id").into());
    }

    Ok(entry)
}

async fn mcp_interface_operation_map(
    state: &ApiState,
    actor_user_id: Uuid,
) -> Result<HashMap<String, String>, ApiError> {
    Ok(mcp_interface_catalog_entries(state, actor_user_id)
        .await?
        .into_iter()
        .map(|entry| {
            let operation = interface_operation(&entry);
            (entry.interface_id, operation)
        })
        .collect())
}

fn interface_operation(entry: &domain::McpInterfaceCatalogEntry) -> String {
    format!("{} {}", entry.method, entry.path)
}

fn mcp_interface_entry_from_operation(
    operation: &DocsCatalogOperation,
    spec: &Value,
    source: McpInterfaceCapabilitySource,
) -> Option<domain::McpInterfaceCatalogEntry> {
    let operation_node = openapi_operation_node(spec, operation)?;
    let path_item_node = openapi_path_item_node(spec, operation)?;
    let scope_policy = mcp_interface_scope_policy(operation, source);

    Some(domain::McpInterfaceCatalogEntry {
        interface_id: operation.id.clone(),
        method: operation.method.clone(),
        path: operation.path.clone(),
        name: operation
            .summary
            .clone()
            .unwrap_or_else(|| operation.id.clone()),
        short_description: operation
            .description
            .clone()
            .unwrap_or_else(|| format!("{} {}", operation.method, operation.path)),
        parameter_descriptors: operation_parameter_descriptors(
            spec,
            path_item_node,
            operation_node,
        ),
        parameter_schema: operation_input_schema(spec, path_item_node, operation_node),
        result_schema: operation_response_schema(spec, operation_node),
        permission_code: operation_permission_code(&operation.method, &operation.path),
        security: operation_security(spec, operation_node),
        risk_level: operation_risk_level(&operation.method),
        bindable: scope_policy.bindable,
        disabled_reason: scope_policy.disabled_reason.map(str::to_string),
    })
}

fn mcp_interface_scope_policy(
    operation: &DocsCatalogOperation,
    source: McpInterfaceCapabilitySource,
) -> McpInterfaceScopePolicy {
    if operation.path == "/api/console/mcp/debug/execute" {
        return McpInterfaceScopePolicy {
            bindable: false,
            disabled_reason: Some("unsupported_mcp_interface_scope"),
        };
    }

    let bindable = match source {
        McpInterfaceCapabilitySource::StaticApiDocs => operation.path.starts_with("/api/console/"),
        McpInterfaceCapabilitySource::RuntimeDataModelCrud => {
            runtime_data_model_crud_path_is_concrete(&operation.path)
        }
    };

    McpInterfaceScopePolicy {
        bindable,
        disabled_reason: if bindable {
            None
        } else {
            Some("unsupported_mcp_interface_scope")
        },
    }
}

fn runtime_data_model_crud_path_is_concrete(path: &str) -> bool {
    path.starts_with("/api/runtime/models/")
        && !path.contains("{model_code}")
        && (path.ends_with("/list")
            || path.ends_with("/create")
            || path.ends_with("/get/{id}")
            || path.ends_with("/update/{id}")
            || path.ends_with("/delete/{id}"))
}

fn openapi_operation_node<'a>(
    spec: &'a Value,
    operation: &DocsCatalogOperation,
) -> Option<&'a Value> {
    let method = operation.method.to_ascii_lowercase();
    spec.pointer(&format!(
        "/paths/{}/{}",
        escape_json_pointer_token(&operation.path),
        method
    ))
}

fn openapi_path_item_node<'a>(
    spec: &'a Value,
    operation: &DocsCatalogOperation,
) -> Option<&'a Value> {
    spec.pointer(&format!(
        "/paths/{}",
        escape_json_pointer_token(&operation.path)
    ))
}

fn escape_json_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn operation_input_schema(spec: &Value, path_item_node: &Value, operation_node: &Value) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();

    if let Some(path_schema) =
        operation_parameter_location_schema(spec, path_item_node, operation_node, "path")
    {
        properties.insert("path".into(), path_schema);
        required.push(Value::String("path".into()));
    }

    if let Some(query_schema) =
        operation_parameter_location_schema(spec, path_item_node, operation_node, "query")
    {
        let query_required = query_schema
            .get("required")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false);
        properties.insert("query".into(), query_schema);
        if query_required {
            required.push(Value::String("query".into()));
        }
    }

    if let Some((body_schema, body_required)) = operation_request_body_schema(spec, operation_node)
    {
        properties.insert("body".into(), body_schema);
        if body_required {
            required.push(Value::String("body".into()));
        }
    }

    object_schema(properties, required)
}

fn operation_parameter_descriptors(
    spec: &Value,
    path_item_node: &Value,
    operation_node: &Value,
) -> Vec<McpParameterDescriptor> {
    let mut descriptors = Vec::new();

    for location in ["path", "query"] {
        descriptors.extend(operation_parameter_location_descriptors(
            spec,
            path_item_node,
            operation_node,
            location,
        ));
    }

    descriptors.extend(operation_request_body_descriptors(spec, operation_node));
    descriptors
}

fn operation_parameter_location_descriptors(
    spec: &Value,
    path_item_node: &Value,
    operation_node: &Value,
    location: &str,
) -> Vec<McpParameterDescriptor> {
    let mut descriptors = Vec::new();

    for raw_parameter in path_item_node
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            operation_node
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
    {
        let parameter = resolve_openapi_schema(spec, raw_parameter);
        if parameter.get("in").and_then(Value::as_str) != Some(location) {
            continue;
        }
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };

        let schema = parameter
            .get("schema")
            .map(|schema| resolve_openapi_schema(spec, schema))
            .unwrap_or_else(default_string_schema);
        let description = parameter
            .get("description")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let required = location == "path"
            || parameter
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false);

        descriptors.push(McpParameterDescriptor {
            name: name.into(),
            field_type: schema_field_type(&schema),
            parameter_type: McpParameterType::Url,
            description,
            required,
            schema,
        });
    }

    descriptors
}

fn operation_parameter_location_schema(
    spec: &Value,
    path_item_node: &Value,
    operation_node: &Value,
    location: &str,
) -> Option<Value> {
    let mut properties = Map::new();
    let mut required = Vec::new();

    for raw_parameter in path_item_node
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            operation_node
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
    {
        let parameter = resolve_openapi_schema(spec, raw_parameter);
        if parameter.get("in").and_then(Value::as_str) != Some(location) {
            continue;
        }
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            continue;
        };

        let mut schema = parameter
            .get("schema")
            .map(|schema| resolve_openapi_schema(spec, schema))
            .unwrap_or_else(default_string_schema);
        if let Some(description) = parameter.get("description").and_then(Value::as_str) {
            schema = schema_with_description(schema, description);
        }
        properties.insert(name.into(), schema);

        if location == "path"
            || parameter
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            required.push(Value::String(name.into()));
        }
    }

    if properties.is_empty() {
        return None;
    }

    Some(object_schema(properties, required))
}

fn operation_request_body_schema(spec: &Value, operation_node: &Value) -> Option<(Value, bool)> {
    let request_body = operation_node.get("requestBody")?;
    let request_body = resolve_openapi_schema(spec, request_body);
    let schema = json_content_schema(spec, request_body.get("content")?)?;
    let required = request_body
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some((schema, required))
}

fn operation_request_body_descriptors(
    spec: &Value,
    operation_node: &Value,
) -> Vec<McpParameterDescriptor> {
    let Some(request_body) = operation_node.get("requestBody") else {
        return Vec::new();
    };
    let request_body = resolve_openapi_schema(spec, request_body);
    let request_body_required = request_body
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let Some((schema, parameter_type)) =
        request_body_descriptor_schema(spec, request_body.get("content").unwrap_or(&Value::Null))
    else {
        return Vec::new();
    };

    schema_property_descriptors(schema, parameter_type, request_body_required)
}

fn request_body_descriptor_schema(
    spec: &Value,
    content: &Value,
) -> Option<(Value, McpParameterType)> {
    let content = content.as_object()?;

    for content_type in ["application/x-www-form-urlencoded", "multipart/form-data"] {
        if let Some(media_type) = content.get(content_type) {
            if let Some(schema) = media_type.get("schema") {
                return Some((resolve_openapi_schema(spec, schema), McpParameterType::Form));
            }
        }
    }

    if let Some(media_type) = content.get("application/json").or_else(|| {
        content
            .iter()
            .find(|(content_type, _)| content_type.ends_with("+json"))
            .map(|(_, media_type)| media_type)
    }) {
        if let Some(schema) = media_type.get("schema") {
            return Some((
                resolve_openapi_schema(spec, schema),
                McpParameterType::JsonBody,
            ));
        }
    }

    None
}

fn schema_property_descriptors(
    schema: Value,
    parameter_type: McpParameterType,
    request_body_required: bool,
) -> Vec<McpParameterDescriptor> {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return vec![McpParameterDescriptor {
            name: "body".into(),
            field_type: schema_field_type(&schema),
            parameter_type,
            description: schema_description(&schema),
            required: request_body_required,
            schema,
        }];
    };

    let required_fields = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let mut descriptors = Vec::new();
    for (name, property_schema) in properties {
        append_schema_property_descriptors(
            &mut descriptors,
            name.clone(),
            property_schema.clone(),
            parameter_type,
            request_body_required && required_fields.contains(name.as_str()),
        );
    }

    descriptors
}

fn append_schema_property_descriptors(
    descriptors: &mut Vec<McpParameterDescriptor>,
    path: String,
    schema: Value,
    parameter_type: McpParameterType,
    required: bool,
) {
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        if !properties.is_empty() {
            let required_fields = schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>();
            for (name, property_schema) in properties {
                append_schema_property_descriptors(
                    descriptors,
                    format!("{path}.{name}"),
                    property_schema.clone(),
                    parameter_type,
                    required && required_fields.contains(name.as_str()),
                );
            }
            return;
        }
    }

    descriptors.push(McpParameterDescriptor {
        name: path,
        field_type: schema_field_type(&schema),
        parameter_type,
        description: schema_description(&schema),
        required,
        schema,
    });
}

fn operation_response_schema(spec: &Value, operation_node: &Value) -> Value {
    let Some(responses) = operation_node.get("responses").and_then(Value::as_object) else {
        return object_schema(Map::new(), Vec::new());
    };

    let mut status_codes = responses
        .keys()
        .filter(|status| status.starts_with('2'))
        .cloned()
        .collect::<Vec<_>>();
    status_codes.sort();

    for status in status_codes {
        let Some(response) = responses.get(&status) else {
            continue;
        };
        let response = resolve_openapi_schema(spec, response);
        if let Some(schema) = response
            .get("content")
            .and_then(|content| json_content_schema(spec, content))
        {
            return schema;
        }
    }

    object_schema(Map::new(), Vec::new())
}

fn json_content_schema(spec: &Value, content: &Value) -> Option<Value> {
    let content = content.as_object()?;
    let media_schema = content
        .get("application/json")
        .or_else(|| {
            content
                .iter()
                .find(|(content_type, _)| content_type.ends_with("+json"))
                .map(|(_, media_type)| media_type)
        })?
        .get("schema")?;

    Some(resolve_openapi_schema(spec, media_schema))
}

fn operation_security(spec: &Value, operation_node: &Value) -> Value {
    operation_node
        .get("security")
        .or_else(|| spec.get("security"))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

fn operation_risk_level(method: &str) -> domain::McpRiskLevel {
    match method {
        "GET" | "HEAD" | "OPTIONS" => domain::McpRiskLevel::Low,
        "DELETE" => domain::McpRiskLevel::Critical,
        "POST" | "PUT" | "PATCH" => domain::McpRiskLevel::High,
        _ => domain::McpRiskLevel::Medium,
    }
}

fn permission_code(code: &str) -> Option<String> {
    Some(code.to_string())
}

fn read_or_manage_permission(method: &str, resource: &str) -> Option<String> {
    let action = match method {
        "GET" | "HEAD" | "OPTIONS" => "view",
        _ => "manage",
    };
    permission_code(&format!("{resource}.{action}.all"))
}

fn view_or_configure_permission(method: &str, resource: &str) -> Option<String> {
    let action = match method {
        "GET" | "HEAD" | "OPTIONS" => "view",
        _ => "configure",
    };
    permission_code(&format!("{resource}.{action}.all"))
}

fn application_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("application.view.all"),
        "POST" if path == "/api/console/applications" => permission_code("application.create.all"),
        "DELETE" => permission_code("application.delete.all"),
        "POST" if path.contains("/actions/") || path.contains("/runs/") => {
            permission_code("application.use.all")
        }
        _ => permission_code("application.edit.all"),
    }
}

fn file_table_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("file_table.view.all"),
        "POST" if path == "/api/console/file-tables" => permission_code("file_table.create.all"),
        "DELETE" => permission_code("file_table.delete.all"),
        "PUT" if path.ends_with("/binding") => permission_code("file_table.bind.all"),
        _ => permission_code("file_table.bind.all"),
    }
}

fn role_permission(method: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("role_permission.view.all"),
        _ => permission_code("role_permission.manage.all"),
    }
}

fn state_model_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("state_model.view.all"),
        "POST" if path == "/api/console/models" => permission_code("state_model.create.all"),
        "POST" if path == "/api/console/models:batchDelete" => {
            permission_code("state_model.delete.all")
        }
        "DELETE" => permission_code("state_model.delete.all"),
        _ => permission_code("state_model.edit.all"),
    }
}

fn external_data_source_permission(method: &str, path: &str) -> Option<String> {
    match method {
        "GET" | "HEAD" | "OPTIONS" => permission_code("external_data_source.view.all"),
        "POST" if path == "/api/console/data-sources" => {
            permission_code("external_data_source.create.all")
        }
        "DELETE" => permission_code("external_data_source.delete.all"),
        _ => permission_code("external_data_source.configure.all"),
    }
}

fn operation_permission_code(method: &str, path: &str) -> Option<String> {
    if path.starts_with("/api/console/docs/") {
        return permission_code("api_reference.view.all");
    }
    if path.starts_with("/api/console/system/runtime-profile")
        || path.starts_with("/api/console/system/release-status")
    {
        return permission_code("system_runtime.view.all");
    }
    if path.starts_with("/api/console/permissions") || path.starts_with("/api/console/roles") {
        return role_permission(method);
    }
    if path.starts_with("/api/console/members") {
        return read_or_manage_permission(method, "user");
    }
    if path.starts_with("/api/console/workspace") || path.starts_with("/api/console/workspaces") {
        return view_or_configure_permission(method, "workspace");
    }
    if path.starts_with("/api/console/mcp/") {
        return read_or_manage_permission(method, "mcp_management");
    }
    if path.starts_with("/api/console/file-storages") {
        return read_or_manage_permission(method, "file_storage");
    }
    if path.starts_with("/api/console/file-tables") {
        return file_table_permission(method, path);
    }
    if path.starts_with("/api/console/model-providers")
        || path.starts_with("/api/console/plugins")
        || path.starts_with("/api/console/host-infrastructure")
    {
        return view_or_configure_permission(method, "plugin_config");
    }
    if path.starts_with("/api/console/data-sources") {
        return external_data_source_permission(method, path);
    }
    if path.starts_with("/api/console/models") {
        return state_model_permission(method, path);
    }
    if path.starts_with("/api/console/applications") {
        return application_permission(method, path);
    }
    if path.starts_with("/api/console/node-contributions")
        || path.starts_with("/api/console/frontend-blocks")
        || path.starts_with("/api/console/js-dependencies")
    {
        return permission_code("plugin_config.view.all");
    }

    None
}

fn object_schema(properties: Map<String, Value>, required: Vec<Value>) -> Value {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".into()));
    schema.insert("properties".into(), Value::Object(properties));
    schema.insert("additionalProperties".into(), Value::Bool(false));
    if !required.is_empty() {
        schema.insert("required".into(), Value::Array(required));
    }
    Value::Object(schema)
}

fn default_string_schema() -> Value {
    let mut fallback = Map::new();
    fallback.insert("type".into(), Value::String("string".into()));
    Value::Object(fallback)
}

fn schema_field_type(schema: &Value) -> String {
    schema
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("object")
        .into()
}

fn schema_description(schema: &Value) -> Option<String> {
    schema
        .get("description")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn schema_with_description(mut schema: Value, description: &str) -> Value {
    if let Value::Object(schema_map) = &mut schema {
        schema_map
            .entry("description")
            .or_insert_with(|| Value::String(description.into()));
    }
    schema
}

fn resolve_openapi_schema(spec: &Value, value: &Value) -> Value {
    resolve_openapi_schema_at_depth(spec, value, 0)
}

fn resolve_openapi_schema_at_depth(spec: &Value, value: &Value, depth: usize) -> Value {
    if depth > 16 {
        return value.clone();
    }

    match value {
        Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                if let Some(pointer) = reference.strip_prefix('#') {
                    if let Some(target) = spec.pointer(pointer) {
                        let mut resolved = resolve_openapi_schema_at_depth(spec, target, depth + 1);
                        if let Value::Object(resolved_map) = &mut resolved {
                            for (key, sibling) in map {
                                if key != "$ref" {
                                    resolved_map.insert(
                                        key.clone(),
                                        resolve_openapi_schema_at_depth(spec, sibling, depth + 1),
                                    );
                                }
                            }
                        }
                        return resolved;
                    }
                }
            }

            Value::Object(
                map.iter()
                    .map(|(key, nested)| {
                        (
                            key.clone(),
                            resolve_openapi_schema_at_depth(spec, nested, depth + 1),
                        )
                    })
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_openapi_schema_at_depth(spec, item, depth + 1))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn to_instance_command(
    actor_user_id: Uuid,
    body: CreateMcpInstanceBody,
) -> Result<CreateMcpInstanceCommand, ApiError> {
    Ok(CreateMcpInstanceCommand {
        actor_user_id,
        instance_id: body.instance_id,
        name: body.name,
        description_short: body.description_short,
        status: parse_instance_status(&body.status)?,
        default_entry_path: body.default_entry_path,
    })
}

fn to_create_tool_command(
    actor_user_id: Uuid,
    body: CreateMcpToolBody,
    interface_entry: domain::McpInterfaceCatalogEntry,
) -> Result<CreateMcpToolCommand, ApiError> {
    Ok(CreateMcpToolCommand {
        actor_user_id,
        tool_id: body.tool_id,
        des_id: body.des_id,
        name: body.name,
        short_description: body.short_description,
        full_description: body.full_description,
        interface_entry,
        input_mapping: body.input_mapping,
        output_mapping: body.output_mapping,
        status: parse_tool_status(&body.status)?,
    })
}

fn to_update_tool_command(
    actor_user_id: Uuid,
    tool_id: String,
    body: UpdateMcpToolBody,
    interface_entry: domain::McpInterfaceCatalogEntry,
) -> Result<UpdateMcpToolCommand, ApiError> {
    Ok(UpdateMcpToolCommand {
        actor_user_id,
        tool_id,
        des_id: body.des_id,
        name: body.name,
        short_description: body.short_description,
        full_description: body.full_description,
        interface_entry,
        input_mapping: body.input_mapping,
        output_mapping: body.output_mapping,
        status: parse_tool_status(&body.status)?,
    })
}

fn to_catalog_response(
    snapshot: domain::McpCatalogSnapshot,
    operations: &HashMap<String, String>,
) -> Result<McpCatalogResponse, ApiError> {
    let discovery_policies =
        discovery_policy_responses(&snapshot.instances, snapshot.discovery_policies)?;
    Ok(McpCatalogResponse {
        instances: snapshot
            .instances
            .into_iter()
            .map(to_instance_response)
            .collect(),
        groups: snapshot.groups.into_iter().map(to_group_response).collect(),
        tools: snapshot
            .tools
            .into_iter()
            .map(|record| to_tool_response(record, operations))
            .collect(),
        bindings: snapshot
            .bindings
            .into_iter()
            .map(to_binding_response)
            .collect(),
        discovery_policies,
    })
}

fn to_export_response(
    export: domain::McpExportPackage,
    operations: &HashMap<String, String>,
) -> Result<McpExportPackageResponse, ApiError> {
    let discovery_policies =
        discovery_policy_responses(&export.instances, export.discovery_policies)?;
    Ok(McpExportPackageResponse {
        instances: export
            .instances
            .into_iter()
            .map(to_instance_response)
            .collect(),
        groups: export.groups.into_iter().map(to_group_response).collect(),
        tools: export
            .tools
            .into_iter()
            .map(|record| to_tool_response(record, operations))
            .collect(),
        bindings: export
            .bindings
            .into_iter()
            .map(to_binding_response)
            .collect(),
        discovery_policies,
    })
}

fn to_instance_directory_export_response(
    export: domain::McpInstanceDirectoryExportPackage,
) -> Result<McpInstanceDirectoryExportPackageResponse, ApiError> {
    let discovery_policies =
        discovery_policy_responses(&export.instances, export.discovery_policies)?;
    Ok(McpInstanceDirectoryExportPackageResponse {
        instances: export
            .instances
            .into_iter()
            .map(to_instance_response)
            .collect(),
        groups: export.groups.into_iter().map(to_group_response).collect(),
        bindings: export
            .bindings
            .into_iter()
            .map(to_binding_response)
            .collect(),
        discovery_policies,
    })
}

fn to_instance_response(record: domain::McpInstanceRecord) -> McpInstanceResponse {
    McpInstanceResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        instance_id: record.instance_id,
        name: record.name,
        description_short: record.description_short,
        status: record.status.as_str().into(),
        default_entry_path: record.default_entry_path,
        created_by: record.created_by.to_string(),
        updated_by: record.updated_by.to_string(),
        created_at: record.created_at.to_string(),
        updated_at: record.updated_at.to_string(),
    }
}

fn to_group_response(record: domain::McpGroupRecord) -> McpGroupResponse {
    McpGroupResponse {
        id: record.id.to_string(),
        instance_record_id: record.instance_record_id.to_string(),
        path: record.path,
        display_name: record.display_name,
        description_short: record.description_short,
        enabled: record.enabled,
        sort_order: record.sort_order,
    }
}

fn to_tool_response(
    record: domain::McpToolRecord,
    operations: &HashMap<String, String>,
) -> McpToolResponse {
    let operation = operations
        .get(&record.interface_id)
        .cloned()
        .unwrap_or_else(|| record.interface_id.clone());

    to_tool_response_with_operation(record, operation)
}

fn to_tool_response_with_operation(
    record: domain::McpToolRecord,
    operation: String,
) -> McpToolResponse {
    McpToolResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        tool_id: record.tool_id,
        name: record.name,
        short_description: record.short_description,
        full_description: record.full_description,
        interface_id: record.interface_id,
        operation,
        parameter_schema: record.parameter_schema,
        result_schema: record.result_schema,
        input_mapping: record.input_mapping,
        output_mapping: record.output_mapping,
        permission_code: record.permission_code,
        risk_level: record.risk_level.as_str().into(),
        des_id: record.des_id,
        des_id_required: record.des_id_required,
        status: record.status.as_str().into(),
        revision: record.revision,
    }
}

fn to_binding_response(record: domain::McpToolBindingRecord) -> McpToolBindingResponse {
    McpToolBindingResponse {
        id: record.id.to_string(),
        instance_record_id: record.instance_record_id.to_string(),
        tool_record_id: record.tool_record_id.to_string(),
        group_path: record.group_path,
        tool_id: record.tool_id,
        display_alias: record.display_alias,
        visible: record.visible,
        sort_order: record.sort_order,
    }
}

fn to_discovery_policy_response(
    record: domain::McpInstanceDiscoveryPolicyRecord,
    instance_id: String,
) -> McpInstanceDiscoveryPolicyResponse {
    McpInstanceDiscoveryPolicyResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        instance_record_id: record.instance_record_id.to_string(),
        instance_id,
        list_default_limit: record.list_default_limit,
        list_max_depth: record.list_max_depth,
        list_regex_enabled: record.list_regex_enabled,
        list_regex_max_length: record.list_regex_max_length,
        list_return_fields: record.list_return_fields,
    }
}

fn discovery_policy_responses(
    instances: &[domain::McpInstanceRecord],
    policies: Vec<domain::McpInstanceDiscoveryPolicyRecord>,
) -> Result<Vec<McpInstanceDiscoveryPolicyResponse>, ApiError> {
    let instance_ids = instances
        .iter()
        .map(|instance| (instance.id, instance.instance_id.clone()))
        .collect::<HashMap<_, _>>();
    policies
        .into_iter()
        .map(|policy| {
            let instance_id = instance_ids
                .get(&policy.instance_record_id)
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "MCP discovery policy references missing instance record {}",
                        policy.instance_record_id
                    )
                })?;
            Ok(to_discovery_policy_response(policy, instance_id))
        })
        .collect()
}

fn to_interface_response(
    entry: domain::McpInterfaceCatalogEntry,
) -> McpInterfaceCatalogEntryResponse {
    McpInterfaceCatalogEntryResponse {
        interface_id: entry.interface_id,
        method: entry.method,
        path: entry.path,
        name: entry.name,
        short_description: entry.short_description,
        parameter_descriptors: entry
            .parameter_descriptors
            .into_iter()
            .map(to_parameter_descriptor_response)
            .collect(),
        parameter_schema: entry.parameter_schema,
        result_schema: entry.result_schema,
        permission_code: entry.permission_code,
        security: entry.security,
        risk_level: entry.risk_level.as_str().into(),
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason,
    }
}

fn to_parameter_descriptor_response(
    descriptor: McpParameterDescriptor,
) -> McpParameterDescriptorResponse {
    McpParameterDescriptorResponse {
        name: descriptor.name,
        field_type: descriptor.field_type,
        parameter_type: descriptor.parameter_type.as_str().into(),
        description: descriptor.description,
        required: descriptor.required,
        schema: descriptor.schema,
    }
}

fn list_response_field_set(value: &serde_json::Value) -> Result<BTreeSet<String>, ApiError> {
    let Some(fields) = value.as_array() else {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("list_return_fields").into(),
        );
    };
    let mut field_set = BTreeSet::new();
    for field in fields {
        let Some(field) = field.as_str() else {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "list_return_fields",
            )
            .into());
        };
        field_set.insert(field.to_string());
    }
    Ok(field_set)
}

fn includes_list_response_field(fields: &BTreeSet<String>, field: &str) -> bool {
    fields.contains(field) || (field == "item_kind" && fields.contains("type"))
}

fn to_list_item_response(
    item: domain::McpListItemSummary,
    fields: &BTreeSet<String>,
) -> McpListItemSummaryResponse {
    let item_kind = match item.item_kind {
        domain::McpListItemKind::Group => "group".to_string(),
        domain::McpListItemKind::Tool => "tool".to_string(),
    };
    McpListItemSummaryResponse {
        id: if includes_list_response_field(fields, "id") {
            Some(item.id)
        } else {
            None
        },
        item_kind: if includes_list_response_field(fields, "item_kind") {
            Some(item_kind)
        } else {
            None
        },
        path: if includes_list_response_field(fields, "path") {
            Some(item.path)
        } else {
            None
        },
        name: if includes_list_response_field(fields, "name") {
            Some(item.name)
        } else {
            None
        },
        description_short: if includes_list_response_field(fields, "description_short") {
            item.description_short
        } else {
            None
        },
        children_count: if includes_list_response_field(fields, "children_count") {
            Some(item.children_count)
        } else {
            None
        },
        risk_level: if includes_list_response_field(fields, "risk_level") {
            item.risk_level.map(|risk| risk.as_str().into())
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn operation(id: &str, method: &str, path: &str) -> DocsCatalogOperation {
        DocsCatalogOperation {
            id: id.into(),
            method: method.into(),
            path: path.into(),
            summary: None,
            description: None,
            tags: Vec::new(),
            group: "settings".into(),
            deprecated: false,
        }
    }

    #[test]
    fn mcp_interface_descriptors_classify_url_json_body_and_form_parameters() {
        let spec = json!({
            "paths": {
                "/api/console/widgets/{widget_id}": {
                    "parameters": [
                        {
                            "name": "widget_id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "post": {
                        "operationId": "create_widget",
                        "parameters": [
                            {
                                "name": "locale",
                                "in": "query",
                                "required": false,
                                "schema": { "type": "string" }
                            }
                        ],
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "required": ["title"],
                                        "properties": {
                                            "title": {
                                                "type": "string",
                                                "description": "Widget title"
                                            },
                                            "enabled": { "type": "boolean" }
                                        }
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object" }
                                    }
                                }
                            }
                        }
                    }
                },
                "/api/console/uploads": {
                    "post": {
                        "operationId": "upload_widget",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "multipart/form-data": {
                                    "schema": {
                                        "type": "object",
                                        "required": ["file"],
                                        "properties": {
                                            "file": { "type": "string", "format": "binary" },
                                            "label": { "type": "string" }
                                        }
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let json_entry = mcp_interface_entry_from_operation(
            &operation("create_widget", "POST", "/api/console/widgets/{widget_id}"),
            &spec,
            McpInterfaceCapabilitySource::StaticApiDocs,
        )
        .expect("JSON operation should become an MCP interface entry");
        assert!(json_entry
            .parameter_descriptors
            .iter()
            .any(|descriptor| descriptor.name == "widget_id"
                && descriptor.parameter_type == McpParameterType::Url
                && descriptor.required));
        assert!(json_entry
            .parameter_descriptors
            .iter()
            .any(|descriptor| descriptor.name == "locale"
                && descriptor.parameter_type == McpParameterType::Url
                && !descriptor.required));
        assert!(json_entry
            .parameter_descriptors
            .iter()
            .any(|descriptor| descriptor.name == "title"
                && descriptor.parameter_type == McpParameterType::JsonBody
                && descriptor.field_type == "string"
                && descriptor.required
                && descriptor.description.as_deref() == Some("Widget title")));
        assert!(json_entry
            .parameter_descriptors
            .iter()
            .any(|descriptor| descriptor.name == "enabled"
                && descriptor.parameter_type == McpParameterType::JsonBody
                && descriptor.field_type == "boolean"
                && !descriptor.required));

        let form_entry = mcp_interface_entry_from_operation(
            &operation("upload_widget", "POST", "/api/console/uploads"),
            &spec,
            McpInterfaceCapabilitySource::StaticApiDocs,
        )
        .expect("form operation should become an MCP interface entry");
        assert!(form_entry
            .parameter_descriptors
            .iter()
            .any(|descriptor| descriptor.name == "file"
                && descriptor.parameter_type == McpParameterType::Form
                && descriptor.required));
        assert!(form_entry
            .parameter_descriptors
            .iter()
            .any(|descriptor| descriptor.name == "label"
                && descriptor.parameter_type == McpParameterType::Form
                && !descriptor.required));
    }

    #[test]
    fn mcp_interface_descriptors_expand_nested_json_body_schema_properties() {
        let spec = json!({
            "paths": {
                "/api/console/applications/{application_id}/api-publications": {
                    "parameters": [
                        {
                            "name": "application_id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }
                    ],
                    "post": {
                        "operationId": "publish_application_api",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "required": ["api_enabled", "mapping"],
                                        "properties": {
                                            "api_enabled": { "type": "boolean" },
                                            "mapping": {
                                                "type": "object",
                                                "required": ["input", "output"],
                                                "properties": {
                                                    "input": {
                                                        "type": "object",
                                                        "required": ["query_target"],
                                                        "properties": {
                                                            "query_target": {
                                                                "type": "string",
                                                                "description": "Query target"
                                                            },
                                                            "history_target": { "type": "string" }
                                                        }
                                                    },
                                                    "output": {
                                                        "type": "object",
                                                        "properties": {
                                                            "answer_selector": { "type": "string" },
                                                            "usage_selector": { "type": "string" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object" }
                                    }
                                }
                            }
                        }
                    }
                },
                "/api/console/optional-publications": {
                    "post": {
                        "operationId": "optional_publish_application_api",
                        "requestBody": {
                            "required": false,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "required": ["mapping"],
                                        "properties": {
                                            "mapping": {
                                                "type": "object",
                                                "required": ["input"],
                                                "properties": {
                                                    "input": {
                                                        "type": "object",
                                                        "required": ["query_target"],
                                                        "properties": {
                                                            "query_target": { "type": "string" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let entry = mcp_interface_entry_from_operation(
            &operation(
                "publish_application_api",
                "POST",
                "/api/console/applications/{application_id}/api-publications",
            ),
            &spec,
            McpInterfaceCapabilitySource::StaticApiDocs,
        )
        .expect("publish operation should become an MCP interface entry");

        let descriptor = |name: &str| {
            entry
                .parameter_descriptors
                .iter()
                .find(|descriptor| descriptor.name == name)
                .unwrap_or_else(|| panic!("missing descriptor {name}"))
        };

        assert_eq!(
            entry
                .parameter_descriptors
                .iter()
                .map(|descriptor| descriptor.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "application_id",
                "api_enabled",
                "mapping.input.query_target",
                "mapping.input.history_target",
                "mapping.output.answer_selector",
                "mapping.output.usage_selector",
            ]
        );
        assert_eq!(
            descriptor("mapping.input.query_target").parameter_type,
            McpParameterType::JsonBody
        );
        assert_eq!(
            descriptor("mapping.input.query_target").field_type,
            "string"
        );
        assert_eq!(
            descriptor("mapping.input.query_target")
                .description
                .as_deref(),
            Some("Query target")
        );
        assert!(descriptor("api_enabled").required);
        assert!(descriptor("mapping.input.query_target").required);
        assert!(!descriptor("mapping.input.history_target").required);
        assert!(!descriptor("mapping.output.answer_selector").required);

        let optional_entry = mcp_interface_entry_from_operation(
            &operation(
                "optional_publish_application_api",
                "POST",
                "/api/console/optional-publications",
            ),
            &spec,
            McpInterfaceCapabilitySource::StaticApiDocs,
        )
        .expect("optional publish operation should become an MCP interface entry");
        let optional_descriptor = optional_entry
            .parameter_descriptors
            .iter()
            .find(|descriptor| descriptor.name == "mapping.input.query_target")
            .expect("optional body should still expose nested descriptor");
        assert!(!optional_descriptor.required);
    }

    #[test]
    fn mcp_interface_descriptors_keep_non_object_json_body_fallback() {
        let spec = json!({
            "paths": {
                "/api/console/raw-body": {
                    "post": {
                        "operationId": "submit_raw_body",
                        "requestBody": {
                            "required": true,
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "string",
                                        "description": "Raw body"
                                    }
                                }
                            }
                        },
                        "responses": {
                            "200": {
                                "description": "ok",
                                "content": {
                                    "application/json": {
                                        "schema": { "type": "object" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        let entry = mcp_interface_entry_from_operation(
            &operation("submit_raw_body", "POST", "/api/console/raw-body"),
            &spec,
            McpInterfaceCapabilitySource::StaticApiDocs,
        )
        .expect("raw body operation should become an MCP interface entry");

        assert_eq!(entry.parameter_descriptors.len(), 1);
        let descriptor = &entry.parameter_descriptors[0];
        assert_eq!(descriptor.name, "body");
        assert_eq!(descriptor.field_type, "string");
        assert_eq!(descriptor.parameter_type, McpParameterType::JsonBody);
        assert_eq!(descriptor.description.as_deref(), Some("Raw body"));
        assert!(descriptor.required);
    }
}
