use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::workspace::WorkspaceService;
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
pub struct WorkspaceSummaryResponse {
    pub id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
    pub is_current: bool,
}

fn to_workspace_summary(
    workspace: domain::WorkspaceRecord,
    current_workspace_id: uuid::Uuid,
) -> WorkspaceSummaryResponse {
    WorkspaceSummaryResponse {
        id: workspace.id.to_string(),
        name: workspace.name,
        logo_url: workspace.logo_url,
        introduction: workspace.introduction,
        is_current: workspace.id == current_workspace_id,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new().route("/workspaces", console_get(list_workspaces, Authenticated))
}

#[utoipa::path(
    get,
    path = "/api/console/workspaces",
    responses((status = 200, body = [WorkspaceSummaryResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_workspaces(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<WorkspaceSummaryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspaces = WorkspaceService::new(state.store.clone())
        .list_accessible_workspaces(context.user.id)
        .await?;
    let current_workspace_id = context.actor.current_workspace_id;

    let mut current = Vec::new();
    let mut remaining = Vec::new();
    for workspace in workspaces {
        let response = to_workspace_summary(workspace, current_workspace_id);
        if response.is_current {
            current.push(response);
        } else {
            remaining.push(response);
        }
    }
    current.extend(remaining);

    Ok(Json(ApiSuccess::new(current)))
}
