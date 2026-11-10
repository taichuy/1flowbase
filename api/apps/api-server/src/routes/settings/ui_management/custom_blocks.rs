use super::custom_blocks_interface::{Input, Output};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_put, ConsoleRouteAssembly},
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateBlockBody {
    pub provider_code: String,
    pub contribution_code: String,
    pub title: String,
    pub source: String,
    pub language: String,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateBlockBody {
    pub title: String,
    pub source: String,
    pub language: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct CustomBlockResponse {
    pub id: String,
    pub workspace_id: String,
    pub provider_code: String,
    pub contribution_code: String,
    pub title: String,
    pub source: String,
    pub language: String,
    pub revision: i32,
}
impl From<control_plane_contracts::ports::custom_frontend_blocks::CustomFrontendBlock>
    for CustomBlockResponse
{
    fn from(
        value: control_plane_contracts::ports::custom_frontend_blocks::CustomFrontendBlock,
    ) -> Self {
        Self {
            id: value.id.to_string(),
            workspace_id: value.workspace_id.to_string(),
            provider_code: value.provider_code,
            contribution_code: value.contribution_code,
            title: value.title,
            source: value.source,
            language: value.language,
            revision: value.revision,
        }
    }
}
async fn invoke(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding: &'static str,
    input: Input,
    mutating: bool,
) -> Result<Output, ApiError> {
    let snapshot = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot, binding, credential, input).await
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    ConsoleRouteAssembly::new()
        .route(
            "/settings/ui-management/blocks",
            console_get(
                list_blocks,
                ConsoleOperation("ui_management.blocks.list".into()),
            )
            .post(
                create_block,
                ConsoleOperation("ui_management.blocks.create".into()),
            ),
        )
        .route(
            "/settings/ui-management/blocks/:id",
            console_put(
                update_block,
                ConsoleOperation("ui_management.blocks.update".into()),
            )
            .delete(
                delete_block,
                ConsoleOperation("ui_management.blocks.delete".into()),
            ),
        )
}

/// List custom block definitions in the current workspace.
#[utoipa::path(get, summary="List custom blocks", description="Return the custom block definitions owned by the authenticated workspace.", path="/api/console/settings/ui-management/blocks", responses((status=200, body=[CustomBlockResponse])))]
pub async fn list_blocks(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<CustomBlockResponse>>>, ApiError> {
    let Output::List(value) = invoke(
        state,
        headers,
        "http.console.ui-management.blocks.list.get.v1",
        Input::List,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}
/// Create an independent custom block from an authorized base contribution.
#[utoipa::path(post, summary="Create a custom block", description="Copy an authorized native contribution into an independent workspace-owned block definition.", path="/api/console/settings/ui-management/blocks", request_body=CreateBlockBody, responses((status=201, body=CustomBlockResponse)))]
pub async fn create_block(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateBlockBody>,
) -> Result<(StatusCode, Json<ApiSuccess<CustomBlockResponse>>), ApiError> {
    let Output::Record(value) = invoke(
        state,
        headers,
        "http.console.ui-management.blocks.create.post.v1",
        Input::Create(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}
/// Update a workspace-owned custom block without changing its native base.
#[utoipa::path(put, summary="Update a custom block", description="Update the name and source of a custom definition in the authenticated workspace. Native registrations cannot be modified.", path="/api/console/settings/ui-management/blocks/{id}", params(("id"=String, Path)), request_body=UpdateBlockBody, responses((status=200, body=CustomBlockResponse)))]
pub async fn update_block(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateBlockBody>,
) -> Result<Json<ApiSuccess<CustomBlockResponse>>, ApiError> {
    let Output::Record(value) = invoke(
        state,
        headers,
        "http.console.ui-management.blocks.update.put.v1",
        Input::Update { id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}
/// Delete a workspace-owned custom definition; existing page instances retain their source.
#[utoipa::path(delete, summary="Delete a custom block", description="Delete a custom definition from the authenticated workspace without changing existing page instances or native registrations.", path="/api/console/settings/ui-management/blocks/{id}", params(("id"=String, Path)), responses((status=204)))]
pub async fn delete_block(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let Output::Empty = invoke(
        state,
        headers,
        "http.console.ui-management.blocks.delete.delete.v1",
        Input::Delete { id },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}
