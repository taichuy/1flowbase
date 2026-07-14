use std::sync::Arc;

use axum::{Json, Router, extract::State, http::HeaderMap};
use control_plane::workspace::{UpdateWorkspaceCommand, WorkspaceService};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{ConsoleRouteAssembly, console_get},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchWorkspaceBody {
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkspaceResponse {
    pub id: String,
    pub name: String,
    pub logo_url: Option<String>,
    pub introduction: String,
}

fn to_workspace_response(workspace: domain::WorkspaceRecord) -> WorkspaceResponse {
    WorkspaceResponse {
        id: workspace.id.to_string(),
        name: workspace.name,
        logo_url: workspace.logo_url,
        introduction: workspace.introduction,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::{Authenticated, ConsoleOperation};

    ConsoleRouteAssembly::new().route(
        "/workspace",
        console_get(get_workspace, Authenticated).patch(
            patch_workspace,
            ConsoleOperation("workspace.update".to_string()),
        ),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/workspace",
    responses((status = 200, body = WorkspaceResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn get_workspace(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<WorkspaceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace = WorkspaceService::new(state.store.clone())
        .get_workspace(context.actor.current_workspace_id)
        .await?;

    Ok(Json(ApiSuccess::new(to_workspace_response(workspace))))
}

#[utoipa::path(
    patch,
    path = "/api/console/workspace",
    request_body = PatchWorkspaceBody,
    responses((status = 200, body = WorkspaceResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn patch_workspace(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<PatchWorkspaceBody>,
) -> Result<Json<ApiSuccess<WorkspaceResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = context.actor.current_workspace_id;

    let workspace = WorkspaceService::new(state.store.clone())
        .update_workspace(UpdateWorkspaceCommand {
            actor: context.actor,
            workspace_id,
            name: body.name,
            logo_url: body.logo_url,
            introduction: body.introduction,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_workspace_response(workspace))))
}
