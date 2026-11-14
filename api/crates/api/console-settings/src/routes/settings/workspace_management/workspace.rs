use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
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

pub(crate) fn to_workspace_response(workspace: domain::WorkspaceRecord) -> WorkspaceResponse {
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.workspace.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::GetWorkspace,
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::Workspace(workspace) = output else {
        return Err(anyhow::anyhow!("workspace output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(workspace)))
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.workspace.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::PatchWorkspace(body),
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::Workspace(workspace) = output else {
        return Err(anyhow::anyhow!("workspace output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(workspace)))
}
