use std::sync::Arc;

use access_control::{
    ConsoleNavigation, ConsoleNavigationItem, ConsolePermissionBinding, ConsoleRouteDefinition,
};
use axum::{extract::State, http::HeaderMap, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsoleNavigationResponse {
    pub route_definitions: Vec<ConsoleRouteDefinitionResponse>,
    pub navigation_items: Vec<ConsoleNavigationItemResponse>,
    pub permission_bindings: Vec<ConsolePermissionBindingResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsoleRouteDefinitionResponse {
    pub route_id: String,
    pub surface_key: String,
    pub path: String,
    pub surface_kind: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsoleNavigationItemResponse {
    pub item_id: String,
    pub route_id: String,
    pub parent_item_id: Option<String>,
    pub label_key: String,
    pub navigation_slot: String,
    pub order: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConsolePermissionBindingResponse {
    pub binding_id: String,
    pub route_id: String,
    pub permission_codes: Vec<String>,
    pub requirement: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new().route(
        "/navigation",
        console_get(
            get_console_navigation,
            access_control::ConsoleRouteOwnership::Authenticated,
        ),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/navigation",
    responses(
        (status = 200, body = ConsoleNavigationResponse),
        (status = 401, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_console_navigation(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ConsoleNavigationResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let navigation = state
        .console_surface_registry
        .accessible_navigation(&context.actor);
    Ok(Json(ApiSuccess::new(ConsoleNavigationResponse::from(
        navigation,
    ))))
}

impl From<ConsoleNavigation> for ConsoleNavigationResponse {
    fn from(navigation: ConsoleNavigation) -> Self {
        Self {
            route_definitions: navigation
                .route_definitions
                .into_iter()
                .map(ConsoleRouteDefinitionResponse::from)
                .collect(),
            navigation_items: navigation
                .navigation_items
                .into_iter()
                .map(ConsoleNavigationItemResponse::from)
                .collect(),
            permission_bindings: navigation
                .permission_bindings
                .into_iter()
                .map(ConsolePermissionBindingResponse::from)
                .collect(),
        }
    }
}

impl From<ConsoleRouteDefinition> for ConsoleRouteDefinitionResponse {
    fn from(route: ConsoleRouteDefinition) -> Self {
        Self {
            route_id: route.route_id,
            surface_key: route.surface_key,
            path: route.path,
            surface_kind: route.surface_kind.as_str().to_string(),
        }
    }
}

impl From<ConsoleNavigationItem> for ConsoleNavigationItemResponse {
    fn from(item: ConsoleNavigationItem) -> Self {
        Self {
            item_id: item.item_id,
            route_id: item.route_id,
            parent_item_id: item.parent_item_id,
            label_key: item.label_key,
            navigation_slot: item.navigation_slot.as_str().to_string(),
            order: item.order,
        }
    }
}

impl From<ConsolePermissionBinding> for ConsolePermissionBindingResponse {
    fn from(binding: ConsolePermissionBinding) -> Self {
        Self {
            binding_id: binding.binding_id,
            route_id: binding.route_id,
            permission_codes: binding.permission_codes,
            requirement: binding.requirement.as_str().to_string(),
        }
    }
}
