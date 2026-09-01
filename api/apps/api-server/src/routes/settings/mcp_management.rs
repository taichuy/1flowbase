pub(crate) mod bundles;
pub(crate) mod debug_execute;
mod dto;
pub(crate) mod interface_catalog;
pub(crate) mod interface_catalog_routes;
pub(crate) mod interface_core;
pub(crate) mod interface_tools;
pub(crate) use interface_catalog::mcp_interface_entry_from_capability;
mod projections;
pub(crate) mod upstream;
pub(crate) mod upstream_client;

pub(crate) use interface_catalog::bindable_mcp_interface;
pub(crate) use interface_catalog::*;
use projections::*;

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
    ports::ApplicationPublicationRepository,
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
    openapi_interface::{OpenApiCapabilityCatalogEntry, OpenApiParameterLocation},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_post, console_put, ConsoleRouteAssembly,
    },
};

pub use debug_execute::{
    McpDebugExecuteBody, McpDebugExecuteDetailsResponse, McpDebugResponseMode,
};

pub use dto::*;

async fn invoke_core(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface_core::McpCoreInput,
    mutating: bool,
) -> Result<interface_core::McpCoreOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}

async fn invoke_catalog(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface_catalog_routes::McpCatalogInput,
) -> Result<interface_catalog_routes::McpCatalogOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential =
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}

async fn invoke_tools(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: interface_tools::McpToolsInput,
    mutating: bool,
) -> Result<interface_tools::McpToolsOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
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
    let interface_core::McpCoreOutput::Credential(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.client-credential.get.v1",
        interface_core::McpCoreInput::GetCredential(instance_id),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

pub async fn save_mcp_client_credential(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<SaveMcpClientCredentialBody>,
) -> Result<Json<ApiSuccess<McpClientCredentialResponse>>, ApiError> {
    let interface_core::McpCoreOutput::Credential(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.client-credential.put.v1",
        interface_core::McpCoreInput::SaveCredential(instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

pub async fn delete_mcp_client_credential(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let interface_core::McpCoreOutput::NoContent = invoke_core(
        state,
        headers,
        "http.console.mcp.client-credential.delete.v1",
        interface_core::McpCoreInput::DeleteCredential(instance_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/mcp/catalog", responses((status = 200, body = McpCatalogResponse)))]
pub async fn get_mcp_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpCatalogResponse>>, ApiError> {
    let interface_catalog_routes::McpCatalogOutput::Catalog(value) = invoke_catalog(
        state,
        headers,
        "http.console.mcp.catalog.get.v1",
        interface_catalog_routes::McpCatalogInput::Catalog,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/mcp/interface-capabilities", params(McpInterfaceCatalogQuery), responses((status = 200, body = [McpInterfaceCatalogEntryResponse])))]
pub async fn list_mcp_interface_capabilities(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpInterfaceCatalogQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpInterfaceCatalogEntryResponse>>>, ApiError> {
    let interface_catalog_routes::McpCatalogOutput::Interfaces(value) = invoke_catalog(
        state,
        headers,
        "http.console.mcp.interfaces.get.v1",
        interface_catalog_routes::McpCatalogInput::Interfaces(query),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/mcp/list", params(McpListQuery), responses((status = 200, body = [McpListItemSummaryResponse])))]
pub async fn list_mcp_items(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<McpListQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpListItemSummaryResponse>>>, ApiError> {
    let interface_catalog_routes::McpCatalogOutput::List(value) = invoke_catalog(
        state,
        headers,
        "http.console.mcp.list.get.v1",
        interface_catalog_routes::McpCatalogInput::List(query),
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/mcp/export", responses((status = 200, body = McpExportPackageResponse)))]
pub async fn export_mcp_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpExportPackageResponse>>, ApiError> {
    let interface_catalog_routes::McpCatalogOutput::Export(value) = invoke_catalog(
        state,
        headers,
        "http.console.mcp.export.get.v1",
        interface_catalog_routes::McpCatalogInput::Export,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/mcp/instances", responses((status = 200, body = [McpInstanceResponse])))]
pub async fn list_mcp_instances(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpInstanceResponse>>>, ApiError> {
    let interface_core::McpCoreOutput::Instances(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.instances.get.v1",
        interface_core::McpCoreInput::ListInstances,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/mcp/instances", request_body = CreateMcpInstanceBody, responses((status = 201, body = McpInstanceResponse)))]
pub async fn create_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateMcpInstanceBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpInstanceResponse>>), ApiError> {
    let interface_core::McpCoreOutput::Instance(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.instances.post.v1",
        interface_core::McpCoreInput::CreateInstance(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/copy", request_body = CopyMcpInstanceBody, responses((status = 201, body = McpInstanceResponse)))]
pub async fn copy_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(source_instance_id): Path<String>,
    Json(body): Json<CopyMcpInstanceBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpInstanceResponse>>), ApiError> {
    let interface_core::McpCoreOutput::Instance(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.instances.copy.v1",
        interface_core::McpCoreInput::CopyInstance(source_instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}

#[utoipa::path(put, path = "/api/console/mcp/instances/{instance_id}", request_body = CreateMcpInstanceBody, responses((status = 200, body = McpInstanceResponse)))]
pub async fn update_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<CreateMcpInstanceBody>,
) -> Result<Json<ApiSuccess<McpInstanceResponse>>, ApiError> {
    let interface_core::McpCoreOutput::Instance(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.instances.put.v1",
        interface_core::McpCoreInput::UpdateInstance(instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(delete, path = "/api/console/mcp/instances/{instance_id}", responses((status = 204)))]
pub async fn delete_mcp_instance(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let interface_core::McpCoreOutput::NoContent = invoke_core(
        state,
        headers,
        "http.console.mcp.instances.delete.v1",
        interface_core::McpCoreInput::DeleteInstance(instance_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/groups", request_body = UpsertMcpGroupBody, responses((status = 200, body = McpGroupResponse)))]
pub async fn upsert_mcp_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<UpsertMcpGroupBody>,
) -> Result<Json<ApiSuccess<McpGroupResponse>>, ApiError> {
    let interface_core::McpCoreOutput::Group(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.groups.post.v1",
        interface_core::McpCoreInput::UpsertGroup(instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/mcp/instances/{instance_id}/groups/move", request_body = MoveMcpGroupBody, responses((status = 200, body = McpGroupResponse)))]
pub async fn move_mcp_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Json(body): Json<MoveMcpGroupBody>,
) -> Result<Json<ApiSuccess<McpGroupResponse>>, ApiError> {
    let interface_core::McpCoreOutput::Group(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.groups.move.v1",
        interface_core::McpCoreInput::MoveGroup(instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(delete, path = "/api/console/mcp/instances/{instance_id}/groups", params(DeleteMcpGroupQuery), responses((status = 204)))]
pub async fn delete_mcp_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(instance_id): Path<String>,
    Query(query): Query<DeleteMcpGroupQuery>,
) -> Result<StatusCode, ApiError> {
    let interface_core::McpCoreOutput::NoContent = invoke_core(
        state,
        headers,
        "http.console.mcp.groups.delete.v1",
        interface_core::McpCoreInput::DeleteGroup(instance_id, query),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/mcp/tools", responses((status = 200, body = [McpToolResponse])))]
pub async fn list_mcp_tools(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<McpToolResponse>>>, ApiError> {
    let interface_tools::McpToolsOutput::Tools(value) = invoke_tools(
        state,
        headers,
        "http.console.mcp.tools.get.v1",
        interface_tools::McpToolsInput::List,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/mcp/tools", request_body = CreateMcpToolBody, responses((status = 201, body = McpToolResponse)))]
pub async fn create_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateMcpToolBody>,
) -> Result<(StatusCode, Json<ApiSuccess<McpToolResponse>>), ApiError> {
    let interface_tools::McpToolsOutput::Tool(value) = invoke_tools(
        state,
        headers,
        "http.console.mcp.tools.post.v1",
        interface_tools::McpToolsInput::Create(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}

#[utoipa::path(get, path = "/api/console/mcp/tools/{tool_id}", responses((status = 200, body = McpToolResponse)))]
pub async fn get_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<Json<ApiSuccess<McpToolResponse>>, ApiError> {
    let interface_tools::McpToolsOutput::Tool(value) = invoke_tools(
        state,
        headers,
        "http.console.mcp.tool.get.v1",
        interface_tools::McpToolsInput::Get(tool_id),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(put, path = "/api/console/mcp/tools/{tool_id}", request_body = UpdateMcpToolBody, responses((status = 200, body = McpToolResponse)))]
pub async fn update_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
    Json(body): Json<UpdateMcpToolBody>,
) -> Result<Json<ApiSuccess<McpToolResponse>>, ApiError> {
    let interface_tools::McpToolsOutput::Tool(value) = invoke_tools(
        state,
        headers,
        "http.console.mcp.tool.put.v1",
        interface_tools::McpToolsInput::Update(tool_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(delete, path = "/api/console/mcp/tools/{tool_id}", responses((status = 204)))]
pub async fn delete_mcp_tool(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let interface_tools::McpToolsOutput::NoContent = invoke_tools(
        state,
        headers,
        "http.console.mcp.tool.delete.v1",
        interface_tools::McpToolsInput::Delete(tool_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/console/mcp/tools/{tool_id}/description/refresh", responses((status = 200, body = McpToolResponse)))]
pub async fn refresh_mcp_tool_description(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<Json<ApiSuccess<McpToolResponse>>, ApiError> {
    let interface_tools::McpToolsOutput::Tool(value) = invoke_tools(
        state,
        headers,
        "http.console.mcp.tool-description.refresh.v1",
        interface_tools::McpToolsInput::RefreshDescription(tool_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/mcp/tools/{tool_id}/description-check", request_body = McpDescriptionCheckBody, responses((status = 200, body = McpDescriptionCheckResponse)))]
pub async fn check_mcp_tool_description(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
    Json(body): Json<McpDescriptionCheckBody>,
) -> Result<Json<ApiSuccess<McpDescriptionCheckResponse>>, ApiError> {
    let interface_tools::McpToolsOutput::Check(value) = invoke_tools(
        state,
        headers,
        "http.console.mcp.tool-description.check.v1",
        interface_tools::McpToolsInput::CheckDescription(tool_id, body),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
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
        bindable_mcp_interface(state.as_ref(), &context.actor, &body.interface_id).await?;

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
    let interface_core::McpCoreOutput::Binding(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.bindings.post.v1",
        interface_core::McpCoreInput::CreateBinding(instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}

#[utoipa::path(put, path = "/api/console/mcp/tool-bindings/{binding_id}", request_body = UpdateMcpToolBindingBody, responses((status = 200, body = McpToolBindingResponse)))]
pub async fn update_mcp_tool_binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(binding_id): Path<String>,
    Json(body): Json<UpdateMcpToolBindingBody>,
) -> Result<Json<ApiSuccess<McpToolBindingResponse>>, ApiError> {
    let interface_core::McpCoreOutput::Binding(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.bindings.put.v1",
        interface_core::McpCoreInput::UpdateBinding(binding_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(delete, path = "/api/console/mcp/tool-bindings/{binding_id}", responses((status = 204)))]
pub async fn delete_mcp_tool_binding(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(binding_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let interface_core::McpCoreOutput::NoContent = invoke_core(
        state,
        headers,
        "http.console.mcp.bindings.delete.v1",
        interface_core::McpCoreInput::DeleteBinding(binding_id),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/mcp/instances/{instance_id}/discovery-policy", responses((status = 200, body = McpInstanceDiscoveryPolicyResponse)))]
pub async fn get_mcp_instance_discovery_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<McpInstanceDiscoveryPolicyResponse>>, ApiError> {
    let interface_core::McpCoreOutput::DiscoveryPolicy(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.discovery-policy.get.v1",
        interface_core::McpCoreInput::GetDiscoveryPolicy(instance_id),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(put, path = "/api/console/mcp/instances/{instance_id}/discovery-policy", request_body = UpdateMcpInstanceDiscoveryPolicyBody, responses((status = 200, body = McpInstanceDiscoveryPolicyResponse)))]
pub async fn update_mcp_instance_discovery_policy(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateMcpInstanceDiscoveryPolicyBody>,
) -> Result<Json<ApiSuccess<McpInstanceDiscoveryPolicyResponse>>, ApiError> {
    let interface_core::McpCoreOutput::DiscoveryPolicy(value) = invoke_core(
        state,
        headers,
        "http.console.mcp.discovery-policy.put.v1",
        interface_core::McpCoreInput::UpdateDiscoveryPolicy(instance_id, body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
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
        McpToolExecutionTargetDto::AssistantClient { .. } => {
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
        McpToolExecutionTargetDto::AssistantClient { capability_code } => {
            domain::McpToolExecutionTarget::AssistantClient {
                capability_code: capability_code.clone(),
            }
        }
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
        full_description: body.full_description.unwrap_or_default(),
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
        full_description: body.full_description.unwrap_or_default(),
        interface_entry,
        input_mapping: body.input_mapping,
        output_mapping: body.output_mapping,
        status: parse_tool_status(&body.status)?,
    })
}

#[cfg(test)]
mod tests;
