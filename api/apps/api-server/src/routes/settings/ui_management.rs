use std::sync::Arc;

use access_control::SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_PERMISSION;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use control_plane::{
    ports::{CreateUiCodeTemplateInput, ReviseUiCodeTemplateInput, ReviseUiComponentContractInput},
    ui_management::{OfficialUiCodeTemplate, UiComponentCandidate, UiManagementService},
};
use domain::{
    FrontendComponentContract, UiCodeTemplate, UiCodeTemplateLanguage, UiComponentLocator,
    UiComponentOverrideState,
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

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ComponentLocatorBody {
    pub provider_code: String,
    pub contribution_code: String,
    pub module_source: String,
    pub export_name: String,
}

impl From<ComponentLocatorBody> for UiComponentLocator {
    fn from(value: ComponentLocatorBody) -> Self {
        Self {
            provider_code: value.provider_code,
            contribution_code: value.contribution_code,
            module_source: value.module_source,
            export_name: value.export_name,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ComponentContractBody {
    #[serde(flatten)]
    pub locator: ComponentLocatorBody,
    #[schema(value_type = Object)]
    pub contract: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ComponentStateBody {
    #[serde(flatten)]
    pub locator: ComponentLocatorBody,
    #[schema(value_type = String)]
    pub state: UiComponentOverrideState,
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
pub struct ComponentCandidateResponse {
    pub provider_code: String,
    pub contribution_code: String,
    pub module_source: String,
    pub module_version: String,
    pub export_name: String,
    #[schema(value_type = String)]
    pub state: UiComponentOverrideState,
    #[schema(value_type = Option<Object>)]
    pub official_contract: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub latest_contract: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub published_contract: Option<serde_json::Value>,
    pub latest_revision: Option<i32>,
    pub published_revision: Option<i32>,
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

fn component_response(value: UiComponentCandidate) -> ComponentCandidateResponse {
    let state = value
        .override_record
        .as_ref()
        .map(|v| v.state)
        .unwrap_or(UiComponentOverrideState::Inherit);
    ComponentCandidateResponse {
        provider_code: value.locator.provider_code,
        contribution_code: value.locator.contribution_code,
        module_source: value.locator.module_source,
        module_version: value.module_version,
        export_name: value.locator.export_name,
        state,
        official_contract: value
            .official_contract
            .and_then(|v| serde_json::to_value(v).ok()),
        latest_contract: value
            .override_record
            .as_ref()
            .and_then(|v| v.latest_revision.as_ref())
            .and_then(|v| serde_json::to_value(&v.contract).ok()),
        published_contract: value
            .override_record
            .as_ref()
            .and_then(|v| v.published_revision.as_ref())
            .and_then(|v| serde_json::to_value(&v.contract).ok()),
        latest_revision: value
            .override_record
            .as_ref()
            .and_then(|v| v.latest_revision.as_ref())
            .map(|v| v.revision),
        published_revision: value
            .override_record
            .as_ref()
            .and_then(|v| v.published_revision.as_ref())
            .map(|v| v.revision),
    }
}

fn parse_id(value: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(value)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("template_id").into())
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;
    let owner = || ConsoleOperation(SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_PERMISSION.to_string());
    ConsoleRouteAssembly::new()
        .route(
            "/settings/ui-management/templates",
            console_get(list_templates, owner()).post(create_template, owner()),
        )
        .route(
            "/settings/ui-management/templates/default",
            console_delete(reset_default_template, owner()),
        )
        .route(
            "/settings/ui-management/templates/:id",
            console_put(update_template, owner()),
        )
        .route(
            "/settings/ui-management/templates/:id/publish",
            console_post(publish_template, owner()),
        )
        .route(
            "/settings/ui-management/templates/:id/default",
            console_put(set_default_template, owner()),
        )
        .route(
            "/settings/ui-management/templates/:id/archive",
            console_put(archive_template, owner()),
        )
        .route(
            "/settings/ui-management/components",
            console_get(list_components, owner()),
        )
        .route(
            "/settings/ui-management/components/contract",
            console_put(update_component_contract, owner()),
        )
        .route(
            "/settings/ui-management/components/state",
            console_put(update_component_state, owner()),
        )
}

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

pub async fn list_components(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<ComponentCandidateResponse>>>, ApiError> {
    require_session(&state, &headers).await?;
    let values = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .list_component_candidates()
        .await?;
    Ok(Json(ApiSuccess::new(
        values.into_iter().map(component_response).collect(),
    )))
}
pub async fn update_component_contract(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ComponentContractBody>,
) -> Result<Json<ApiSuccess<ComponentCandidateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let locator: UiComponentLocator = body.locator.into();
    let contract: FrontendComponentContract = serde_json::from_value(body.contract)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("contract"))?;
    UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .revise_component_contract(ReviseUiComponentContractInput {
            locator: locator.clone(),
            contract,
            actor_user_id: context.user.id,
        })
        .await?;
    let candidate = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .list_component_candidates()
        .await?
        .into_iter()
        .find(|v| v.locator == locator)
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "component",
        ))?;
    Ok(Json(ApiSuccess::new(component_response(candidate))))
}
pub async fn update_component_state(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ComponentStateBody>,
) -> Result<Json<ApiSuccess<ComponentCandidateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let locator: UiComponentLocator = body.locator.into();
    UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .set_component_state(&locator, body.state, context.user.id)
        .await?;
    let candidate = UiManagementService::new(state.store.clone(), state.api_node_id.clone())
        .list_component_candidates()
        .await?
        .into_iter()
        .find(|v| v.locator == locator)
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "component",
        ))?;
    Ok(Json(ApiSuccess::new(component_response(candidate))))
}
