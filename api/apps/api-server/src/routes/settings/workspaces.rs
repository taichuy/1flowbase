use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
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

pub(crate) fn to_workspace_summaries(
    records: Vec<domain::WorkspaceRecord>,
    current_workspace_id: uuid::Uuid,
) -> Vec<WorkspaceSummaryResponse> {
    let (mut current, remaining): (Vec<_>, Vec<_>) = records
        .into_iter()
        .map(|workspace| to_workspace_summary(workspace, current_workspace_id))
        .partition(|workspace| workspace.is_current);
    current.extend(remaining);
    current
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
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.workspaces.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: Arc::clone(&state),
            headers,
        },
        crate::routes::membership_interface::MembershipInput::ListWorkspaces,
    )
    .await?;
    let crate::routes::membership_interface::MembershipOutput::Workspaces(workspaces) = output
    else {
        return Err(anyhow::anyhow!("workspaces output contract mismatch").into());
    };
    Ok(Json(ApiSuccess::new(workspaces)))
}
