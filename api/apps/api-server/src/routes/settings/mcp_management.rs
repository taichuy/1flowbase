pub(crate) mod bundles;
pub(crate) mod debug_execute;
pub(crate) mod upstream;
pub(crate) mod upstream_client;

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use control_plane::mcp_management::{
    CopyMcpInstanceCommand, CreateMcpInstanceCommand, CreateMcpToolBindingCommand,
    CreateMcpToolCommand, McpManagementService, MoveMcpGroupCommand,
    RefreshMcpToolDescriptionCommand, SaveMcpClientCredentialCommand,
    UpdateMcpInstanceDiscoveryPolicyCommand, UpdateMcpProxyToolCommand,
    UpdateMcpToolBindingCommand, UpdateMcpToolCommand, UpsertMcpGroupCommand,
};
use control_plane::{
    application_public_api::published_workflow_operation::build_published_workflow_operations,
    ports::{ApplicationPublicationRepository, AuthRepository},
};
use domain::mcp_management::{McpParameterDescriptor, McpParameterType};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    openapi_docs::DocsCatalogOperation,
    openapi_interface::{
        build_openapi_capability_catalog, OpenApiCapabilityCatalogEntry, OpenApiParameterLocation,
    },
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_post, console_put, ConsoleRouteAssembly,
    },
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
    pub execution_target: McpToolExecutionTargetDto,
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
    pub availability_status: McpToolAvailabilityStatusDto,
    pub availability_reason: Option<String>,
    pub revision: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpToolAvailabilityStatusDto {
    Available,
    InterfaceMissing,
    UpstreamDisabled,
    CredentialsMissing,
    UpstreamToolMissing,
    MappingInvalid,
}

impl From<domain::McpToolAvailabilityStatus> for McpToolAvailabilityStatusDto {
    fn from(status: domain::McpToolAvailabilityStatus) -> Self {
        match status {
            domain::McpToolAvailabilityStatus::Available => Self::Available,
            domain::McpToolAvailabilityStatus::InterfaceMissing => Self::InterfaceMissing,
            domain::McpToolAvailabilityStatus::UpstreamDisabled => Self::UpstreamDisabled,
            domain::McpToolAvailabilityStatus::CredentialsMissing => Self::CredentialsMissing,
            domain::McpToolAvailabilityStatus::UpstreamToolMissing => Self::UpstreamToolMissing,
            domain::McpToolAvailabilityStatus::MappingInvalid => Self::MappingInvalid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpToolExecutionTargetDto {
    InterfaceWrapper {
        interface_id: String,
    },
    McpProxy {
        upstream_connection_id: String,
        remote_tool_name: String,
        source_schema_hash: String,
    },
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
    pub parameter_schema: serde_json::Value,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMcpInstanceBody {
    pub instance_id: String,
    pub name: String,
    pub description_short: Option<String>,
    pub status: String,
    pub default_entry_path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyMcpInstanceBody {
    pub instance_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertMcpGroupBody {
    pub path: String,
    pub display_name: String,
    pub description_short: Option<String>,
    pub enabled: bool,
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveMcpGroupBody {
    pub source_path: String,
    pub target_parent_path: String,
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
    pub execution_target: McpToolExecutionTargetDto,
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
    pub execution_target: McpToolExecutionTargetDto,
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
    pub keywords: Option<Vec<String>>,
    pub depth: Option<i32>,
    pub path_regex: Option<String>,
    pub limit: Option<usize>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/mcp/catalog",
            console_get(
                get_mcp_catalog,
                ConsoleOperation("mcp.catalog.view".to_string()),
            ),
        )
        .route(
            "/mcp/interface-capabilities",
            console_get(
                list_mcp_interface_capabilities,
                ConsoleOperation("mcp.catalog.view".to_string()),
            ),
        )
        .route(
            "/mcp/list",
            console_get(
                list_mcp_items,
                ConsoleOperation("mcp.catalog.view".to_string()),
            ),
        )
        .route(
            "/mcp/export",
            console_get(
                export_mcp_catalog,
                ConsoleOperation("mcp.catalog.export".to_string()),
            ),
        )
        .route(
            "/mcp/instances",
            console_get(
                list_mcp_instances,
                ConsoleOperation("mcp.instances.view".to_string()),
            )
            .post(
                create_mcp_instance,
                ConsoleOperation("mcp.instances.create".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/copy",
            console_post(
                copy_mcp_instance,
                ConsoleOperation("mcp.instances.copy".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id",
            console_put(
                update_mcp_instance,
                ConsoleOperation("mcp.instances.update".to_string()),
            )
            .delete(
                delete_mcp_instance,
                ConsoleOperation("mcp.instances.delete".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/client-credential",
            console_get(
                get_mcp_client_credential,
                ConsoleOperation("mcp.client_credential.reveal".to_string()),
            )
            .put(
                save_mcp_client_credential,
                ConsoleOperation("mcp.client_credential.save".to_string()),
            )
            .delete(
                delete_mcp_client_credential,
                ConsoleOperation("mcp.client_credential.delete".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/groups",
            console_post(
                upsert_mcp_group,
                ConsoleOperation("mcp.groups.upsert".to_string()),
            )
            .delete(
                delete_mcp_group,
                ConsoleOperation("mcp.groups.delete".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/groups/move",
            console_post(
                move_mcp_group,
                ConsoleOperation("mcp.groups.move".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/tool-bindings",
            console_post(
                create_mcp_tool_binding,
                ConsoleOperation("mcp.tool_bindings.create".to_string()),
            ),
        )
        .route(
            "/mcp/tool-bindings/:binding_id",
            console_put(
                update_mcp_tool_binding,
                ConsoleOperation("mcp.tool_bindings.update".to_string()),
            )
            .delete(
                delete_mcp_tool_binding,
                ConsoleOperation("mcp.tool_bindings.delete".to_string()),
            ),
        )
        .route(
            "/mcp/tools",
            console_get(
                list_mcp_tools,
                ConsoleOperation("mcp.tools.view".to_string()),
            )
            .post(
                create_mcp_tool,
                ConsoleOperation("mcp.tools.create".to_string()),
            ),
        )
        .route(
            "/mcp/tools/:tool_id",
            console_get(get_mcp_tool, ConsoleOperation("mcp.tools.view".to_string()))
                .put(
                    update_mcp_tool,
                    ConsoleOperation("mcp.tools.update".to_string()),
                )
                .delete(
                    delete_mcp_tool,
                    ConsoleOperation("mcp.tools.delete".to_string()),
                ),
        )
        .route(
            "/mcp/tools/:tool_id/description/refresh",
            console_post(
                refresh_mcp_tool_description,
                ConsoleOperation("mcp.tools.description.refresh".to_string()),
            ),
        )
        .route(
            "/mcp/tools/:tool_id/description-check",
            console_post(
                check_mcp_tool_description,
                ConsoleOperation("mcp.tools.description.check".to_string()),
            ),
        )
        .route(
            "/mcp/debug/execute",
            console_post(
                execute_mcp_debug,
                ConsoleOperation("mcp.debug.execute".to_string()),
            ),
        )
        .route(
            "/mcp/instances/:instance_id/discovery-policy",
            console_get(
                get_mcp_instance_discovery_policy,
                ConsoleOperation("mcp.discovery_policy.view".to_string()),
            )
            .put(
                update_mcp_instance_discovery_policy,
                ConsoleOperation("mcp.discovery_policy.update".to_string()),
            ),
        )
        .merge(bundles::route_assembly())
        .merge(upstream::route_assembly())
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
            query.keywords.as_deref(),
            query.depth,
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

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/copy", request_body = CopyMcpInstanceBody, responses((status = 201, body = McpInstanceResponse)))]
pub async fn copy_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(source_instance_id): Path<String>,
    Json(body): Json<CopyMcpInstanceBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpInstanceResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .copy_instance(CopyMcpInstanceCommand {
            actor_user_id: context.user.id,
            source_instance_id,
            instance_id: body.instance_id,
            name: body.name,
        })
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

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/groups/move", request_body = MoveMcpGroupBody, responses((status = 200, body = McpGroupResponse)))]
pub async fn move_mcp_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<MoveMcpGroupBody>,
) -> Result<Json<ApiSuccess<McpGroupResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let record = McpManagementService::new(state.store.clone())
        .move_group(MoveMcpGroupCommand {
            actor_user_id: context.user.id,
            instance_id,
            source_path: body.source_path,
            target_parent_path: body.target_parent_path,
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
    let mut tools = Vec::with_capacity(snapshot.tools.len());
    for record in snapshot.tools {
        tools.push(
            to_tool_response_for_actor(state.as_ref(), context.user.id, record, &operations)
                .await?,
        );
    }
    Ok(Json(ApiSuccess::new(tools)))
}

#[utoipa::path(post, path = "/api/console/mcp/tools", request_body = CreateMcpToolBody, responses((status = 201, body = McpToolResponse)))]
pub async fn create_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateMcpToolBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpToolResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let interface_id = interface_target_id(&body.execution_target)?;
    let interface_entry =
        bindable_mcp_interface(state.as_ref(), context.user.id, interface_id).await?;
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
            record,
            operation,
            domain::McpToolAvailabilityStatus::Available,
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
    Ok(Json(ApiSuccess::new(
        to_tool_response_for_actor(state.as_ref(), context.user.id, record, &operations).await?,
    )))
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
    match &body.execution_target {
        McpToolExecutionTargetDto::InterfaceWrapper { interface_id } => {
            let interface_entry =
                bindable_mcp_interface(state.as_ref(), context.user.id, interface_id).await?;
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
                record,
                operation,
                domain::McpToolAvailabilityStatus::Available,
            ))))
        }
        McpToolExecutionTargetDto::McpProxy { .. } => {
            let execution_target = to_domain_execution_target(&body.execution_target)?;
            let record = McpManagementService::new(state.store.clone())
                .update_proxy_tool(UpdateMcpProxyToolCommand {
                    actor_user_id: context.user.id,
                    tool_id,
                    des_id: body.des_id,
                    name: body.name,
                    short_description: body.short_description,
                    full_description: body.full_description,
                    execution_target,
                    parameter_schema: body.parameter_schema,
                    result_schema: body.result_schema,
                    input_mapping: body.input_mapping,
                    output_mapping: body.output_mapping,
                    risk_level: parse_risk_level(&body.risk_level)?,
                    status: parse_tool_status(&body.status)?,
                })
                .await?;
            let operations = mcp_interface_operation_map(state.as_ref(), context.user.id).await?;
            Ok(Json(ApiSuccess::new(
                to_tool_response_for_actor(state.as_ref(), context.user.id, record, &operations)
                    .await?,
            )))
        }
    }
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

fn interface_target_id(target: &McpToolExecutionTargetDto) -> Result<&str, ApiError> {
    match target {
        McpToolExecutionTargetDto::InterfaceWrapper { interface_id } => Ok(interface_id),
        McpToolExecutionTargetDto::McpProxy { .. } => {
            Err(control_plane::errors::ControlPlaneError::InvalidInput("execution_target").into())
        }
    }
}

fn to_domain_execution_target(
    target: &McpToolExecutionTargetDto,
) -> Result<domain::McpToolExecutionTarget, ApiError> {
    Ok(match target {
        McpToolExecutionTargetDto::InterfaceWrapper { interface_id } => {
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: interface_id.clone(),
            }
        }
        McpToolExecutionTargetDto::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            source_schema_hash,
        } => domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id: parse_uuid(upstream_connection_id, "upstream_connection_id")?,
            remote_tool_name: remote_tool_name.clone(),
            source_schema_hash: source_schema_hash.clone(),
        },
    })
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

fn parse_risk_level(value: &str) -> Result<domain::McpRiskLevel, ApiError> {
    match value {
        "low" => Ok(domain::McpRiskLevel::Low),
        "medium" => Ok(domain::McpRiskLevel::Medium),
        "high" => Ok(domain::McpRiskLevel::High),
        "critical" => Ok(domain::McpRiskLevel::Critical),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("risk_level").into()),
    }
}

async fn mcp_interface_catalog_entries(
    state: &ApiState,
    actor_user_id: Uuid,
) -> Result<Vec<domain::McpInterfaceCatalogEntry>, ApiError> {
    let actor = state
        .store
        .load_actor_context_for_user(actor_user_id)
        .await?;
    let mut entries = build_openapi_capability_catalog(state, actor.current_workspace_id)
        .await?
        .into_iter()
        .map(mcp_interface_entry_from_capability)
        .collect::<Vec<_>>();
    let publications = state.store.list_enabled_extension_publications().await?;
    let operations = build_published_workflow_operations(publications)
        .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("workflow_route"))?;
    for operation in operations
        .into_iter()
        .filter(|operation| operation.workspace_id == actor.current_workspace_id)
    {
        let path = operation.public_path();
        let method = operation.method.as_str().to_string();
        let docs_operation = DocsCatalogOperation {
            id: operation.interface_id.clone(),
            method: method.clone(),
            path: path.clone(),
            summary: Some(format!(
                "Invoke published workflow {}",
                operation.application_id
            )),
            description: Some("Invoke the active publication of a Workflow application".into()),
            tags: vec!["Workflow Extensions".into()],
            group: "workflow_extensions".into(),
            deprecated: false,
        };
        let spec = serde_json::json!({
            "openapi": "3.1.0",
            "paths": {
                (path): {
                    (method.to_ascii_lowercase()): crate::openapi::workflow_extension_operation(&operation)
                }
            }
        });
        if let Some(entry) = mcp_interface_entry_from_operation(&docs_operation, &spec) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn mcp_interface_entry_from_capability(
    entry: OpenApiCapabilityCatalogEntry,
) -> domain::McpInterfaceCatalogEntry {
    let interface = entry.interface;
    domain::McpInterfaceCatalogEntry {
        interface_id: interface.operation_id,
        method: interface.method.clone(),
        path: interface.path.clone(),
        name: interface.name,
        short_description: interface.description,
        parameter_descriptors: interface
            .parameter_descriptors
            .into_iter()
            .filter_map(|descriptor| {
                let parameter_type = match descriptor.location {
                    OpenApiParameterLocation::Path | OpenApiParameterLocation::Query => {
                        McpParameterType::Url
                    }
                    OpenApiParameterLocation::JsonBody => McpParameterType::JsonBody,
                    OpenApiParameterLocation::FormBody => McpParameterType::Form,
                    OpenApiParameterLocation::Header => return None,
                };
                Some(McpParameterDescriptor {
                    name: descriptor.name,
                    field_type: descriptor.field_type,
                    parameter_type,
                    description: descriptor.description,
                    required: descriptor.required,
                    schema: descriptor.schema,
                })
            })
            .collect(),
        parameter_schema: interface.request_schema,
        result_schema: interface.response_schema,
        permission_code: operation_permission_code(&interface.method, &interface.path),
        security: interface.security,
        risk_level: mcp_risk_level(entry.risk_level),
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason.map(str::to_string),
    }
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
) -> Option<domain::McpInterfaceCatalogEntry> {
    let interface = crate::openapi_interface::catalog_entry_from_operation(operation, spec)?;

    Some(domain::McpInterfaceCatalogEntry {
        interface_id: interface.operation_id,
        method: interface.method,
        path: interface.path,
        name: interface.name,
        short_description: interface.description,
        parameter_descriptors: interface
            .parameter_descriptors
            .into_iter()
            .filter_map(|descriptor| {
                use crate::openapi_interface::OpenApiParameterLocation;
                let parameter_type = match descriptor.location {
                    OpenApiParameterLocation::Path | OpenApiParameterLocation::Query => {
                        McpParameterType::Url
                    }
                    OpenApiParameterLocation::JsonBody => McpParameterType::JsonBody,
                    OpenApiParameterLocation::FormBody => McpParameterType::Form,
                    OpenApiParameterLocation::Header => return None,
                };
                Some(McpParameterDescriptor {
                    name: descriptor.name,
                    field_type: descriptor.field_type,
                    parameter_type,
                    description: descriptor.description,
                    required: descriptor.required,
                    schema: descriptor.schema,
                })
            })
            .collect(),
        parameter_schema: interface.request_schema,
        result_schema: interface.response_schema,
        permission_code: operation_permission_code(&operation.method, &operation.path),
        security: interface.security,
        risk_level: operation_risk_level(&operation.method),
        bindable: true,
        disabled_reason: None,
    })
}

fn operation_risk_level(method: &str) -> domain::McpRiskLevel {
    mcp_risk_level(crate::openapi_interface::operation_risk_level(method))
}

fn mcp_risk_level(risk_level: &str) -> domain::McpRiskLevel {
    match risk_level {
        "low" => domain::McpRiskLevel::Low,
        "medium" => domain::McpRiskLevel::Medium,
        "high" => domain::McpRiskLevel::High,
        "critical" => domain::McpRiskLevel::Critical,
        _ => unreachable!("shared OpenAPI capability catalog emitted an unknown risk level"),
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
    if path.starts_with("/api/console/settings/members") {
        return permission_code(access_control::SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/settings/roles") {
        return permission_code(access_control::SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/docs/") {
        return permission_code(access_control::SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/user-api-keys") {
        return permission_code(
            access_control::SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_PERMISSION,
        );
    }
    if path.starts_with("/api/console/system/runtime-profile")
        || path.starts_with("/api/console/system/release-status")
    {
        return permission_code(access_control::SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION);
    }
    if path.starts_with("/api/console/workspace") || path.starts_with("/api/console/workspaces") {
        return view_or_configure_permission(method, "workspace");
    }
    if path.starts_with("/api/console/mcp/") {
        return permission_code(access_control::SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION);
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
    let (operation, availability_status) = match &record.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            let available_operation = operations.get(interface_id).cloned();
            let availability_status = if available_operation.is_some() {
                domain::McpToolAvailabilityStatus::Available
            } else {
                domain::McpToolAvailabilityStatus::InterfaceMissing
            };
            (
                available_operation.unwrap_or_else(|| interface_id.clone()),
                availability_status,
            )
        }
        domain::McpToolExecutionTarget::McpProxy {
            remote_tool_name, ..
        } => (
            format!("MCP tools/call {remote_tool_name}"),
            domain::McpToolAvailabilityStatus::Available,
        ),
    };

    to_tool_response_with_operation(record, operation, availability_status)
}

async fn to_tool_response_for_actor(
    state: &ApiState,
    actor_user_id: Uuid,
    record: domain::McpToolRecord,
    operations: &HashMap<String, String>,
) -> Result<McpToolResponse, ApiError> {
    let availability = match &record.execution_target {
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } => Some(
            McpManagementService::new(state.store.clone())
                .upstream_proxy_availability(
                    actor_user_id,
                    *upstream_connection_id,
                    remote_tool_name,
                )
                .await?,
        ),
        domain::McpToolExecutionTarget::InterfaceWrapper { .. } => None,
    };
    if let Some(availability) = availability {
        let operation = match &record.execution_target {
            domain::McpToolExecutionTarget::McpProxy {
                remote_tool_name, ..
            } => {
                format!("MCP tools/call {remote_tool_name}")
            }
            domain::McpToolExecutionTarget::InterfaceWrapper { .. } => String::new(),
        };
        Ok(to_tool_response_with_operation(
            record,
            operation,
            availability,
        ))
    } else {
        Ok(to_tool_response(record, operations))
    }
}

pub(super) fn to_tool_response_with_operation(
    record: domain::McpToolRecord,
    operation: String,
    availability_status: domain::McpToolAvailabilityStatus,
) -> McpToolResponse {
    McpToolResponse {
        id: record.id.to_string(),
        workspace_id: record.workspace_id.to_string(),
        tool_id: record.tool_id,
        name: record.name,
        short_description: record.short_description,
        full_description: record.full_description,
        execution_target: match record.execution_target {
            domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
                McpToolExecutionTargetDto::InterfaceWrapper { interface_id }
            }
            domain::McpToolExecutionTarget::McpProxy {
                upstream_connection_id,
                remote_tool_name,
                source_schema_hash,
            } => McpToolExecutionTargetDto::McpProxy {
                upstream_connection_id: upstream_connection_id.to_string(),
                remote_tool_name,
                source_schema_hash,
            },
        },
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
        availability_status: availability_status.into(),
        availability_reason: (availability_status != domain::McpToolAvailabilityStatus::Available)
            .then(|| availability_status.as_str().to_string()),
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

    #[test]
    fn ac_007_published_workflow_interface_is_bindable_with_stable_identity() {
        let path = "/api/ex/orders/{order_id}";
        let spec = json!({
            "paths": {
                (path): {
                    "post": {
                        "operationId": "published_workflow_operation:11111111-1111-1111-1111-111111111111",
                        "security": [{ "UserApiKey": [] }],
                        "parameters": [{
                            "name": "order_id",
                            "in": "path",
                            "required": true,
                            "schema": { "type": "string" }
                        }],
                        "responses": { "200": { "description": "Workflow Result", "content": {
                            "application/json": { "schema": { "type": "object", "properties": {
                                "accepted": { "type": "boolean" }
                            } } }
                        } } }
                    }
                }
            }
        });
        let entry = mcp_interface_entry_from_operation(
            &operation(
                "published_workflow_operation:11111111-1111-1111-1111-111111111111",
                "POST",
                path,
            ),
            &spec,
        )
        .expect("published workflow operation should become an MCP interface");

        assert!(entry.bindable);
        assert_eq!(
            entry.interface_id,
            "published_workflow_operation:11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(entry.parameter_descriptors[0].name, "order_id");
        assert_eq!(entry.security, json!([{ "UserApiKey": [] }]));
        assert_eq!(
            entry.result_schema["properties"]["accepted"]["type"],
            json!("boolean")
        );
    }
}
