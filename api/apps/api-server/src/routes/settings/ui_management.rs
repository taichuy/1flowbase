use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::{
    ports::{
        CreateUiCodeTemplateInput, CreateUiComponentRecordInput, ReviseUiCodeTemplateInput,
        UiComponentRecordPatch,
    },
    ui_management::{OfficialUiCodeTemplate, UiManagementService},
};
use domain::{
    UiCodeTemplate, UiCodeTemplateLanguage, UiComponentRecord, UiComponentRecordOrigin,
    UiComponentRecordUpstream,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_post, console_put, ConsoleRouteAssembly,
    },
};

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
    pub created_at: String,
    pub updated_at: String,
}

fn template_response(value: UiCodeTemplate) -> ManagedTemplateResponse {
    ManagedTemplateResponse {
        id: value.id.to_string(),
        provider_code: value.provider_code,
        contribution_code: value.contribution_code,
        name: value.name,
        latest_revision: TemplateRevisionResponse {
            revision: value.latest_revision.revision,
            source: value.latest_revision.source,
            language: value.latest_revision.language,
            is_published: value.latest_revision.is_published,
        },
        published_revision: value.published_revision.map(|r| TemplateRevisionResponse {
            revision: r.revision,
            source: r.source,
            language: r.language,
            is_published: true,
        }),
        is_default: value.is_default,
        is_archived: value.archived_at.is_some(),
    }
}

fn official_response(value: OfficialUiCodeTemplate) -> OfficialTemplateResponse {
    OfficialTemplateResponse {
        provider_code: value.provider_code,
        contribution_code: value.contribution_code,
        title: value.title,
        source: value.source,
        language: value.language,
        version: value.version,
        is_default: value.is_default,
    }
}

fn component_response(value: UiComponentRecord) -> Result<ComponentRecordResponse, ApiError> {
    use time::format_description::well_known::Rfc3339;
    Ok(ComponentRecordResponse {
        id: value.id.to_string(),
        scope_id: value.scope_id.to_string(),
        component_code: value.component_code,
        name: value.name,
        description: value.description,
        import_code: value.import_code,
        source_code: value.source_code,
        origin: value.origin,
        source: value.source,
        group: value.group,
        upstream: ComponentUpstreamBody {
            identity: value.upstream.identity,
            version: value.upstream.version,
        },
        version: value.version,
        keywords: value.keywords,
        created_at: value.created_at.format(&Rfc3339)?,
        updated_at: value.updated_at.format(&Rfc3339)?,
    })
}

fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("template_id").into())
}

fn parse_component_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("ui_component_record_id").into()
    })
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
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/templates", responses((status = 200, body = TemplateListResponse), (status = 403, body = crate::error_response::ErrorBody)))]
pub async fn list_templates(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ListTemplatesQuery>,
) -> Result<Json<ApiSuccess<TemplateListResponse>>, ApiError> {
    require_session(&state, &headers).await?;
    let (official, managed) =
        UiManagementService::new(state.store.clone(), state.api_node_id.clone())
            .list_templates(query.include_archived)
            .await?;
    Ok(Json(ApiSuccess::new(TemplateListResponse {
        official: official.into_iter().map(official_response).collect(),
        managed: managed.into_iter().map(template_response).collect(),
    })))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/templates", request_body = TemplateBody, responses((status = 201, body = ManagedTemplateResponse), (status = 403, body = crate::error_response::ErrorBody)))]
pub async fn create_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<TemplateBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ManagedTemplateResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .create_template(CreateUiCodeTemplateInput {
            provider_code: body.provider_code,
            contribution_code: body.contribution_code,
            name: body.name,
            source: body.source,
            language: body.language,
            actor_user_id: context.user.id,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(template_response(value))),
    ))
}

#[utoipa::path(put, path = "/api/console/settings/ui-management/templates/{id}", request_body = UpdateTemplateBody, params(("id" = String, Path)), responses((status = 200, body = ManagedTemplateResponse)))]
pub async fn update_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateTemplateBody>,
) -> Result<Json<ApiSuccess<ManagedTemplateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .revise_template(ReviseUiCodeTemplateInput {
            template_id: parse_id(&id)?,
            name: body.name,
            source: body.source,
            language: body.language,
            actor_user_id: context.user.id,
        })
        .await?;
    Ok(Json(ApiSuccess::new(template_response(value))))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/templates/{id}/publish", request_body = PublishTemplateBody, params(("id" = String, Path)), responses((status = 200, body = ManagedTemplateResponse)))]
pub async fn publish_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<PublishTemplateBody>,
) -> Result<Json<ApiSuccess<ManagedTemplateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .publish_template(parse_id(&id)?, body.revision, context.user.id)
        .await?;
    Ok(Json(ApiSuccess::new(template_response(value))))
}
#[utoipa::path(put, path = "/api/console/settings/ui-management/templates/{id}/default", params(("id" = String, Path)), responses((status = 204)))]
pub async fn set_default_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .set_template_default(parse_id(&id)?, context.user.id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(delete, path = "/api/console/settings/ui-management/templates/default", request_body = ResetDefaultTemplateBody, responses((status = 204)))]
pub async fn reset_default_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ResetDefaultTemplateBody>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .reset_template_default(&body.provider_code, &body.contribution_code)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(put, path = "/api/console/settings/ui-management/templates/{id}/archive", request_body = ArchiveTemplateBody, params(("id" = String, Path)), responses((status = 200, body = ManagedTemplateResponse)))]
pub async fn archive_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ArchiveTemplateBody>,
) -> Result<Json<ApiSuccess<ManagedTemplateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .set_template_archived(parse_id(&id)?, body.archived, context.user.id)
        .await?;
    Ok(Json(ApiSuccess::new(template_response(value))))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components", responses((status = 200, body = [ComponentRecordResponse])))]
pub async fn list_components(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<ComponentRecordResponse>>>, ApiError> {
    require_session(&state, &headers).await?;
    let values = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .list_component_records()
        .await?;
    let response = values
        .into_iter()
        .map(component_response)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(get, path = "/api/console/settings/ui-management/components/{id}", params(("id" = String, Path)), responses((status = 200, body = ComponentRecordResponse), (status = 404, body = crate::error_response::ErrorBody)))]
pub async fn get_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApiSuccess<ComponentRecordResponse>>, ApiError> {
    require_session(&state, &headers).await?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .get_component_record(parse_component_id(&id)?)
        .await?;
    Ok(Json(ApiSuccess::new(component_response(value)?)))
}

#[utoipa::path(post, path = "/api/console/settings/ui-management/components", request_body = CreateComponentBody, responses((status = 201, body = ComponentRecordResponse), (status = 400, body = crate::error_response::ErrorBody)))]
pub async fn create_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateComponentBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ComponentRecordResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .create_component_record(CreateUiComponentRecordInput {
            component_code: body.component_code,
            name: body.name,
            description: body.description,
            import_code: body.import_code,
            source_code: body.source_code,
            source: body.source,
            group: body.group,
            upstream: UiComponentRecordUpstream {
                identity: body.upstream.identity,
                version: body.upstream.version,
            },
            version: body.version,
            keywords: body.keywords,
            actor_user_id: context.user.id,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(component_response(value)?)),
    ))
}

#[utoipa::path(put, path = "/api/console/settings/ui-management/components/{id}", params(("id" = String, Path)), request_body = UpdateComponentBody, responses((status = 200, body = ComponentRecordResponse), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn update_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateComponentBody>,
) -> Result<Json<ApiSuccess<ComponentRecordResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let value = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .update_component_record(
            parse_component_id(&id)?,
            UiComponentRecordPatch {
                name: body.name,
                description: body.description,
                import_code: body.import_code,
                source_code: body.source_code,
                source: body.source,
                group: body.group,
                upstream: UiComponentRecordUpstream {
                    identity: body.upstream.identity,
                    version: body.upstream.version,
                },
                version: body.version,
                keywords: body.keywords,
                actor_user_id: context.user.id,
            },
        )
        .await?;
    Ok(Json(ApiSuccess::new(component_response(value)?)))
}

#[utoipa::path(delete, path = "/api/console/settings/ui-management/components/{id}", params(("id" = String, Path)), responses((status = 204), (status = 409, body = crate::error_response::ErrorBody)))]
pub async fn delete_component(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .delete_component_record(parse_component_id(&id)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
