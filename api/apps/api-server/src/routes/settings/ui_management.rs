use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use domain::{UiCodeTemplateLanguage, UiComponentRecordOrigin};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, console_put, ConsoleRouteAssembly,
    },
};

use super::ui_management_interface::{UiManagementInput, UiManagementOutput};

async fn invoke_ui_management(
    state: Arc<ApiState>,
    headers: HeaderMap,
    binding_id: &'static str,
    input: UiManagementInput,
    mutating: bool,
) -> Result<UiManagementOutput, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let credential = if mutating {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers }
    };
    crate::routes::console_interface::invoke(snapshot_state, binding_id, credential, input).await
}

#[derive(Debug, Deserialize)]
pub struct ListTemplatesQuery {
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TemplateBody {
    pub provider_code: String,
    pub contribution_code: String,
    pub name: String,
    pub source: String,
    #[schema(value_type = String)]
    pub language: UiCodeTemplateLanguage,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTemplateBody {
    pub name: String,
    pub source: String,
    #[schema(value_type = String)]
    pub language: UiCodeTemplateLanguage,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PublishTemplateBody {
    pub revision: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ArchiveTemplateBody {
    pub archived: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResetDefaultTemplateBody {
    pub provider_code: String,
    pub contribution_code: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ComponentUpstreamBody {
    pub identity: String,
    pub version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateComponentBody {
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    pub source: String,
    pub group: String,
    pub upstream: ComponentUpstreamBody,
    pub version: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateComponentBody {
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    pub source: String,
    pub group: String,
    pub upstream: ComponentUpstreamBody,
    pub version: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TemplateRevisionResponse {
    pub revision: i32,
    pub source: String,
    #[schema(value_type = String)]
    pub language: UiCodeTemplateLanguage,
    pub is_published: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ManagedTemplateResponse {
    pub id: String,
    pub provider_code: String,
    pub contribution_code: String,
    pub name: String,
    pub latest_revision: TemplateRevisionResponse,
    pub published_revision: Option<TemplateRevisionResponse>,
    pub is_default: bool,
    pub is_archived: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OfficialTemplateResponse {
    pub provider_code: String,
    pub contribution_code: String,
    pub title: String,
    pub source: String,
    #[schema(value_type = String)]
    pub language: UiCodeTemplateLanguage,
    pub version: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TemplateListResponse {
    pub official: Vec<OfficialTemplateResponse>,
    pub managed: Vec<ManagedTemplateResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ComponentRecordResponse {
    pub id: String,
    pub scope_id: String,
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    #[schema(value_type = String)]
    pub origin: UiComponentRecordOrigin,
    pub source: String,
    pub group: String,
    pub upstream: ComponentUpstreamBody,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_updated_at: Option<String>,
    pub source_locator: Option<String>,
    pub source_checksum: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CatalogSearchQuery {
    pub q: String,
    #[serde(default = "default_catalog_page")]
    pub page: u32,
    #[serde(default = "default_catalog_page_size")]
    pub page_size: usize,
}

fn default_catalog_page() -> u32 {
    1
}

fn default_catalog_page_size() -> usize {
    20
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogIndexResponse {
    pub catalog_version: String,
    pub generated_at: String,
    pub page_size: usize,
    pub total_components: usize,
    pub source_fingerprint: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogComponentResponse {
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub import_code: String,
    pub source_code: String,
    pub source: String,
    pub group: String,
    pub upstream: ComponentUpstreamBody,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_updated_at: String,
    pub source_locator: String,
    pub source_checksum: String,
    pub local_version: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogPageResponse {
    pub catalog_version: String,
    pub total_components: usize,
    pub page_size: usize,
    pub page: u32,
    pub cursor: String,
    pub next_cursor: Option<String>,
    pub records: Vec<CatalogComponentResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogSearchEntryResponse {
    pub component_code: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub group: String,
    pub upstream: ComponentUpstreamBody,
    pub version: String,
    pub keywords: Vec<String>,
    pub catalog_page: u32,
    pub local_version: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogSearchResponse {
    pub catalog_version: String,
    pub page: u32,
    pub page_size: usize,
    pub total_entries: usize,
    pub entries: Vec<CatalogSearchEntryResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogGroupUpdateResponse {
    pub source: String,
    pub group: String,
    pub remote_records: usize,
    pub new_or_updated_records: usize,
    pub removed_records: usize,
    pub update_available: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogUpdateStatusResponse {
    pub catalog_version: String,
    pub source_fingerprint: String,
    pub update_available: bool,
    pub groups: Vec<CatalogGroupUpdateResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CatalogSyncResponse {
    pub synchronized_records: usize,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    let owner = |operation_id: &str| ConsoleOperation(operation_id.to_string());
    ConsoleRouteAssembly::new()
        .route(
            "/settings/ui-management/templates",
            console_get(list_templates, owner("ui_management.templates.list"))
                .post(create_template, owner("ui_management.templates.create")),
        )
        .route(
            "/settings/ui-management/templates/default",
            console_delete(
                reset_default_template,
                owner("ui_management.templates.default.reset"),
            ),
        )
        .route(
            "/settings/ui-management/templates/:id",
            console_put(update_template, owner("ui_management.templates.update")),
        )
        .route(
            "/settings/ui-management/templates/:id/publish",
            console_post(publish_template, owner("ui_management.templates.publish")),
        )
        .route(
            "/settings/ui-management/templates/:id/default",
            console_put(
                set_default_template,
                owner("ui_management.templates.default.set"),
            ),
        )
        .route(
            "/settings/ui-management/templates/:id/archive",
            console_put(archive_template, owner("ui_management.templates.archive")),
        )
        .route(
            "/settings/ui-management/components",
            console_get(list_components, owner("ui_management.components.list"))
                .post(create_component, owner("ui_management.components.create")),
        )
        .route(
            "/settings/ui-management/components/:id",
            console_get(get_component, owner("ui_management.components.view"))
                .put(update_component, owner("ui_management.components.update"))
                .delete(delete_component, owner("ui_management.components.delete")),
        )
        .route(
            "/settings/ui-management/components/catalog/index",
            console_get(catalog_index, owner("ui_management.catalog.index")),
        )
        .route(
            "/settings/ui-management/components/catalog/pages/:page",
            console_get(catalog_page, owner("ui_management.catalog.page")),
        )
        .route(
            "/settings/ui-management/components/catalog/search",
            console_get(catalog_search, owner("ui_management.catalog.search")),
        )
        .route(
            "/settings/ui-management/components/catalog/update-status",
            console_get(
                catalog_update_status,
                owner("ui_management.catalog.update_status"),
            ),
        )
        .route(
            "/settings/ui-management/components/catalog/:component_code/download",
            console_post(catalog_download, owner("ui_management.catalog.download")),
        )
        .route(
            "/settings/ui-management/components/catalog/groups/:source/:group/sync",
            console_post(
                catalog_sync_group,
                owner("ui_management.catalog.sync_group"),
            ),
        )
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/templates", responses((status = 200, body = TemplateListResponse), (status = 403, body = crate::error_response::ErrorBody)))]
pub async fn list_templates(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ListTemplatesQuery>,
) -> Result<Json<ApiSuccess<TemplateListResponse>>, ApiError> {
    let UiManagementOutput::Templates(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.list.get.v1",
        UiManagementInput::ListTemplates(query),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/templates", request_body = TemplateBody, responses((status = 201, body = ManagedTemplateResponse), (status = 403, body = crate::error_response::ErrorBody)))]
pub async fn create_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<TemplateBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ManagedTemplateResponse>>), ApiError> {
    let UiManagementOutput::Template(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.create.post.v1",
        UiManagementInput::CreateTemplate(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}

#[utoipa::path(put, path = "/api/console/settings/ui-management/templates/{id}", request_body = UpdateTemplateBody, params(("id" = String, Path)), responses((status = 200, body = ManagedTemplateResponse)))]
pub async fn update_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateTemplateBody>,
) -> Result<Json<ApiSuccess<ManagedTemplateResponse>>, ApiError> {
    let UiManagementOutput::Template(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.update.put.v1",
        UiManagementInput::UpdateTemplate { id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/templates/{id}/publish", request_body = PublishTemplateBody, params(("id" = String, Path)), responses((status = 200, body = ManagedTemplateResponse)))]
pub async fn publish_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<PublishTemplateBody>,
) -> Result<Json<ApiSuccess<ManagedTemplateResponse>>, ApiError> {
    let UiManagementOutput::Template(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.publish.post.v1",
        UiManagementInput::PublishTemplate { id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}
#[utoipa::path(put, path = "/api/console/settings/ui-management/templates/{id}/default", params(("id" = String, Path)), responses((status = 204)))]
pub async fn set_default_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let UiManagementOutput::NoContent = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.default.set.put.v1",
        UiManagementInput::SetDefaultTemplate { id },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(delete, path = "/api/console/settings/ui-management/templates/default", request_body = ResetDefaultTemplateBody, responses((status = 204)))]
pub async fn reset_default_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ResetDefaultTemplateBody>,
) -> Result<StatusCode, ApiError> {
    let UiManagementOutput::NoContent = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.default.reset.delete.v1",
        UiManagementInput::ResetDefaultTemplate(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(put, path = "/api/console/settings/ui-management/templates/{id}/archive", request_body = ArchiveTemplateBody, params(("id" = String, Path)), responses((status = 200, body = ManagedTemplateResponse)))]
pub async fn archive_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ArchiveTemplateBody>,
) -> Result<Json<ApiSuccess<ManagedTemplateResponse>>, ApiError> {
    let UiManagementOutput::Template(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.templates.archive.put.v1",
        UiManagementInput::ArchiveTemplate { id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components", responses((status = 200, body = [ComponentRecordResponse])))]
pub async fn list_components(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<ComponentRecordResponse>>>, ApiError> {
    let UiManagementOutput::Components(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.components.list.get.v1",
        UiManagementInput::ListComponents,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components/{id}", params(("id" = String, Path)), responses((status = 200, body = ComponentRecordResponse), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn get_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<ComponentRecordResponse>>, ApiError> {
    let UiManagementOutput::Component(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.components.view.get.v1",
        UiManagementInput::GetComponent { id },
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/components", request_body = CreateComponentBody, responses((status = 201, body = ComponentRecordResponse), (status = 400, body = crate::error_response::ErrorBody)))]
pub async fn create_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateComponentBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ComponentRecordResponse>>), ApiError> {
    let UiManagementOutput::Component(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.components.create.post.v1",
        UiManagementInput::CreateComponent(body),
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(value))))
}

#[utoipa::path(put, path = "/api/console/settings/ui-management/components/{id}", params(("id" = String, Path)), request_body = UpdateComponentBody, responses((status = 200, body = ComponentRecordResponse), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn update_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateComponentBody>,
) -> Result<Json<ApiSuccess<ComponentRecordResponse>>, ApiError> {
    let UiManagementOutput::Component(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.components.update.put.v1",
        UiManagementInput::UpdateComponent { id, body },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(delete, path = "/api/console/settings/ui-management/components/{id}", params(("id" = String, Path)), responses((status = 204), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn delete_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let UiManagementOutput::NoContent = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.components.delete.delete.v1",
        UiManagementInput::DeleteComponent { id },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components/catalog/index", responses((status = 200, body = CatalogIndexResponse), (status = 403, body = crate::error_response::ErrorBody)))]
pub async fn catalog_index(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<CatalogIndexResponse>>, ApiError> {
    let UiManagementOutput::CatalogIndex(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.catalog.index.get.v1",
        UiManagementInput::CatalogIndex,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components/catalog/pages/{page}", params(("page" = u32, Path)), responses((status = 200, body = CatalogPageResponse)))]
pub async fn catalog_page(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(page): Path<u32>,
) -> Result<Json<ApiSuccess<CatalogPageResponse>>, ApiError> {
    let UiManagementOutput::CatalogPage(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.catalog.page.get.v1",
        UiManagementInput::CatalogPage { page },
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components/catalog/search", params(("q" = String, Query), ("page" = Option<u32>, Query), ("page_size" = Option<usize>, Query)), responses((status = 200, body = CatalogSearchResponse)))]
pub async fn catalog_search(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<CatalogSearchQuery>,
) -> Result<Json<ApiSuccess<CatalogSearchResponse>>, ApiError> {
    let UiManagementOutput::CatalogSearch(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.catalog.search.get.v1",
        UiManagementInput::CatalogSearch(query),
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components/catalog/update-status", responses((status = 200, body = CatalogUpdateStatusResponse)))]
pub async fn catalog_update_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<CatalogUpdateStatusResponse>>, ApiError> {
    let UiManagementOutput::CatalogUpdateStatus(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.catalog.update-status.get.v1",
        UiManagementInput::CatalogUpdateStatus,
        false,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/components/catalog/{component_code}/download", params(("component_code" = String, Path)), responses((status = 200, body = CatalogComponentResponse)))]
pub async fn catalog_download(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(component_code): Path<String>,
) -> Result<Json<ApiSuccess<CatalogComponentResponse>>, ApiError> {
    let UiManagementOutput::CatalogComponent(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.catalog.download.post.v1",
        UiManagementInput::CatalogDownload { component_code },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/components/catalog/groups/{source}/{group}/sync", params(("source" = String, Path), ("group" = String, Path)), responses((status = 200, body = CatalogSyncResponse)))]
pub async fn catalog_sync_group(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((source, group)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<CatalogSyncResponse>>, ApiError> {
    let UiManagementOutput::CatalogSync(value) = invoke_ui_management(
        state,
        headers,
        "http.console.ui-management.catalog.sync-group.post.v1",
        UiManagementInput::CatalogSyncGroup { source, group },
        true,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}
