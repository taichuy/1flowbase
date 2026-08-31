use std::sync::Arc;

use access_control::{
    APPLICATIONS_API_SET_ENABLED_OPERATION_ID, APPLICATIONS_PUBLISH_OPERATION_ID,
    APPLICATIONS_UPDATE_OPERATION_ID, APPLICATIONS_VIEW_OPERATION_ID,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json, Router,
};
use control_plane::{
    application_public_api::{
        mapping::{
            ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
            WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode,
        },
        publications::ApplicationPublicationVersionRecord,
        published_workflow_operation::PublishedWorkflowOperation,
        workflow_schedule::WorkflowScheduleTriggerRecord,
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
    error_response::ApiError,
    openapi_docs::{DocsCatalog, DocsCatalogCategoryOperationsPage, DOCS_OPERATIONS_PAGE_SIZE},
    response::ApiSuccess,
    routes::console_route_assembly::{
        console_delete, console_get, console_patch, console_post, ConsoleRouteAssembly,
    },
};

pub(crate) mod interface_docs;
pub(crate) mod interface_keys;
pub(crate) mod interface_publication;
pub(crate) mod interface_schedule;

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
#[serde(deny_unknown_fields)]
pub struct WorkflowExtensionApiBody {
    /// Route template below /api/ex/ without a leading slash. Workflow Start path-source keys must
    /// match every {placeholder} exactly.
    pub slug: String,
    /// HTTP method accepted by the published extension interface.
    pub method: WorkflowExtensionHttpMethodBody,
    /// sync waits for a result up to Workflow Start sync_timeout_ms; async returns an accepted run.
    pub response_mode: WorkflowExtensionResponseModeBody,
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
    /// Agent Flow request/output mapping or Workflow extension publication mapping.
    pub mapping: ApplicationApiMappingBody,
    /// Enables invocation for the new active publication. Workflow extension invocation uses the
    /// normal /api/ex/{slug} interface and never requires an Application API Key.
    pub api_enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchApplicationApiStatusBody {
    pub api_enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct WorkflowScheduleTriggerBody {
    /// Desired scheduler state. This PUT resource changes configuration and enabled state together.
    pub enabled: bool,
    /// Standard cron expression.
    pub cron: String,
    /// IANA timezone used to evaluate cron.
    pub timezone: String,
    /// Defaults keyed by Workflow Start input field key. HTTP source is irrelevant for schedules.
    pub input_payload: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkflowScheduleTriggerResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    /// False for schedule configuration created with a new Application until explicitly enabled by
    /// PUT /workflow-schedule-trigger.
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
    /// For Workflow extension publications, the Agent-readable normal HTTP operation under
    /// /api/ex/{slug}; absent for Agent Flow and schedule-triggered Workflow publications.
    #[schema(inline)]
    pub operation: Option<PublishedWorkflowOperationResponse>,
    pub dependency_snapshot: Vec<ApplicationPublicationJsDependencySnapshotResponse>,
    /// /api/ex/{slug} for extension publications; /api/agent/v1/runs for Agent Flow.
    pub public_url: String,
    pub created_by: Uuid,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PublishedWorkflowOperationResponse {
    pub interface_id: String,
    pub method: WorkflowExtensionHttpMethodBody,
    /// Relative route template appended to the normal /api/ex/ interface prefix.
    pub route_template: String,
    pub response_mode: WorkflowExtensionResponseModeBody,
    #[schema(value_type = Object)]
    pub parameter_schema: Value,
    #[schema(value_type = Object)]
    pub result_schema: Value,
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
    }
}

fn to_publication_response(
    publication: ApplicationPublicationVersionRecord,
) -> ApplicationPublicationResponse {
    let operation = PublishedWorkflowOperation::from_publication(publication.clone())
        .ok()
        .map(to_published_workflow_operation_response);
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
        operation,
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

fn to_published_workflow_operation_response(
    operation: PublishedWorkflowOperation,
) -> PublishedWorkflowOperationResponse {
    let extension = operation
        .publication
        .mapping_snapshot
        .extension
        .expect("published workflow operation must have extension config");
    PublishedWorkflowOperationResponse {
        interface_id: operation.interface_id,
        method: to_extension_body(extension.clone()).method,
        route_template: operation.route_template,
        response_mode: to_extension_body(extension).response_mode,
        parameter_schema: operation.parameter_schema,
        result_schema: operation.result_schema,
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
    let output: interface_keys::ApplicationApiKeyOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.api-keys.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_keys::ApplicationApiKeyInput::List { application_id },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.into_list()?)))
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
    let output: interface_keys::ApplicationApiKeyOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.api-keys.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface_keys::ApplicationApiKeyInput::Create {
            application_id,
            body,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(output.into_created()?)),
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
    crate::routes::console_interface::invoke::<_, interface_keys::ApplicationApiKeyOutput>(
        Arc::clone(&state),
        "http.console.applications.api-keys.revoke.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface_keys::ApplicationApiKeyInput::Revoke {
            application_id,
            key_id,
        },
    )
    .await?;

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
    let output: interface_publication::ApplicationPublicationOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.api-mapping.get.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
            interface_publication::ApplicationPublicationInput::GetMapping { application_id },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.into_mapping()?)))
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
    let output: interface_publication::ApplicationPublicationOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.api-mapping.replace.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            },
            interface_publication::ApplicationPublicationInput::ReplaceMapping {
                application_id,
                body,
            },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.into_mapping()?)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/api-publication",
    summary = "Get the active Application publication",
    description = "Returns the active publication and its invocation contract. Workflow extension publications expose a normal /api/ex/{slug} interface without an Application API Key.",
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
    let output: interface_publication::ApplicationPublicationOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.api-publication.get.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
            interface_publication::ApplicationPublicationInput::GetPublication { application_id },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.into_publication()?)))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{application_id}/api-publications",
    summary = "Publish the active Application version",
    description = "Publishes the current flow version. Agent Flow uses the native run/API Gateway surface; an extension-triggered Workflow publishes a normal /api/ex/{slug} interface that does not require an Application API Key.",
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
    let output: interface_publication::ApplicationPublicationOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.api-publication.publish.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            },
            interface_publication::ApplicationPublicationInput::Publish {
                application_id,
                body,
            },
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(output.into_publication()?)),
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
    crate::routes::console_interface::invoke::<
        _,
        interface_publication::ApplicationPublicationOutput,
    >(
        Arc::clone(&state),
        "http.console.applications.api-publication.unpublish.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface_publication::ApplicationPublicationInput::Unpublish { application_id },
    )
    .await?;

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
    let output: interface_publication::ApplicationPublicationOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.api-status.update.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            },
            interface_publication::ApplicationPublicationInput::SetEnabled {
                application_id,
                body,
            },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.into_status()?)))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{application_id}/workflow-schedule-trigger",
    summary = "Get Workflow schedule configuration and enabled state",
    description = "Returns the schedule resource for a schedule-triggered Workflow. A schedule created during Application creation starts disabled.",
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
    let output: interface_schedule::WorkflowScheduleOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.workflow-schedule.get.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
            interface_schedule::WorkflowScheduleInput::Get { application_id },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.into_optional()?)))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{application_id}/workflow-schedule-trigger",
    summary = "Replace Workflow schedule configuration and enabled state",
    description = "Atomically replaces cron, timezone, Workflow Start input defaults, and the desired enabled state for a schedule-triggered Workflow.",
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
    let output: interface_schedule::WorkflowScheduleOutput =
        crate::routes::console_interface::invoke(
            Arc::clone(&state),
            "http.console.applications.workflow-schedule.replace.v1",
            crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
                state,
                headers,
            },
            interface_schedule::WorkflowScheduleInput::Replace {
                application_id,
                body,
            },
        )
        .await?;
    Ok(Json(ApiSuccess::new(output.into_required()?)))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output: interface_docs::ApplicationDocsOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.api-docs.catalog.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_docs::ApplicationDocsInput::Catalog {
            application_id,
            query_locale: query.locale,
            locale,
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.into_catalog()?)))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output: interface_docs::ApplicationDocsOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.api-docs.category-operations.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_docs::ApplicationDocsInput::CategoryOperations {
            application_id,
            category_id,
            query,
            locale,
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output.into_category_operations()?)))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output: interface_docs::ApplicationDocsOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.api-docs.category-openapi.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_docs::ApplicationDocsInput::CategoryOpenApi {
            application_id,
            category_id,
            query_locale: query.locale,
            locale,
        },
    )
    .await?;
    Ok(Json(output.into_openapi()?))
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
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output: interface_docs::ApplicationDocsOutput = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.applications.api-docs.operation-openapi.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface_docs::ApplicationDocsInput::OperationOpenApi {
            application_id,
            operation_id,
            query_locale: query.locale,
            locale,
        },
    )
    .await?;
    Ok(Json(output.into_openapi()?))
}
