use std::{collections::BTreeMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::{
    frontstage::{
        CreateFrontstageBlockNodeCommand, DeleteFrontstageBlockSubtreeCommand,
        FrontstageBlockScopeCommand, FrontstagePageService, ListFrontstageBlockChildrenCommand,
        ListFrontstageBlockDescendantsCommand, ListFrontstageBlocksCommand,
        MoveFrontstageBlockNodeCommand, SaveFrontstageBlockNodeCodeCommand,
        SearchFrontstageBlocksCommand, UpdateFrontstageBlockNodeCommand,
    },
    ports::FrontstageBlockPosition,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

use super::parse_uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrontstageBlockPresentationDto {
    Page,
    Drawer,
    Modal,
    Inline,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFrontstageBlockNodeBody {
    pub tab_id: String,
    pub title: String,
    pub presentation: FrontstageBlockPresentationDto,
    pub parent_block_id: Option<String>,
    pub before_block_id: Option<String>,
    pub after_block_id: Option<String>,
    pub code: String,
    #[serde(default)]
    pub input_mapping: BTreeMap<String, String>,
    #[serde(default)]
    pub output_mapping: BTreeMap<String, String>,
    pub runtime_descriptor: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateFrontstageBlockNodeBody {
    pub title: Option<String>,
    pub presentation: Option<FrontstageBlockPresentationDto>,
    pub input_mapping: Option<BTreeMap<String, String>>,
    pub output_mapping: Option<BTreeMap<String, String>>,
    pub runtime_descriptor: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveFrontstageBlockNodeBody {
    pub parent_block_id: Option<String>,
    pub before_block_id: Option<String>,
    pub after_block_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteFrontstageBlockSubtreeBody {
    pub expected_affected_count: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveFrontstageBlockNodeCodeBody {
    pub code: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageBlockListQuery {
    #[serde(default = "default_result_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageBlockSearchQuery {
    pub query: String,
    #[serde(default = "default_result_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageBlockDescendantsQuery {
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_result_limit")]
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FrontstageBlockNodeSummaryResponse {
    pub block_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub tab_id: String,
    pub parent_block_id: Option<String>,
    pub rank: String,
    pub presentation: FrontstageBlockPresentationDto,
    pub title: Option<String>,
    pub schema_version: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockNodeResponse {
    pub block_id: String,
    pub workspace_id: String,
    pub page_id: String,
    pub tab_id: String,
    pub parent_block_id: Option<String>,
    pub rank: String,
    pub presentation: FrontstageBlockPresentationDto,
    pub title: Option<String>,
    pub schema_version: u32,
    pub input_mapping: BTreeMap<String, String>,
    pub output_mapping: BTreeMap<String, String>,
    #[schema(value_type = Object)]
    pub runtime_descriptor: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockDescendantResponse {
    pub node: FrontstageBlockNodeSummaryResponse,
    pub depth: u32,
    pub has_children: bool,
    pub path: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockSearchResultResponse {
    pub node: FrontstageBlockNodeSummaryResponse,
    pub ancestors: Vec<FrontstageBlockNodeSummaryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockDeleteImpactResponse {
    pub affected_count: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockSubtreeDeleteResponse {
    pub deleted_count: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockNodeCodeResponse {
    pub block_id: String,
    pub page_id: String,
    pub code: String,
    pub source_sha256: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageBlockOpenResponse {
    pub canonical_url: String,
}

pub(super) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/search",
            console_get(search_frontstage_blocks, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks",
            console_get(list_frontstage_block_roots, Authenticated)
                .post(create_frontstage_block_node, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id",
            console_get(get_frontstage_block_node, Authenticated)
                .patch(update_frontstage_block_node, Authenticated)
                .delete(delete_frontstage_block_leaf, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/children",
            console_get(list_frontstage_block_children, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/ancestors",
            console_get(list_frontstage_block_ancestors, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/descendants",
            console_get(list_frontstage_block_descendants, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/delete-impact",
            console_get(get_frontstage_block_delete_impact, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/move",
            console_post(move_frontstage_block_node, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/delete-subtree",
            console_post(delete_frontstage_block_subtree, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/open",
            console_get(open_frontstage_block, Authenticated),
        )
        .route(
            "/frontstage/:workspace_id/pages/:page_id/blocks/:block_id/code",
            console_get(get_frontstage_block_node_code, Authenticated)
                .put(save_frontstage_block_node_code, Authenticated),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/open",
    summary = "Open a Frontstage block",
    description = "Resolves a visible block to its canonical Frontstage URL. The backend restores page ancestry and presentation; callers only provide the public block id.",
    responses(
        (status = 200, body = FrontstageBlockOpenResponse),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn open_frontstage_block(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<FrontstageBlockOpenResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let target = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .open_block(FrontstageBlockScopeCommand {
            actor_user_id: context.user.id,
            workspace_id: parse_uuid(&workspace_id, "workspace_id")?,
            page_id: parse_uuid(&page_id, "page_id")?,
            block_id,
        })
        .await?;
    Ok(Json(ApiSuccess::new(FrontstageBlockOpenResponse {
        canonical_url: format!(
            "/{}/pages/{}/blocks/{}",
            target.slug,
            target.page_id,
            encode_block_path_segment(&target.block_id)
        ),
    })))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks", summary = "List Frontstage block roots", description = "Lists public Block Node Descriptor v1 roots in the page ordered-tree partition.", params(("workspace_id" = String, Path), ("page_id" = String, Path), FrontstageBlockListQuery), responses((status = 200, body = Vec<FrontstageBlockNodeSummaryResponse>), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody)))]
pub async fn list_frontstage_block_roots(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id)): Path<(String, String)>,
    Query(query): Query<FrontstageBlockListQuery>,
) -> Result<Json<ApiSuccess<Vec<FrontstageBlockNodeSummaryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let nodes = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .list_block_roots(ListFrontstageBlocksCommand {
            actor_user_id: context.user.id,
            workspace_id: parse_uuid(&workspace_id, "workspace_id")?,
            page_id: parse_uuid(&page_id, "page_id")?,
            limit: query.limit,
        })
        .await?;
    Ok(Json(ApiSuccess::new(
        nodes.into_iter().map(to_summary_response).collect(),
    )))
}

#[utoipa::path(post, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks", summary = "Create a Frontstage block", description = "Creates one independently stored Block Node Descriptor v1 in the page ordered-tree partition.", request_body = CreateFrontstageBlockNodeBody, responses((status = 201, body = FrontstageBlockNodeResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn create_frontstage_block_node(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id)): Path<(String, String)>,
    Json(body): Json<CreateFrontstageBlockNodeBody>,
) -> Result<(StatusCode, Json<ApiSuccess<FrontstageBlockNodeResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let node = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .create_block_node(CreateFrontstageBlockNodeCommand {
            actor_user_id: context.user.id,
            workspace_id: parse_uuid(&workspace_id, "workspace_id")?,
            page_id: parse_uuid(&page_id, "page_id")?,
            tab_id: parse_uuid(&body.tab_id, "tab_id")?,
            title: body.title,
            presentation: to_domain_presentation(body.presentation),
            position: FrontstageBlockPosition {
                parent_block_id: body.parent_block_id,
                before_block_id: body.before_block_id,
                after_block_id: body.after_block_id,
            },
            code: body.code,
            input_mapping: body.input_mapping,
            output_mapping: body.output_mapping,
            runtime_descriptor: body.runtime_descriptor,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_node_response(node))),
    ))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/search", params(("workspace_id" = String, Path), ("page_id" = String, Path), FrontstageBlockSearchQuery), responses((status = 200, body = Vec<FrontstageBlockSearchResultResponse>), (status = 400, body = crate::error_response::ErrorBody)))]
pub async fn search_frontstage_blocks(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id)): Path<(String, String)>,
    Query(query): Query<FrontstageBlockSearchQuery>,
) -> Result<Json<ApiSuccess<Vec<FrontstageBlockSearchResultResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let results = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .search_blocks(SearchFrontstageBlocksCommand {
            actor_user_id: context.user.id,
            workspace_id: parse_uuid(&workspace_id, "workspace_id")?,
            page_id: parse_uuid(&page_id, "page_id")?,
            query: query.query,
            limit: query.limit,
        })
        .await?;
    Ok(Json(ApiSuccess::new(
        results
            .into_iter()
            .map(|result| FrontstageBlockSearchResultResponse {
                node: to_summary_response(result.node),
                ancestors: result
                    .ancestors
                    .into_iter()
                    .map(to_summary_response)
                    .collect(),
            })
            .collect(),
    )))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}", summary = "Get a Frontstage block", description = "Gets one public Block Node Descriptor v1 without exposing internal ordered-tree identity or model codes.", responses((status = 200, body = FrontstageBlockNodeResponse), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn get_frontstage_block_node(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<FrontstageBlockNodeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let node = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .get_block_node(block_scope(&context, workspace_id, page_id, block_id)?)
        .await?;
    Ok(Json(ApiSuccess::new(to_node_response(node))))
}

#[utoipa::path(patch, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}", request_body = UpdateFrontstageBlockNodeBody, responses((status = 200, body = FrontstageBlockNodeResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn update_frontstage_block_node(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
    Json(body): Json<UpdateFrontstageBlockNodeBody>,
) -> Result<Json<ApiSuccess<FrontstageBlockNodeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let node = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .update_block_node(UpdateFrontstageBlockNodeCommand {
            scope: block_scope(&context, workspace_id, page_id, block_id)?,
            title: body.title,
            presentation: body.presentation.map(to_domain_presentation),
            input_mapping: body.input_mapping,
            output_mapping: body.output_mapping,
            runtime_descriptor: body.runtime_descriptor,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_node_response(node))))
}

#[utoipa::path(delete, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}", summary = "Delete a Frontstage block leaf", description = "Deletes one leaf block; nodes with children require the explicit subtree action.", responses((status = 204), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn delete_frontstage_block_leaf(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .delete_block_leaf(block_scope(&context, workspace_id, page_id, block_id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/children", params(FrontstageBlockListQuery), responses((status = 200, body = Vec<FrontstageBlockNodeSummaryResponse>)))]
pub async fn list_frontstage_block_children(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
    Query(query): Query<FrontstageBlockListQuery>,
) -> Result<Json<ApiSuccess<Vec<FrontstageBlockNodeSummaryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let nodes = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .list_block_children(ListFrontstageBlockChildrenCommand {
            scope: block_scope(&context, workspace_id, page_id, block_id)?,
            limit: query.limit,
        })
        .await?;
    Ok(Json(ApiSuccess::new(
        nodes.into_iter().map(to_summary_response).collect(),
    )))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/ancestors", responses((status = 200, body = Vec<FrontstageBlockNodeSummaryResponse>), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn list_frontstage_block_ancestors(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<Vec<FrontstageBlockNodeSummaryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let nodes = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .list_block_ancestors(block_scope(&context, workspace_id, page_id, block_id)?)
        .await?;
    Ok(Json(ApiSuccess::new(
        nodes.into_iter().map(to_summary_response).collect(),
    )))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/descendants", params(FrontstageBlockDescendantsQuery), responses((status = 200, body = Vec<FrontstageBlockDescendantResponse>), (status = 400, body = crate::error_response::ErrorBody)))]
pub async fn list_frontstage_block_descendants(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
    Query(query): Query<FrontstageBlockDescendantsQuery>,
) -> Result<Json<ApiSuccess<Vec<FrontstageBlockDescendantResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let nodes = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .list_block_descendants(ListFrontstageBlockDescendantsCommand {
            scope: block_scope(&context, workspace_id, page_id, block_id)?,
            max_depth: query.max_depth,
            limit: query.limit,
        })
        .await?;
    Ok(Json(ApiSuccess::new(
        nodes
            .into_iter()
            .map(|projection| FrontstageBlockDescendantResponse {
                node: to_summary_response(projection.node),
                depth: projection.depth,
                has_children: projection.has_children,
                path: projection.path,
            })
            .collect(),
    )))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/delete-impact", responses((status = 200, body = FrontstageBlockDeleteImpactResponse), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn get_frontstage_block_delete_impact(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<FrontstageBlockDeleteImpactResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let impact = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .get_block_delete_impact(block_scope(&context, workspace_id, page_id, block_id)?)
        .await?;
    Ok(Json(ApiSuccess::new(FrontstageBlockDeleteImpactResponse {
        affected_count: impact.affected_count,
    })))
}

#[utoipa::path(post, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/move", summary = "Move a Frontstage block", description = "Moves one block within its page ordered-tree partition using public block positions.", request_body = MoveFrontstageBlockNodeBody, responses((status = 200, body = FrontstageBlockNodeResponse), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn move_frontstage_block_node(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
    Json(body): Json<MoveFrontstageBlockNodeBody>,
) -> Result<Json<ApiSuccess<FrontstageBlockNodeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let node = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .move_block_node(MoveFrontstageBlockNodeCommand {
            scope: block_scope(&context, workspace_id, page_id, block_id)?,
            position: FrontstageBlockPosition {
                parent_block_id: body.parent_block_id,
                before_block_id: body.before_block_id,
                after_block_id: body.after_block_id,
            },
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_node_response(node))))
}

#[utoipa::path(post, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/delete-subtree", summary = "Delete a Frontstage block subtree", description = "Explicitly deletes a block subtree after the caller confirms the backend impact count.", request_body = DeleteFrontstageBlockSubtreeBody, responses((status = 200, body = FrontstageBlockSubtreeDeleteResponse), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn delete_frontstage_block_subtree(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
    Json(body): Json<DeleteFrontstageBlockSubtreeBody>,
) -> Result<Json<ApiSuccess<FrontstageBlockSubtreeDeleteResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let deleted = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .delete_block_subtree(DeleteFrontstageBlockSubtreeCommand {
            scope: block_scope(&context, workspace_id, page_id, block_id)?,
            expected_affected_count: body.expected_affected_count,
        })
        .await?;
    Ok(Json(ApiSuccess::new(
        FrontstageBlockSubtreeDeleteResponse {
            deleted_count: deleted.deleted_count,
        },
    )))
}

#[utoipa::path(get, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/code", responses((status = 200, body = FrontstageBlockNodeCodeResponse), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn get_frontstage_block_node_code(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
) -> Result<Json<ApiSuccess<FrontstageBlockNodeCodeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let scope = block_scope(&context, workspace_id, page_id, block_id)?;
    let public_block_id = scope.block_id.clone();
    let code = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .get_block_node_code(scope)
        .await?;
    Ok(Json(ApiSuccess::new(to_code_response(
        public_block_id,
        code,
    ))))
}

#[utoipa::path(put, path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/blocks/{block_id}/code", request_body = SaveFrontstageBlockNodeCodeBody, responses((status = 200, body = FrontstageBlockNodeCodeResponse), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn save_frontstage_block_node_code(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, block_id)): Path<(String, String, String)>,
    Json(body): Json<SaveFrontstageBlockNodeCodeBody>,
) -> Result<Json<ApiSuccess<FrontstageBlockNodeCodeResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let scope = block_scope(&context, workspace_id, page_id, block_id)?;
    let public_block_id = scope.block_id.clone();
    let code = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .save_block_node_code(SaveFrontstageBlockNodeCodeCommand {
            scope,
            code: body.code,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_code_response(
        public_block_id,
        code,
    ))))
}

fn block_scope(
    context: &crate::middleware::require_session::RequestContext,
    workspace_id: String,
    page_id: String,
    block_id: String,
) -> Result<FrontstageBlockScopeCommand, ApiError> {
    Ok(FrontstageBlockScopeCommand {
        actor_user_id: context.user.id,
        workspace_id: parse_uuid(&workspace_id, "workspace_id")?,
        page_id: parse_uuid(&page_id, "page_id")?,
        block_id,
    })
}

fn default_result_limit() -> u32 {
    100
}

fn default_max_depth() -> u32 {
    64
}

fn encode_block_path_segment(block_id: &str) -> String {
    block_id
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                vec![byte as char]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect()
}

fn to_domain_presentation(
    presentation: FrontstageBlockPresentationDto,
) -> domain::FrontstageBlockPresentation {
    match presentation {
        FrontstageBlockPresentationDto::Page => domain::FrontstageBlockPresentation::Page,
        FrontstageBlockPresentationDto::Drawer => domain::FrontstageBlockPresentation::Drawer,
        FrontstageBlockPresentationDto::Modal => domain::FrontstageBlockPresentation::Modal,
        FrontstageBlockPresentationDto::Inline => domain::FrontstageBlockPresentation::Inline,
    }
}

fn to_presentation_response(
    presentation: domain::FrontstageBlockPresentation,
) -> FrontstageBlockPresentationDto {
    match presentation {
        domain::FrontstageBlockPresentation::Page => FrontstageBlockPresentationDto::Page,
        domain::FrontstageBlockPresentation::Drawer => FrontstageBlockPresentationDto::Drawer,
        domain::FrontstageBlockPresentation::Modal => FrontstageBlockPresentationDto::Modal,
        domain::FrontstageBlockPresentation::Inline => FrontstageBlockPresentationDto::Inline,
    }
}

fn to_summary_response(
    node: domain::FrontstageBlockNodeSummary,
) -> FrontstageBlockNodeSummaryResponse {
    FrontstageBlockNodeSummaryResponse {
        block_id: node.block_id,
        workspace_id: node.workspace_id.to_string(),
        page_id: node.page_id.to_string(),
        tab_id: node.tab_id.to_string(),
        parent_block_id: node.parent_block_id,
        rank: node.rank,
        presentation: to_presentation_response(node.presentation),
        title: node.title,
        schema_version: node.schema_version,
        created_at: format_time(node.created_at),
        updated_at: format_time(node.updated_at),
    }
}

fn to_node_response(node: domain::FrontstageBlockNodeRecord) -> FrontstageBlockNodeResponse {
    FrontstageBlockNodeResponse {
        block_id: node.block_id,
        workspace_id: node.workspace_id.to_string(),
        page_id: node.page_id.to_string(),
        tab_id: node.tab_id.to_string(),
        parent_block_id: node.parent_block_id,
        rank: node.rank,
        presentation: to_presentation_response(node.presentation),
        title: node.title,
        schema_version: node.schema_version,
        input_mapping: node.input_mapping,
        output_mapping: node.output_mapping,
        runtime_descriptor: node.runtime_descriptor,
        created_at: format_time(node.created_at),
        updated_at: format_time(node.updated_at),
    }
}

fn to_code_response(
    block_id: String,
    code: domain::frontstage::FrontstageBlockCodeRecord,
) -> FrontstageBlockNodeCodeResponse {
    let source_sha256 = format!("{:x}", Sha256::digest(code.code.as_bytes()));
    FrontstageBlockNodeCodeResponse {
        block_id,
        page_id: code.page_id.to_string(),
        code: code.code,
        source_sha256,
    }
}

fn format_time(value: time::OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("stored frontstage block timestamps must format as RFC3339")
}
