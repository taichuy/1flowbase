use std::sync::Arc;

use access_control::{
    APPLICATIONS_API_SET_ENABLED_OPERATION_ID, APPLICATIONS_PUBLISH_OPERATION_ID,
    APPLICATIONS_UPDATE_OPERATION_ID, APPLICATIONS_VIEW_OPERATION_ID,
};
use axum::{
    extract::{Path, Query, State},
    http::{header::ACCEPT_LANGUAGE, HeaderMap, StatusCode},
    response::IntoResponse,
    Json, Router,
};
use control_plane::{
    application::ApplicationService,
    application_public_api::{
        api_keys::{
            ApplicationApiKeyService, CreateApplicationApiKeyCommand,
            ListApplicationApiKeysCommand, RevokeApplicationApiKeyCommand,
        },
        mapping::{
            ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
            ApplicationApiMappingService, GetApplicationApiMappingCommand,
            ReplaceApplicationApiMappingCommand, WorkflowExtensionApiConfig,
            WorkflowExtensionHttpMethod, WorkflowExtensionParameterMapping,
            WorkflowExtensionParameterSource, WorkflowExtensionResponseMode,
        },
        publications::{
            ApplicationPublicationService, ApplicationPublicationVersionRecord,
            LoadActiveApplicationPublicationCommand, PublishApplicationCommand,
            SetApplicationApiEnabledCommand, UnpublishApplicationCommand,
        },
        workflow_schedule::{
            GetWorkflowScheduleTriggerCommand, ReplaceWorkflowScheduleTriggerCommand,
            WorkflowScheduleTriggerRecord, WorkflowScheduleTriggerService,
        },
    },
    errors::ControlPlaneError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    application_public_docs::{
        build_application_public_docs_catalog, build_application_public_docs_category_operations,
        build_application_public_docs_category_spec, build_application_public_docs_operation_spec,
        ApplicationPublicDocsContext,
    },
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    openapi_docs::{
        filter_category_operations, paginate_category_operations, DocsCatalog,
        DocsCatalogCategoryOperationsPage, DOCS_OPERATIONS_PAGE_SIZE,
    },
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

const PUBLIC_RUNS_PATH: &str = "/api/agent/v1/runs";

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApplicationApiKeyBody {
    pub name: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub creator_user_id: Uuid,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedApplicationApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub token: String,
    pub token_prefix: String,
    pub creator_user_id: Uuid,
    pub enabled: bool,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationApiMappingInputBody {
    pub query_target: String,
    pub model_target: Option<String>,
    pub inputs_target: Option<String>,
    pub history_target: Option<String>,
    pub attachments_target: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationApiMappingOutputBody {
    pub answer_selector: Option<String>,
    pub usage_selector: Option<String>,
    pub files_selector: Option<String>,
    pub error_selector: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ApplicationApiMappingBody {
    pub input: ApplicationApiMappingInputBody,
    pub output: ApplicationApiMappingOutputBody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension: Option<WorkflowExtensionApiBody>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct WorkflowExtensionApiBody {
    pub slug: String,
    pub method: WorkflowExtensionHttpMethodBody,
    pub response_mode: WorkflowExtensionResponseModeBody,
    #[serde(default)]
    pub parameters: Vec<WorkflowExtensionParameterMappingBody>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkflowExtensionHttpMethodBody {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExtensionResponseModeBody {
    Sync,
    Async,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct WorkflowExtensionParameterMappingBody {
    pub name: String,
    pub source: WorkflowExtensionParameterSourceBody,
    pub target: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExtensionParameterSourceBody {
    Path,
    Query,
    Form,
    Body,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ApplicationApiDocsQuery {
    pub locale: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub q: Option<String>,
}

impl ApplicationApiDocsQuery {
    fn offset(&self) -> usize {
        self.offset.unwrap_or(0)
    }

    fn limit(&self) -> usize {
        self.limit
            .unwrap_or(DOCS_OPERATIONS_PAGE_SIZE)
            .clamp(1, DOCS_OPERATIONS_PAGE_SIZE)
    }

    fn search_query(&self) -> Option<&str> {
        self.q.as_deref()
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PublishApplicationApiBody {
    pub mapping: ApplicationApiMappingBody,
    pub api_enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchApplicationApiStatusBody {
    pub api_enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct WorkflowScheduleTriggerBody {
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkflowScheduleTriggerResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub enabled: bool,
    pub cron: String,
    pub timezone: String,
    pub input_payload: Value,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationApiStatusResponse {
    pub application_id: Uuid,
    pub api_enabled: bool,
    pub public_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationPublicationJsDependencyPermissionsResponse {
    pub network: String,
    pub filesystem: String,
    pub env: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationPublicationJsDependencySnapshotResponse {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
    pub permissions: ApplicationPublicationJsDependencyPermissionsResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationPublicationResponse {
    pub id: Uuid,
    pub application_id: Uuid,
    pub flow_id: Uuid,
    pub flow_version_id: Uuid,
    pub compiled_plan_id: Uuid,
    pub version_sequence: i64,
    pub active: bool,
    pub api_enabled: bool,
    pub mapping_snapshot: ApplicationApiMappingBody,
    pub dependency_snapshot: Vec<ApplicationPublicationJsDependencySnapshotResponse>,
    pub public_url: String,
    pub created_by: Uuid,
    pub created_at: String,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/applications/:application_id/api-keys",
            console_get(
                list_application_api_keys,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .post(
                create_application_api_key,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-keys/:key_id",
            console_delete(
                revoke_application_api_key,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-mapping",
            console_get(
                get_application_api_mapping,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .put(
                replace_application_api_mapping,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-publication",
            console_get(
                get_application_api_publication,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .delete(
                unpublish_application_api,
                ConsoleOperation(APPLICATIONS_PUBLISH_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-publications",
            console_post(
                publish_application_api,
                ConsoleOperation(APPLICATIONS_PUBLISH_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-status",
            console_patch(
                patch_application_api_status,
                ConsoleOperation(APPLICATIONS_API_SET_ENABLED_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/workflow-schedule-trigger",
            console_get(
                get_workflow_schedule_trigger,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .put(
                replace_workflow_schedule_trigger,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-docs/catalog",
            console_get(
                get_application_api_docs_catalog,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-docs/categories/:category_id/operations",
            console_get(
                get_application_api_docs_category_operations,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-docs/categories/:category_id/openapi.json",
            console_get(
                get_application_api_docs_category_openapi,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:application_id/api-docs/operations/:operation_id/openapi.json",
            console_get(
                get_application_api_docs_operation_openapi,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            ),
        )
}

fn parse_expires_at(raw: Option<String>) -> Result<Option<OffsetDateTime>, ApiError> {
    raw.map(|value| {
        OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("expires_at").into()
        })
    })
    .transpose()
}

fn format_optional_time(value: Option<OffsetDateTime>) -> Option<String> {
    value.map(|value| value.format(&Rfc3339).unwrap())
}

fn format_time(value: OffsetDateTime) -> String {
    value.format(&Rfc3339).unwrap()
}

fn to_workflow_schedule_trigger_response(
    trigger: WorkflowScheduleTriggerRecord,
) -> WorkflowScheduleTriggerResponse {
    WorkflowScheduleTriggerResponse {
        id: trigger.id,
        workspace_id: trigger.workspace_id,
        application_id: trigger.application_id,
        enabled: trigger.enabled,
        cron: trigger.cron,
        timezone: trigger.timezone,
        input_payload: trigger.input_payload,
        created_by: trigger.created_by,
        updated_by: trigger.updated_by,
        created_at: format_time(trigger.created_at),
        updated_at: format_time(trigger.updated_at),
    }
}

fn to_api_key_response(api_key: domain::ApiKeyRecord) -> ApplicationApiKeyResponse {
    ApplicationApiKeyResponse {
        id: api_key.id,
        name: api_key.name,
        token_prefix: api_key.token_prefix,
        creator_user_id: api_key.creator_user_id,
        enabled: api_key.enabled,
        expires_at: format_optional_time(api_key.expires_at),
        last_used_at: format_optional_time(api_key.last_used_at),
        created_at: format_time(api_key.created_at),
        updated_at: format_time(api_key.updated_at),
    }
}

fn to_created_api_key_response(
    api_key: domain::ApiKeyRecord,
    token: String,
) -> CreatedApplicationApiKeyResponse {
    CreatedApplicationApiKeyResponse {
        id: api_key.id,
        name: api_key.name,
        token,
        token_prefix: api_key.token_prefix,
        creator_user_id: api_key.creator_user_id,
        enabled: api_key.enabled,
        expires_at: format_optional_time(api_key.expires_at),
        last_used_at: format_optional_time(api_key.last_used_at),
        created_at: format_time(api_key.created_at),
        updated_at: format_time(api_key.updated_at),
    }
}

fn to_mapping_config(body: ApplicationApiMappingBody) -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: body.input.query_target,
            model_target: body.input.model_target,
            inputs_target: body.input.inputs_target,
            history_target: body.input.history_target,
            attachments_target: body.input.attachments_target,
        },
        output: ApplicationApiMappingOutput {
            answer_selector: body.output.answer_selector,
            usage_selector: body.output.usage_selector,
            files_selector: body.output.files_selector,
            error_selector: body.output.error_selector,
        },
        extension: body.extension.map(to_extension_config),
    }
}

fn to_mapping_body(mapping: ApplicationApiMappingConfig) -> ApplicationApiMappingBody {
    ApplicationApiMappingBody {
        input: ApplicationApiMappingInputBody {
            query_target: mapping.input.query_target,
            model_target: mapping.input.model_target,
            inputs_target: mapping.input.inputs_target,
            history_target: mapping.input.history_target,
            attachments_target: mapping.input.attachments_target,
        },
        output: ApplicationApiMappingOutputBody {
            answer_selector: mapping.output.answer_selector,
            usage_selector: mapping.output.usage_selector,
            files_selector: mapping.output.files_selector,
            error_selector: mapping.output.error_selector,
        },
        extension: mapping.extension.map(to_extension_body),
    }
}

fn to_extension_config(body: WorkflowExtensionApiBody) -> WorkflowExtensionApiConfig {
    WorkflowExtensionApiConfig {
        slug: body.slug,
        method: match body.method {
            WorkflowExtensionHttpMethodBody::Get => WorkflowExtensionHttpMethod::Get,
            WorkflowExtensionHttpMethodBody::Post => WorkflowExtensionHttpMethod::Post,
            WorkflowExtensionHttpMethodBody::Put => WorkflowExtensionHttpMethod::Put,
            WorkflowExtensionHttpMethodBody::Patch => WorkflowExtensionHttpMethod::Patch,
            WorkflowExtensionHttpMethodBody::Delete => WorkflowExtensionHttpMethod::Delete,
            WorkflowExtensionHttpMethodBody::Head => WorkflowExtensionHttpMethod::Head,
            WorkflowExtensionHttpMethodBody::Options => WorkflowExtensionHttpMethod::Options,
        },
        response_mode: match body.response_mode {
            WorkflowExtensionResponseModeBody::Sync => WorkflowExtensionResponseMode::Sync,
            WorkflowExtensionResponseModeBody::Async => WorkflowExtensionResponseMode::Async,
        },
        parameters: body
            .parameters
            .into_iter()
            .map(|parameter| WorkflowExtensionParameterMapping {
                name: parameter.name,
                source: match parameter.source {
                    WorkflowExtensionParameterSourceBody::Path => {
                        WorkflowExtensionParameterSource::Path
                    }
                    WorkflowExtensionParameterSourceBody::Query => {
                        WorkflowExtensionParameterSource::Query
                    }
                    WorkflowExtensionParameterSourceBody::Form => {
                        WorkflowExtensionParameterSource::Form
                    }
                    WorkflowExtensionParameterSourceBody::Body => {
                        WorkflowExtensionParameterSource::Body
                    }
                },
                target: parameter.target,
            })
            .collect(),
    }
}

fn to_extension_body(config: WorkflowExtensionApiConfig) -> WorkflowExtensionApiBody {
    WorkflowExtensionApiBody {
        slug: config.slug,
        method: match config.method {
            WorkflowExtensionHttpMethod::Get => WorkflowExtensionHttpMethodBody::Get,
            WorkflowExtensionHttpMethod::Post => WorkflowExtensionHttpMethodBody::Post,
            WorkflowExtensionHttpMethod::Put => WorkflowExtensionHttpMethodBody::Put,
            WorkflowExtensionHttpMethod::Patch => WorkflowExtensionHttpMethodBody::Patch,
            WorkflowExtensionHttpMethod::Delete => WorkflowExtensionHttpMethodBody::Delete,
            WorkflowExtensionHttpMethod::Head => WorkflowExtensionHttpMethodBody::Head,
            WorkflowExtensionHttpMethod::Options => WorkflowExtensionHttpMethodBody::Options,
        },
        response_mode: match config.response_mode {
            WorkflowExtensionResponseMode::Sync => WorkflowExtensionResponseModeBody::Sync,
            WorkflowExtensionResponseMode::Async => WorkflowExtensionResponseModeBody::Async,
        },
        parameters: config
            .parameters
            .into_iter()
            .map(|parameter| WorkflowExtensionParameterMappingBody {
                name: parameter.name,
                source: match parameter.source {
                    WorkflowExtensionParameterSource::Path => {
                        WorkflowExtensionParameterSourceBody::Path
                    }
                    WorkflowExtensionParameterSource::Query => {
                        WorkflowExtensionParameterSourceBody::Query
                    }
                    WorkflowExtensionParameterSource::Form => {
                        WorkflowExtensionParameterSourceBody::Form
                    }
                    WorkflowExtensionParameterSource::Body => {
                        WorkflowExtensionParameterSourceBody::Body
                    }
                },
                target: parameter.target,
            })
            .collect(),
    }
}

fn to_publication_response(
    publication: ApplicationPublicationVersionRecord,
) -> ApplicationPublicationResponse {
    ApplicationPublicationResponse {
        id: publication.id,
        application_id: publication.application_id,
        flow_id: publication.flow_id,
        flow_version_id: publication.flow_version_id,
        compiled_plan_id: publication.compiled_plan_id,
        version_sequence: publication.version_sequence,
        active: publication.active,
        api_enabled: publication.api_enabled,
        public_url: publication_public_url(&publication),
        mapping_snapshot: to_mapping_body(publication.mapping_snapshot),
        dependency_snapshot: publication
            .dependency_snapshot
            .into_iter()
            .map(
                |dependency| ApplicationPublicationJsDependencySnapshotResponse {
                    installation_id: dependency.installation_id,
                    provider_code: dependency.provider_code,
                    plugin_id: dependency.plugin_id,
                    plugin_version: dependency.plugin_version,
                    alias: dependency.alias,
                    package: dependency.package,
                    version: dependency.version,
                    target: dependency.target,
                    artifact_path: dependency.artifact_path,
                    artifact_hash: dependency.artifact_hash,
                    integrity: dependency.integrity,
                    permissions: ApplicationPublicationJsDependencyPermissionsResponse {
                        network: dependency.permissions.network,
                        filesystem: dependency.permissions.filesystem,
                        env: dependency.permissions.env,
                    },
                },
            )
            .collect(),
        created_by: publication.created_by,
        created_at: format_time(publication.created_at),
    }
}

fn publication_public_url(publication: &ApplicationPublicationVersionRecord) -> String {
    publication
        .extension_slug
        .as_deref()
        .map(|slug| format!("/api/ex/{slug}"))
        .unwrap_or_else(|| PUBLIC_RUNS_PATH.to_string())
}

fn map_publication_not_found(error: anyhow::Error) -> ApiError {
    if error.to_string() == "application_not_published" {
        return ControlPlaneError::NotFound("application_publication").into();
    }
    error.into()
}

fn map_application_api_key_not_found(error: anyhow::Error) -> ApiError {
    if error.to_string() == "application_api_key not found" {
        return ControlPlaneError::NotFound("application_api_key").into();
    }
    error.into()
}

async fn load_application_public_docs_context(
    state: &ApiState,
    headers: &HeaderMap,
    application_id: Uuid,
    query_locale: Option<String>,
) -> Result<ApplicationPublicDocsContext, ApiError> {
    let context = require_session(state, headers).await?;
    let locale = runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
        query_locale,
        explicit_header_locale: headers
            .get("x-1flowbase-locale")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        user_preferred_locale: context.user.preferred_locale.clone(),
        accept_language: headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        fallback_locale: runtime_profile::FALLBACK_LOCALE,
        supported_locales: runtime_profile::SUPPORTED_LOCALES
            .iter()
            .map(|value| value.to_string())
            .collect(),
    });
    let application = ApplicationService::new(state.store.clone())
        .get_application(context.user.id, application_id)
        .await?;
    let active_publication = ApplicationPublicationService::new(state.store.clone())
        .load_active_publication(LoadActiveApplicationPublicationCommand { application_id })
        .await
        .ok();

    Ok(ApplicationPublicDocsContext {
        application,
        active_publication,
        locale: locale.resolved_locale,
    })
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-keys",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses(
        (status = 200, body = [ApplicationApiKeyResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_api_keys(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Vec<ApplicationApiKeyResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let api_keys = ApplicationApiKeyService::new(state.store.clone())
        .list_api_keys(ListApplicationApiKeysCommand {
            actor_user_id: context.user.id,
            application_id,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        api_keys.into_iter().map(to_api_key_response).collect(),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{application_id}/api-keys",
    params(("application_id" = Uuid, Path, description = "Application id")),
    request_body = CreateApplicationApiKeyBody,
    responses(
        (status = 201, body = CreatedApplicationApiKeyResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_application_api_key(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<CreateApplicationApiKeyBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<CreatedApplicationApiKeyResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let result = ApplicationApiKeyService::new(state.store.clone())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: context.user.id,
            application_id,
            name: body.name,
            expires_at: parse_expires_at(body.expires_at)?,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_created_api_key_response(
            result.api_key,
            result.token,
        ))),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/console/applications/{application_id}/api-keys/{key_id}",
    params(
        ("application_id" = Uuid, Path, description = "Application id"),
        ("key_id" = Uuid, Path, description = "Application API key id")
    ),
    responses(
        (status = 204),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn revoke_application_api_key(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((application_id, key_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ApplicationApiKeyService::new(state.store.clone())
        .revoke_api_key(RevokeApplicationApiKeyCommand {
            actor_user_id: context.user.id,
            application_id,
            api_key_id: key_id,
        })
        .await
        .map_err(map_application_api_key_not_found)?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-mapping",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses(
        (status = 200, body = ApplicationApiMappingBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_api_mapping(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<ApplicationApiMappingBody>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let mapping = ApplicationApiMappingService::new(state.store.clone())
        .get_mapping(GetApplicationApiMappingCommand {
            actor_user_id: context.user.id,
            application_id,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_mapping_body(mapping))))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{application_id}/api-mapping",
    params(("application_id" = Uuid, Path, description = "Application id")),
    request_body = ApplicationApiMappingBody,
    responses(
        (status = 200, body = ApplicationApiMappingBody),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn replace_application_api_mapping(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<ApplicationApiMappingBody>,
) -> Result<Json<ApiSuccess<ApplicationApiMappingBody>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let mapping = ApplicationApiMappingService::new(state.store.clone())
        .replace_mapping(ReplaceApplicationApiMappingCommand {
            actor_user_id: context.user.id,
            application_id,
            mapping: to_mapping_config(body),
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_mapping_body(mapping))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-publication",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses(
        (status = 200, body = ApplicationPublicationResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_api_publication(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<ApplicationPublicationResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    ApplicationService::new(state.store.clone())
        .get_application(context.user.id, application_id)
        .await?;
    let publication = ApplicationPublicationService::new(state.store.clone())
        .load_active_publication(LoadActiveApplicationPublicationCommand { application_id })
        .await
        .map_err(map_publication_not_found)?;

    Ok(Json(ApiSuccess::new(to_publication_response(publication))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{application_id}/api-publications",
    params(("application_id" = Uuid, Path, description = "Application id")),
    request_body = PublishApplicationApiBody,
    responses(
        (status = 201, body = ApplicationPublicationResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn publish_application_api(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<PublishApplicationApiBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ApplicationPublicationResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let publication = ApplicationPublicationService::new(state.store.clone())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: context.user.id,
            application_id,
            mapping: to_mapping_config(body.mapping),
            api_enabled: body.api_enabled,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_publication_response(publication))),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/console/applications/{application_id}/api-publication",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses(
        (status = 204),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn unpublish_application_api(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ApplicationPublicationService::new(state.store.clone())
        .unpublish(UnpublishApplicationCommand {
            actor_user_id: context.user.id,
            application_id,
        })
        .await
        .map_err(map_publication_not_found)?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    patch,
    path = "/api/console/applications/{application_id}/api-status",
    params(("application_id" = Uuid, Path, description = "Application id")),
    request_body = PatchApplicationApiStatusBody,
    responses(
        (status = 200, body = ApplicationApiStatusResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn patch_application_api_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<PatchApplicationApiStatusBody>,
) -> Result<Json<ApiSuccess<ApplicationApiStatusResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    ApplicationPublicationService::new(state.store.clone())
        .set_api_enabled(SetApplicationApiEnabledCommand {
            actor_user_id: context.user.id,
            application_id,
            api_enabled: body.api_enabled,
        })
        .await?;

    Ok(Json(ApiSuccess::new(ApplicationApiStatusResponse {
        application_id,
        api_enabled: body.api_enabled,
        public_url: PUBLIC_RUNS_PATH.to_string(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/workflow-schedule-trigger",
    params(("application_id" = Uuid, Path, description = "Application id")),
    responses(
        (status = 200, body = Option<WorkflowScheduleTriggerResponse>),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_workflow_schedule_trigger(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Option<WorkflowScheduleTriggerResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let trigger = WorkflowScheduleTriggerService::new(state.store.clone())
        .get_trigger(GetWorkflowScheduleTriggerCommand {
            actor_user_id: context.user.id,
            application_id,
        })
        .await?
        .map(to_workflow_schedule_trigger_response);

    Ok(Json(ApiSuccess::new(trigger)))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{application_id}/workflow-schedule-trigger",
    params(("application_id" = Uuid, Path, description = "Application id")),
    request_body = WorkflowScheduleTriggerBody,
    responses(
        (status = 200, body = WorkflowScheduleTriggerResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn replace_workflow_schedule_trigger(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
    Json(body): Json<WorkflowScheduleTriggerBody>,
) -> Result<Json<ApiSuccess<WorkflowScheduleTriggerResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let trigger = WorkflowScheduleTriggerService::new(state.store.clone())
        .replace_trigger(ReplaceWorkflowScheduleTriggerCommand {
            actor_user_id: context.user.id,
            application_id,
            enabled: body.enabled,
            cron: body.cron,
            timezone: body.timezone,
            input_payload: body.input_payload,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        to_workflow_schedule_trigger_response(trigger),
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-docs/catalog",
    params(
        ("application_id" = Uuid, Path, description = "Application id"),
        ("locale" = Option<String>, Query, description = "Requested docs locale")
    ),
    responses(
        (status = 200, body = DocsCatalog),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_api_docs_catalog(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ApplicationApiDocsQuery>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<Json<ApiSuccess<DocsCatalog>>, ApiError> {
    let context =
        load_application_public_docs_context(&state, &headers, application_id, query.locale)
            .await?;

    Ok(Json(ApiSuccess::new(
        build_application_public_docs_catalog(&context),
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-docs/categories/{category_id}/operations",
    params(
        ("application_id" = Uuid, Path, description = "Application id"),
        ("category_id" = String, Path, description = "Application public API docs category id"),
        ("locale" = Option<String>, Query, description = "Requested docs locale"),
        ("offset" = Option<usize>, Query, description = "Operations page offset"),
        ("limit" = Option<usize>, Query, description = "Operations page size, max 20"),
        ("q" = Option<String>, Query, description = "Operation search query")
    ),
    responses(
        (status = 200, body = DocsCatalogCategoryOperationsPage),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_api_docs_category_operations(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ApplicationApiDocsQuery>,
    headers: HeaderMap,
    Path((application_id, category_id)): Path<(Uuid, String)>,
) -> Result<Json<ApiSuccess<DocsCatalogCategoryOperationsPage>>, ApiError> {
    let context = load_application_public_docs_context(
        &state,
        &headers,
        application_id,
        query.locale.clone(),
    )
    .await?;
    let operations = build_application_public_docs_category_operations(&context, &category_id)
        .ok_or(ControlPlaneError::NotFound("application_api_docs_category"))?;
    let filtered_operations = filter_category_operations(&operations, query.search_query());

    Ok(Json(ApiSuccess::new(paginate_category_operations(
        &filtered_operations,
        query.offset(),
        query.limit(),
    ))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-docs/categories/{category_id}/openapi.json",
    params(
        ("application_id" = Uuid, Path, description = "Application id"),
        ("category_id" = String, Path, description = "Application public API docs category id"),
        ("locale" = Option<String>, Query, description = "Requested docs locale")
    ),
    responses(
        (status = 200, body = Value),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_api_docs_category_openapi(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ApplicationApiDocsQuery>,
    headers: HeaderMap,
    Path((application_id, category_id)): Path<(Uuid, String)>,
) -> Result<Json<Value>, ApiError> {
    let context =
        load_application_public_docs_context(&state, &headers, application_id, query.locale)
            .await?;
    let spec = build_application_public_docs_category_spec(&context, &category_id)
        .ok_or(ControlPlaneError::NotFound("application_api_docs_category"))?;

    Ok(Json(spec))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-docs/operations/{operation_id}/openapi.json",
    params(
        ("application_id" = Uuid, Path, description = "Application id"),
        ("operation_id" = String, Path, description = "Application public API docs operation id"),
        ("locale" = Option<String>, Query, description = "Requested docs locale")
    ),
    responses(
        (status = 200, body = Value),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_api_docs_operation_openapi(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ApplicationApiDocsQuery>,
    headers: HeaderMap,
    Path((application_id, operation_id)): Path<(Uuid, String)>,
) -> Result<Json<Value>, ApiError> {
    let context =
        load_application_public_docs_context(&state, &headers, application_id, query.locale)
            .await?;
    let spec = build_application_public_docs_operation_spec(&context, &operation_id).ok_or(
        ControlPlaneError::NotFound("application_api_docs_operation"),
    )?;

    Ok(Json(spec))
}
