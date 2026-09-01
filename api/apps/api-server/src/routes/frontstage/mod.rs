use std::sync::Arc;

pub mod block_tree;
pub mod callable_interfaces;
pub mod components;
pub mod data_capabilities;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::frontstage::{
    CreateFrontstageGroupCommand, CreateFrontstagePageCommand, CreateFrontstagePageTabCommand,
    DeleteFrontstagePageCommand, DeleteFrontstagePageTabCommand, FrontstagePageService,
    GetFrontstagePageDetailCommand, MoveFrontstagePageCommand, SaveFrontstageTabDocumentCommand,
    UpdateFrontstagePageMetadataCommand, UpdateFrontstagePageTabCommand,
};
use control_plane::resource_action::{
    ActionDefinition, ResourceActionKernel, ResourceActionRegistry, ResourceDefinition,
    ResourceScopeKind,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_get, console_patch, console_post, console_put, ConsoleRouteAssembly,
    },
};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontstagePageTreeNodeKind {
    Group,
    Page,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontstageNavigationPlacementResponse {
    Topbar,
    Sidebar,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontstagePageContentPresentationResponse {
    Single,
    Tabs,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstagePageTreeNodeResponse {
    pub id: String,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub is_hidden: bool,
    pub kind: FrontstagePageTreeNodeKind,
    pub placement: FrontstageNavigationPlacementResponse,
    pub content_presentation: FrontstagePageContentPresentationResponse,
    pub slug: Option<String>,
    #[serde(default)]
    #[schema(value_type = Vec<Object>)]
    pub children: Vec<FrontstagePageTreeNodeResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstagePageResponse {
    pub id: String,
    title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub is_hidden: bool,
    pub kind: FrontstagePageTreeNodeKind,
    pub parent_id: Option<String>,
    pub rank: String,
    pub placement: FrontstageNavigationPlacementResponse,
    pub content_presentation: FrontstagePageContentPresentationResponse,
    pub slug: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstagePageTabResponse {
    pub id: String,
    pub page_id: String,
    pub title: Option<String>,
    pub rank: String,
    pub is_default: bool,
    pub route_segment: Option<String>,
    pub document_root_uid: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstagePageCreationResponse {
    pub page: FrontstagePageResponse,
    pub default_tab: FrontstagePageTabResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageTabDocumentResponse {
    pub root_uid: String,
    pub payload: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstagePageDetailResponse {
    pub page: FrontstagePageResponse,
    pub tab: FrontstagePageTabResponse,
    pub document: FrontstageTabDocumentResponse,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFrontstageGroupBody {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub parent_id: Option<String>,
    pub rank: Option<String>,
    #[serde(default = "default_navigation_placement")]
    pub placement: FrontstageNavigationPlacementResponse,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFrontstagePageBody {
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub parent_id: Option<String>,
    pub rank: Option<String>,
    #[serde(default = "default_navigation_placement")]
    pub placement: FrontstageNavigationPlacementResponse,
    pub slug: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFrontstagePageMetadataBody {
    #[serde(
        default,
        deserialize_with = "crate::routes::helpers::deserialize_present_optional"
    )]
    pub title: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::routes::helpers::deserialize_present_optional"
    )]
    pub icon: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "crate::routes::helpers::deserialize_present_optional"
    )]
    pub tooltip: Option<Option<String>>,
    pub is_hidden: Option<bool>,
    pub placement: Option<FrontstageNavigationPlacementResponse>,
    pub content_presentation: Option<FrontstagePageContentPresentationResponse>,
    #[serde(
        default,
        deserialize_with = "crate::routes::helpers::deserialize_present_optional"
    )]
    pub slug: Option<Option<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveFrontstagePageBody {
    pub parent_id: Option<String>,
    pub rank: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFrontstagePageTabBody {
    pub title: Option<String>,
    pub route_segment: Option<String>,
    pub rank: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFrontstagePageTabBody {
    #[serde(
        default,
        deserialize_with = "crate::routes::helpers::deserialize_present_optional"
    )]
    pub title: Option<Option<String>>,
    pub rank: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveFrontstageTabDocumentBody {
    pub payload: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageUiTemplateResponse {
    pub template_id: Option<String>,
    pub provider_code: String,
    pub contribution_code: String,
    pub name: String,
    pub source: String,
    #[schema(value_type = String)]
    pub language: domain::UiCodeTemplateLanguage,
    pub version: String,
    pub is_official: bool,
    pub is_default: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DispatchFrontstageQueryBody {
    pub query_id: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DispatchFrontstageActionBody {
    pub action_id: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct FrontstageCapabilityInput {
    pub(crate) actor_user_id: Uuid,
    pub(crate) actor: domain::ActorContext,
    pub(crate) workspace_id: Uuid,
    pub(crate) page_id: Uuid,
    pub(crate) tab_id: Uuid,
    pub(crate) params: Value,
}

fn default_navigation_placement() -> FrontstageNavigationPlacementResponse {
    FrontstageNavigationPlacementResponse::Sidebar
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::{Authenticated, ConsoleOperation};

    block_tree::route_assembly()
        .route(
            "/frontstage/pages",
            console_get(list_frontstage_pages, Authenticated)
                .post(create_frontstage_page, Authenticated),
        )
        .route(
            "/frontstage/pages/groups",
            console_post(create_frontstage_group, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id",
            console_patch(update_frontstage_page_title, Authenticated)
                .delete(delete_frontstage_page, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id/move",
            console_post(move_frontstage_page, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id/tabs",
            console_get(list_frontstage_page_tabs, Authenticated)
                .post(create_frontstage_page_tab, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id/tabs/:tab_reference",
            console_get(get_frontstage_page_detail, Authenticated)
                .patch(update_frontstage_page_tab, Authenticated)
                .delete(delete_frontstage_page_tab, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id/tabs/:tab_id/document",
            console_put(save_frontstage_tab_document, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id/tabs/:tab_id/queries/dispatch",
            console_post(dispatch_frontstage_query, Authenticated),
        )
        .route(
            "/frontstage/pages/:page_id/tabs/:tab_id/actions/dispatch",
            console_post(dispatch_frontstage_action, Authenticated),
        )
        .route(
            "/frontstage/data-capabilities",
            console_get(
                data_capabilities::list_frontstage_data_capabilities,
                ConsoleOperation("frontstage.data_capabilities.view".to_string()),
            ),
        )
        .route(
            "/frontstage/interface-capabilities",
            console_get(
                callable_interfaces::list_frontstage_interface_capabilities,
                Authenticated,
            ),
        )
        .route(
            "/frontstage/interface-capabilities/:interface_id",
            console_get(
                callable_interfaces::get_frontstage_interface_capability,
                Authenticated,
            ),
        )
        .route(
            "/frontstage/components",
            console_get(components::list_frontstage_components, Authenticated),
        )
        .route(
            "/frontstage/ui-templates",
            console_get(list_frontstage_ui_templates, Authenticated),
        )
        .route(
            "/frontstage/components/:component_id",
            console_get(components::get_frontstage_component, Authenticated),
        )
        .route(
            "/frontstage/component-module-assets/:sha256",
            console_get(
                components::get_frontstage_component_module_asset,
                Authenticated,
            ),
        )
        .route(
            "/frontstage/pages/:page_id/tabs/:tab_id/callable-interfaces/dispatch",
            console_post(
                callable_interfaces::dispatch_frontstage_callable_interface,
                Authenticated,
            ),
        )
}

fn frontstage_query_kernel(state: Arc<ApiState>) -> Result<ResourceActionKernel, ApiError> {
    let mut registry = ResourceActionRegistry::default();
    registry.register_resource(ResourceDefinition::core(
        "frontstage_page_tab_query",
        ResourceScopeKind::Workspace,
    ))?;
    registry.register_action(ActionDefinition::core(
        "frontstage_page_tab_query",
        "frontstage.page_tab.get",
    ))?;
    data_capabilities::register_data_model_query_capabilities(
        &mut registry,
        "frontstage_page_tab_query",
    )?;

    let mut kernel = ResourceActionKernel::new(registry);
    data_capabilities::register_data_model_query_handlers(
        &mut kernel,
        "frontstage_page_tab_query",
        state.clone(),
    )?;
    kernel.register_json_handler(
        "frontstage_page_tab_query",
        "frontstage.page_tab.get",
        move |input| {
            let state = state.clone();
            async move {
                let input: FrontstageCapabilityInput =
                    serde_json::from_value(input).map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "frontstage_capability_input",
                        )
                    })?;
                if !input.params.is_null() && input.params != serde_json::json!({}) {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "frontstage_query_params",
                    )
                    .into());
                }
                let detail =
                    FrontstagePageService::for_actor(state.store.clone(), input.actor.clone())
                        .get_page_detail(GetFrontstagePageDetailCommand {
                            actor_user_id: input.actor_user_id,
                            workspace_id: input.workspace_id,
                            page_id: input.page_id,
                            tab_reference: input.tab_id.to_string(),
                        })
                        .await?;
                Ok(serde_json::to_value(to_page_detail_response(detail))?)
            }
        },
    )?;
    Ok(kernel)
}

fn frontstage_action_kernel(state: Arc<ApiState>) -> Result<ResourceActionKernel, ApiError> {
    let mut registry = ResourceActionRegistry::default();
    registry.register_resource(ResourceDefinition::core(
        "frontstage_page_tab_action",
        ResourceScopeKind::Workspace,
    ))?;
    registry.register_action(ActionDefinition::core(
        "frontstage_page_tab_action",
        "frontstage.page_tab.document.save",
    ))?;
    data_capabilities::register_data_model_action_capabilities(
        &mut registry,
        "frontstage_page_tab_action",
    )?;

    let mut kernel = ResourceActionKernel::new(registry);
    data_capabilities::register_data_model_action_handlers(
        &mut kernel,
        "frontstage_page_tab_action",
        state.clone(),
    )?;
    kernel.register_json_handler(
        "frontstage_page_tab_action",
        "frontstage.page_tab.document.save",
        move |input| {
            let state = state.clone();
            async move {
                let input: FrontstageCapabilityInput =
                    serde_json::from_value(input).map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "frontstage_capability_input",
                        )
                    })?;
                let body: SaveFrontstageTabDocumentBody = serde_json::from_value(input.params)
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "frontstage_action_params",
                        )
                    })?;
                let detail =
                    FrontstagePageService::for_actor(state.store.clone(), input.actor.clone())
                        .save_tab_document(SaveFrontstageTabDocumentCommand {
                            actor_user_id: input.actor_user_id,
                            workspace_id: input.workspace_id,
                            page_id: input.page_id,
                            tab_id: input.tab_id,
                            document_payload: body.payload,
                        })
                        .await?;
                Ok(serde_json::to_value(to_page_detail_response(detail))?)
            }
        },
    )?;
    Ok(kernel)
}

pub async fn dispatch_frontstage_query(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_id)): Path<(String, String)>,
    Json(body): Json<DispatchFrontstageQueryBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let output = frontstage_query_kernel(state)?
        .dispatch_json(
            "frontstage_page_tab_query",
            &body.query_id,
            serde_json::to_value(FrontstageCapabilityInput {
                actor_user_id: context.user.id,
                actor: context.actor.clone(),
                workspace_id: context.actor.current_workspace_id,
                page_id: parse_uuid(&page_id, "page_id")?,
                tab_id: parse_uuid(&tab_id, "tab_id")?,
                params: body.params,
            })?,
        )
        .await?;
    Ok(Json(ApiSuccess::new(output)))
}

pub async fn dispatch_frontstage_action(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_id)): Path<(String, String)>,
    Json(body): Json<DispatchFrontstageActionBody>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let output = frontstage_action_kernel(state)?
        .dispatch_json(
            "frontstage_page_tab_action",
            &body.action_id,
            serde_json::to_value(FrontstageCapabilityInput {
                actor_user_id: context.user.id,
                actor: context.actor.clone(),
                workspace_id: context.actor.current_workspace_id,
                page_id: parse_uuid(&page_id, "page_id")?,
                tab_id: parse_uuid(&tab_id, "tab_id")?,
                params: body.params,
            })?,
        )
        .await?;
    Ok(Json(ApiSuccess::new(output)))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/pages",
    responses(
        (status = 200, body = [FrontstagePageTreeNodeResponse]),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_pages(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FrontstagePageTreeNodeResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    let tree = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .list_page_tree(context.user.id, workspace_id)
        .await?;

    Ok(Json(ApiSuccess::new(
        tree.into_iter().map(to_tree_node_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/pages/groups",
    request_body = CreateFrontstageGroupBody,
    responses(
        (status = 201, body = FrontstagePageResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_frontstage_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateFrontstageGroupBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FrontstagePageResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let parent_id = parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?;

    let page = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .create_group(CreateFrontstageGroupCommand {
            actor_user_id: context.user.id,
            workspace_id,
            title: body.title,
            icon: body.icon,
            tooltip: body.tooltip,
            parent_id,
            rank: body.rank,
            placement: to_domain_placement(body.placement),
            slug: body.slug,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_page_response(page))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/pages",
    request_body = CreateFrontstagePageBody,
    responses(
        (status = 201, body = FrontstagePageCreationResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_frontstage_page(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateFrontstagePageBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FrontstagePageCreationResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let parent_id = parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?;

    let creation = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .create_page(CreateFrontstagePageCommand {
            actor_user_id: context.user.id,
            workspace_id,
            title: body.title,
            icon: body.icon,
            tooltip: body.tooltip,
            parent_id,
            rank: body.rank,
            placement: to_domain_placement(body.placement),
            slug: body.slug,
        })
        .await?;
    let mut default_tab =
        creation
            .default_tab
            .ok_or(control_plane::errors::ControlPlaneError::Conflict(
                "frontstage_page_requires_tab",
            ))?;
    project_default_tab_title(
        &state,
        &headers,
        context.user.preferred_locale,
        &mut default_tab,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(FrontstagePageCreationResponse {
            page: to_page_response(creation.page),
            default_tab: to_tab_response(default_tab),
        })),
    ))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/pages/{page_id}/tabs/{tab_reference}",
    params(
        ("page_id" = String, Path, description = "Page id")
        ,("tab_reference" = String, Path, description = "Tab route segment or legacy tab id")
    ),
    responses(
        (status = 200, body = FrontstagePageDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_page_detail(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_reference)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<FrontstagePageDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;

    let mut detail = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .get_page_detail(GetFrontstagePageDetailCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            tab_reference,
        })
        .await?;
    project_default_tab_title(
        &state,
        &headers,
        context.user.preferred_locale,
        &mut detail.tab,
    )
    .await?;

    Ok(Json(ApiSuccess::new(to_page_detail_response(detail))))
}

#[utoipa::path(
    patch,
    path = "/api/console/frontstage/pages/{page_id}",
    request_body = UpdateFrontstagePageMetadataBody,
    params(
        ("page_id" = String, Path, description = "Page or group id")
    ),
    responses(
        (status = 200, body = FrontstagePageResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_frontstage_page_title(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(page_id): Path<String>,
    Json(body): Json<UpdateFrontstagePageMetadataBody>,
) -> Result<Json<ApiSuccess<FrontstagePageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;

    let page = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .update_metadata(UpdateFrontstagePageMetadataCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            title: body.title,
            icon: body.icon,
            tooltip: body.tooltip,
            is_hidden: body.is_hidden,
            placement: body.placement.map(to_domain_placement),
            content_presentation: body
                .content_presentation
                .map(to_domain_content_presentation),
            slug: body.slug,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_page_response(page))))
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/pages/{page_id}/move",
    request_body = MoveFrontstagePageBody,
    params(
        ("page_id" = String, Path, description = "Page or group id")
    ),
    responses(
        (status = 200, body = FrontstagePageResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn move_frontstage_page(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(page_id): Path<String>,
    Json(body): Json<MoveFrontstagePageBody>,
) -> Result<Json<ApiSuccess<FrontstagePageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;
    let parent_id = parse_optional_uuid(body.parent_id.as_deref(), "parent_id")?;

    let page = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .move_page(MoveFrontstagePageCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            parent_id,
            rank: body.rank,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_page_response(page))))
}

#[utoipa::path(
    delete,
    path = "/api/console/frontstage/pages/{page_id}",
    params(
        ("page_id" = String, Path, description = "Page or group id")
    ),
    responses(
        (status = 204),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn delete_frontstage_page(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(page_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;

    FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .delete_page(DeleteFrontstagePageCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/frontstage/pages/{page_id}/tabs", responses((status = 200, body = Vec<FrontstagePageTabResponse>)))]
pub async fn list_frontstage_page_tabs(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(page_id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<FrontstagePageTabResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;
    let mut tabs = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .list_page_tabs(context.user.id, workspace_id, page_id)
        .await?;
    for tab in &mut tabs {
        project_default_tab_title(&state, &headers, context.user.preferred_locale.clone(), tab)
            .await?;
    }
    Ok(Json(ApiSuccess::new(
        tabs.into_iter().map(to_tab_response).collect(),
    )))
}

#[utoipa::path(post, path = "/api/console/frontstage/pages/{page_id}/tabs", request_body = CreateFrontstagePageTabBody, responses((status = 201, body = FrontstagePageTabResponse)))]
pub async fn create_frontstage_page_tab(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(page_id): Path<String>,
    Json(body): Json<CreateFrontstagePageTabBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FrontstagePageTabResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;
    let tab = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .create_page_tab(CreateFrontstagePageTabCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            title: body.title,
            route_segment: body.route_segment,
            rank: body.rank,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_tab_response(tab))),
    ))
}

#[utoipa::path(patch, path = "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}", request_body = UpdateFrontstagePageTabBody, responses((status = 200, body = FrontstagePageTabResponse)))]
pub async fn update_frontstage_page_tab(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_id)): Path<(String, String)>,
    Json(body): Json<UpdateFrontstagePageTabBody>,
) -> Result<Json<ApiSuccess<FrontstagePageTabResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let tab = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .update_page_tab(UpdateFrontstagePageTabCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            page_id: parse_uuid(&page_id, "page_id")?,
            tab_id: parse_uuid(&tab_id, "tab_id")?,
            title: body.title,
            rank: body.rank,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_tab_response(tab))))
}

#[utoipa::path(delete, path = "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}", responses((status = 204), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn delete_frontstage_page_tab(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .delete_page_tab(DeleteFrontstagePageTabCommand {
            actor_user_id: context.user.id,
            workspace_id: context.actor.current_workspace_id,
            page_id: parse_uuid(&page_id, "page_id")?,
            tab_id: parse_uuid(&tab_id, "tab_id")?,
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/document", request_body = SaveFrontstageTabDocumentBody, responses((status = 200, body = FrontstagePageDetailResponse)))]
pub async fn save_frontstage_tab_document(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((page_id, tab_id)): Path<(String, String)>,
    Json(body): Json<SaveFrontstageTabDocumentBody>,
) -> Result<Json<ApiSuccess<FrontstagePageDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;
    let page_id = parse_uuid(&page_id, "page_id")?;
    let tab_id = parse_uuid(&tab_id, "tab_id")?;

    let detail = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .save_tab_document(SaveFrontstageTabDocumentCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            tab_id,
            document_payload: body.payload,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_page_detail_response(detail))))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/ui-templates",
    responses((status = 200, body = [FrontstageUiTemplateResponse]), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn list_frontstage_ui_templates(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<FrontstageUiTemplateResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    if !context.actor.has_permission("frontstage.page.design") {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "frontstage.page.design",
        )
        .into());
    }
    let templates = control_plane::ui_management::UiManagementService::new(
        state.store.clone(),
        state.api_node_id.clone(),
    )
    .list_published_templates_for_workspace(workspace_id)
    .await?;
    Ok(Json(ApiSuccess::new(
        templates
            .into_iter()
            .map(|value| FrontstageUiTemplateResponse {
                template_id: value.template_id.map(|id| id.to_string()),
                provider_code: value.provider_code,
                contribution_code: value.contribution_code,
                name: value.name,
                source: value.source,
                language: value.language,
                version: value.version,
                is_official: value.is_official,
                is_default: value.is_default,
            })
            .collect(),
    )))
}

pub(crate) fn parse_uuid(raw: &str, field: &'static str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(raw)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn parse_optional_uuid(raw: Option<&str>, field: &'static str) -> Result<Option<Uuid>, ApiError> {
    raw.map(|value| parse_uuid(value, field)).transpose()
}

fn to_kind_response(kind: domain::FrontstagePageKind) -> FrontstagePageTreeNodeKind {
    match kind {
        domain::FrontstagePageKind::Group => FrontstagePageTreeNodeKind::Group,
        domain::FrontstagePageKind::Page => FrontstagePageTreeNodeKind::Page,
    }
}

fn to_domain_placement(
    placement: FrontstageNavigationPlacementResponse,
) -> domain::frontstage::FrontstageNavigationPlacement {
    match placement {
        FrontstageNavigationPlacementResponse::Topbar => {
            domain::frontstage::FrontstageNavigationPlacement::Topbar
        }
        FrontstageNavigationPlacementResponse::Sidebar => {
            domain::frontstage::FrontstageNavigationPlacement::Sidebar
        }
    }
}

fn to_placement_response(
    placement: domain::frontstage::FrontstageNavigationPlacement,
) -> FrontstageNavigationPlacementResponse {
    match placement {
        domain::frontstage::FrontstageNavigationPlacement::Topbar => {
            FrontstageNavigationPlacementResponse::Topbar
        }
        domain::frontstage::FrontstageNavigationPlacement::Sidebar => {
            FrontstageNavigationPlacementResponse::Sidebar
        }
    }
}

fn to_domain_content_presentation(
    content_presentation: FrontstagePageContentPresentationResponse,
) -> domain::frontstage::FrontstagePageContentPresentation {
    match content_presentation {
        FrontstagePageContentPresentationResponse::Single => {
            domain::frontstage::FrontstagePageContentPresentation::Single
        }
        FrontstagePageContentPresentationResponse::Tabs => {
            domain::frontstage::FrontstagePageContentPresentation::Tabs
        }
    }
}

fn to_content_presentation_response(
    content_presentation: domain::frontstage::FrontstagePageContentPresentation,
) -> FrontstagePageContentPresentationResponse {
    match content_presentation {
        domain::frontstage::FrontstagePageContentPresentation::Single => {
            FrontstagePageContentPresentationResponse::Single
        }
        domain::frontstage::FrontstagePageContentPresentation::Tabs => {
            FrontstagePageContentPresentationResponse::Tabs
        }
    }
}

fn to_page_response(page: domain::FrontstagePageRecord) -> FrontstagePageResponse {
    FrontstagePageResponse {
        id: page.id.to_string(),
        title: page.title,
        icon: page.icon,
        tooltip: page.tooltip,
        is_hidden: page.is_hidden,
        kind: to_kind_response(page.kind),
        parent_id: page.parent_id.map(|id| id.to_string()),
        rank: page.rank,
        placement: to_placement_response(page.placement),
        content_presentation: to_content_presentation_response(page.content_presentation),
        slug: page.slug,
    }
}

async fn project_default_tab_title(
    state: &ApiState,
    headers: &HeaderMap,
    preferred_locale: Option<String>,
    tab: &mut domain::frontstage::FrontstagePageTabRecord,
) -> Result<(), ApiError> {
    if !tab.is_default {
        return Ok(());
    }
    let locale = crate::app_state::request_catalog_locale(headers, preferred_locale);
    let stored = tab.title.as_deref().unwrap_or_default();
    tab.title =
        Some(crate::app_state::project_canonical_display(state, &locale, "Default", stored).await?);
    Ok(())
}

fn to_tab_response(tab: domain::frontstage::FrontstagePageTabRecord) -> FrontstagePageTabResponse {
    FrontstagePageTabResponse {
        id: tab.id.to_string(),
        page_id: tab.page_id.to_string(),
        title: tab.title,
        rank: tab.rank,
        is_default: tab.is_default,
        route_segment: tab.route_segment,
        document_root_uid: tab.document_root_uid,
    }
}

fn to_page_detail_response(
    detail: domain::frontstage::FrontstagePageDetail,
) -> FrontstagePageDetailResponse {
    FrontstagePageDetailResponse {
        page: to_page_response(detail.page),
        tab: to_tab_response(detail.tab),
        document: FrontstageTabDocumentResponse {
            root_uid: detail.document.root_uid,
            payload: detail.document.payload,
        },
    }
}

fn to_tree_node_response(node: domain::FrontstagePageTreeNode) -> FrontstagePageTreeNodeResponse {
    FrontstagePageTreeNodeResponse {
        id: node.page.id.to_string(),
        title: node.page.title,
        icon: node.page.icon,
        tooltip: node.page.tooltip,
        is_hidden: node.page.is_hidden,
        kind: to_kind_response(node.page.kind),
        placement: to_placement_response(node.page.placement),
        content_presentation: to_content_presentation_response(node.page.content_presentation),
        slug: node.page.slug,
        children: node
            .children
            .into_iter()
            .map(to_tree_node_response)
            .collect(),
    }
}
