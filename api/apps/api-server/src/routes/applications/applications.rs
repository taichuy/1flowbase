use std::sync::Arc;

use access_control::{
    APPLICATIONS_CREATE_OPERATION_ID, APPLICATIONS_DELETE_OPERATION_ID,
    APPLICATIONS_UPDATE_OPERATION_ID, APPLICATIONS_VIEW_OPERATION_ID,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::{
    application::{
        ApplicationService, CreateApplicationCommand, CreateApplicationTagCommand,
        DeleteApplicationCommand, ReplaceApplicationEnvironmentVariablesCommand,
        UpdateApplicationCommand,
    },
    errors::ControlPlaneError,
    js_dependency::{
        ApplicationJsDependencyService, ReplaceApplicationJsDependencySelectionCommand,
    },
    ports::{ApplicationEnvironmentVariableInput, CreateWorkflowTriggerConfig},
};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApplicationBody {
    pub application_type: String,
    pub workflow_trigger_type: Option<String>,
    pub workflow_trigger_config: Option<CreateWorkflowTriggerConfigBody>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkflowTriggerConfigBody {
    pub cron: Option<String>,
    pub timezone: Option<String>,
    pub input_payload: Option<serde_json::Value>,
    pub subpath: Option<String>,
    pub http_method: Option<String>,
    pub response_mode: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PatchApplicationBody {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApplicationTagBody {
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplicationEnvironmentVariableBody {
    pub name: String,
    pub value_type: String,
    pub value: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceApplicationEnvironmentVariablesBody {
    pub variables: Vec<ApplicationEnvironmentVariableBody>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReplaceApplicationJsDependencySelectionBody {
    pub installation_id: String,
    pub alias: String,
    pub target: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationTagResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationEnvironmentVariableResponse {
    pub name: String,
    pub value_type: String,
    pub value: serde_json::Value,
    pub description: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationJsDependencyPermissionsResponse {
    pub network: String,
    pub filesystem: String,
    pub env: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationJsDependencySelectionResponse {
    pub application_id: String,
    pub installation_id: String,
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
    pub permissions: ApplicationJsDependencyPermissionsResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationTagCatalogResponse {
    pub id: String,
    pub name: String,
    pub application_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationTypeOptionResponse {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationCatalogResponse {
    pub types: Vec<ApplicationTypeOptionResponse>,
    pub tags: Vec<ApplicationTagCatalogResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationSummaryResponse {
    pub id: String,
    pub application_type: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: String,
    pub updated_at: String,
    pub tags: Vec<ApplicationTagResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationOrchestrationSectionResponse {
    pub status: String,
    pub subject_kind: String,
    pub subject_status: String,
    pub current_subject_id: Option<String>,
    pub current_draft_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationApiSectionStatusResponse {
    Active,
    Planned,
    Available,
    Unavailable,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationApiCredentialKindResponse {
    ApplicationApiKey,
    UserOrPublic,
    NotApplicable,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationApiInvokeRoutingModeResponse {
    ApiKeyBoundApplication,
    PublishedWorkflowOperation,
    NotAvailable,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationApiCapabilityStatusResponse {
    Enabled,
    Disabled,
    NotPublished,
    Available,
    Unavailable,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationApiCredentialsStatusResponse {
    Configured,
    Missing,
    NotRequired,
    NotApplicable,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationApiSectionResponse {
    #[schema(inline)]
    pub status: ApplicationApiSectionStatusResponse,
    #[schema(inline)]
    pub credential_kind: ApplicationApiCredentialKindResponse,
    #[schema(inline)]
    pub invoke_routing_mode: ApplicationApiInvokeRoutingModeResponse,
    pub invoke_path_template: Option<String>,
    #[schema(inline)]
    pub api_capability_status: ApplicationApiCapabilityStatusResponse,
    #[schema(inline)]
    pub credentials_status: ApplicationApiCredentialsStatusResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationLogsSectionResponse {
    pub status: String,
    pub runs_capability_status: String,
    pub run_object_kind: String,
    pub log_retention_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationMonitoringSectionResponse {
    pub status: String,
    pub metrics_capability_status: String,
    pub metrics_object_kind: String,
    pub tracing_config_status: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationSectionsResponse {
    pub orchestration: ApplicationOrchestrationSectionResponse,
    pub api: ApplicationApiSectionResponse,
    pub logs: ApplicationLogsSectionResponse,
    pub monitoring: ApplicationMonitoringSectionResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplicationDetailResponse {
    pub id: String,
    pub application_type: String,
    pub workflow_trigger_type: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: String,
    pub updated_at: String,
    pub tags: Vec<ApplicationTagResponse>,
    pub sections: ApplicationSectionsResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/applications",
            console_get(
                list_applications,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .post(
                create_application,
                ConsoleOperation(APPLICATIONS_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id",
            console_get(
                get_application,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .patch(
                patch_application,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            )
            .delete(
                delete_application,
                ConsoleOperation(APPLICATIONS_DELETE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/catalog",
            console_get(
                get_application_catalog,
                ConsoleOperation(APPLICATIONS_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/tags",
            console_post(
                create_application_tag,
                ConsoleOperation(APPLICATIONS_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/environment-variables",
            console_get(
                list_application_environment_variables,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .put(
                replace_application_environment_variables,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/applications/:id/js-dependencies",
            console_get(
                list_application_js_dependency_selections,
                ConsoleOperation(APPLICATIONS_VIEW_OPERATION_ID.to_string()),
            )
            .put(
                replace_application_js_dependency_selection,
                ConsoleOperation(APPLICATIONS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
}

fn to_application_tag(tag: domain::ApplicationTag) -> ApplicationTagResponse {
    ApplicationTagResponse {
        id: tag.id.to_string(),
        name: tag.name,
    }
}

fn to_application_tag_catalog_entry(
    tag: domain::ApplicationTagCatalogEntry,
) -> ApplicationTagCatalogResponse {
    ApplicationTagCatalogResponse {
        id: tag.id.to_string(),
        name: tag.name,
        application_count: tag.application_count,
    }
}

fn to_application_environment_variable(
    variable: domain::ApplicationEnvironmentVariable,
) -> ApplicationEnvironmentVariableResponse {
    ApplicationEnvironmentVariableResponse {
        name: variable.name,
        value_type: variable.value_type,
        value: variable.value,
        description: variable.description,
        updated_at: variable.updated_at.format(&Rfc3339).unwrap(),
    }
}

fn to_application_js_dependency_selection(
    selection: domain::ApplicationJsDependencySelection,
) -> ApplicationJsDependencySelectionResponse {
    ApplicationJsDependencySelectionResponse {
        application_id: selection.application_id.to_string(),
        installation_id: selection.installation_id.to_string(),
        provider_code: selection.provider_code,
        plugin_id: selection.plugin_id,
        plugin_version: selection.plugin_version,
        alias: selection.alias,
        package: selection.package,
        version: selection.version,
        target: selection.target,
        artifact_path: selection.artifact_path,
        artifact_hash: selection.artifact_hash,
        integrity: selection.integrity,
        permissions: ApplicationJsDependencyPermissionsResponse {
            network: selection.permissions.network,
            filesystem: selection.permissions.filesystem,
            env: selection.permissions.env,
        },
    }
}

fn application_type_catalog() -> Vec<ApplicationTypeOptionResponse> {
    vec![
        ApplicationTypeOptionResponse {
            value: "agent_flow".to_string(),
            label: "AgentFlow".to_string(),
        },
        ApplicationTypeOptionResponse {
            value: "workflow".to_string(),
            label: "工作流".to_string(),
        },
    ]
}

fn to_application_summary(application: domain::ApplicationRecord) -> ApplicationSummaryResponse {
    ApplicationSummaryResponse {
        id: application.id.to_string(),
        application_type: application.application_type.as_str().to_string(),
        name: application.name,
        description: application.description,
        icon: application.icon,
        icon_type: application.icon_type,
        icon_background: application.icon_background,
        created_by: application.created_by.to_string(),
        updated_at: application.updated_at.format(&Rfc3339).unwrap(),
        tags: application
            .tags
            .into_iter()
            .map(to_application_tag)
            .collect(),
    }
}

fn to_sections_response(sections: domain::ApplicationSections) -> ApplicationSectionsResponse {
    ApplicationSectionsResponse {
        orchestration: ApplicationOrchestrationSectionResponse {
            status: sections.orchestration.status,
            subject_kind: sections.orchestration.subject_kind,
            subject_status: sections.orchestration.subject_status,
            current_subject_id: sections
                .orchestration
                .current_subject_id
                .map(|value| value.to_string()),
            current_draft_id: sections
                .orchestration
                .current_draft_id
                .map(|value| value.to_string()),
        },
        api: ApplicationApiSectionResponse {
            status: api_section_status(&sections.api.status),
            credential_kind: api_credential_kind(&sections.api.credential_kind),
            invoke_routing_mode: api_invoke_routing_mode(&sections.api.invoke_routing_mode),
            invoke_path_template: sections.api.invoke_path_template,
            api_capability_status: api_capability_status(&sections.api.api_capability_status),
            credentials_status: api_credentials_status(&sections.api.credentials_status),
        },
        logs: ApplicationLogsSectionResponse {
            status: sections.logs.status,
            runs_capability_status: sections.logs.runs_capability_status,
            run_object_kind: sections.logs.run_object_kind,
            log_retention_status: sections.logs.log_retention_status,
        },
        monitoring: ApplicationMonitoringSectionResponse {
            status: sections.monitoring.status,
            metrics_capability_status: sections.monitoring.metrics_capability_status,
            metrics_object_kind: sections.monitoring.metrics_object_kind,
            tracing_config_status: sections.monitoring.tracing_config_status,
        },
    }
}

fn api_section_status(value: &str) -> ApplicationApiSectionStatusResponse {
    match value {
        "active" => ApplicationApiSectionStatusResponse::Active,
        "planned" => ApplicationApiSectionStatusResponse::Planned,
        "available" => ApplicationApiSectionStatusResponse::Available,
        _ => ApplicationApiSectionStatusResponse::Unavailable,
    }
}

fn api_credential_kind(value: &str) -> ApplicationApiCredentialKindResponse {
    match value {
        "application_api_key" => ApplicationApiCredentialKindResponse::ApplicationApiKey,
        "user_or_public" => ApplicationApiCredentialKindResponse::UserOrPublic,
        _ => ApplicationApiCredentialKindResponse::NotApplicable,
    }
}

fn api_invoke_routing_mode(value: &str) -> ApplicationApiInvokeRoutingModeResponse {
    match value {
        "api_key_bound_application" => {
            ApplicationApiInvokeRoutingModeResponse::ApiKeyBoundApplication
        }
        "published_workflow_operation" => {
            ApplicationApiInvokeRoutingModeResponse::PublishedWorkflowOperation
        }
        _ => ApplicationApiInvokeRoutingModeResponse::NotAvailable,
    }
}

fn api_capability_status(value: &str) -> ApplicationApiCapabilityStatusResponse {
    match value {
        "enabled" => ApplicationApiCapabilityStatusResponse::Enabled,
        "disabled" => ApplicationApiCapabilityStatusResponse::Disabled,
        "not_published" => ApplicationApiCapabilityStatusResponse::NotPublished,
        "available" => ApplicationApiCapabilityStatusResponse::Available,
        _ => ApplicationApiCapabilityStatusResponse::Unavailable,
    }
}

fn api_credentials_status(value: &str) -> ApplicationApiCredentialsStatusResponse {
    match value {
        "configured" => ApplicationApiCredentialsStatusResponse::Configured,
        "missing" => ApplicationApiCredentialsStatusResponse::Missing,
        "not_required" => ApplicationApiCredentialsStatusResponse::NotRequired,
        _ => ApplicationApiCredentialsStatusResponse::NotApplicable,
    }
}

fn to_application_detail(application: domain::ApplicationRecord) -> ApplicationDetailResponse {
    ApplicationDetailResponse {
        id: application.id.to_string(),
        application_type: application.application_type.as_str().to_string(),
        workflow_trigger_type: application
            .workflow_trigger_type
            .map(|value| value.as_str().to_string()),
        name: application.name,
        description: application.description,
        icon: application.icon,
        icon_type: application.icon_type,
        icon_background: application.icon_background,
        created_by: application.created_by.to_string(),
        updated_at: application.updated_at.format(&Rfc3339).unwrap(),
        tags: application
            .tags
            .into_iter()
            .map(to_application_tag)
            .collect(),
        sections: to_sections_response(application.sections),
    }
}

fn parse_application_type(value: &str) -> Result<domain::ApplicationType, ApiError> {
    match value {
        "agent_flow" => Ok(domain::ApplicationType::AgentFlow),
        "workflow" => Ok(domain::ApplicationType::Workflow),
        _ => Err(ControlPlaneError::InvalidInput("application_type").into()),
    }
}

fn parse_create_workflow_trigger_config(
    trigger_type: Option<domain::WorkflowTriggerType>,
    config: Option<CreateWorkflowTriggerConfigBody>,
) -> Result<Option<CreateWorkflowTriggerConfig>, ApiError> {
    match (trigger_type, config) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ControlPlaneError::InvalidInput("workflow_trigger_config").into()),
        (Some(domain::WorkflowTriggerType::Schedule), Some(config)) => {
            let cron = config
                .cron
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("cron"))?;
            let timezone = config
                .timezone
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("timezone"))?;
            Ok(Some(CreateWorkflowTriggerConfig::Schedule {
                cron,
                timezone,
                input_payload: config
                    .input_payload
                    .unwrap_or_else(|| serde_json::json!({})),
            }))
        }
        (Some(domain::WorkflowTriggerType::Extension), Some(config)) => {
            let subpath = config
                .subpath
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("subpath"))?;
            let http_method = config.http_method.unwrap_or_else(|| "POST".to_string());
            let response_mode = config.response_mode.unwrap_or_else(|| "sync".to_string());
            Ok(Some(CreateWorkflowTriggerConfig::Extension {
                subpath,
                http_method,
                response_mode,
            }))
        }
        (Some(_), None) => Ok(None),
    }
}

fn parse_workflow_trigger_type(
    application_type: domain::ApplicationType,
    value: Option<&str>,
) -> Result<Option<domain::WorkflowTriggerType>, ApiError> {
    match application_type {
        domain::ApplicationType::AgentFlow => Ok(None),
        domain::ApplicationType::Workflow => {
            let raw = value.unwrap_or("extension");
            domain::WorkflowTriggerType::parse(raw)
                .map(Some)
                .ok_or_else(|| ControlPlaneError::InvalidInput("workflow_trigger_type").into())
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/console/applications",
    responses(
        (status = 200, body = [ApplicationSummaryResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_applications(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<ApplicationSummaryResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let applications = ApplicationService::new(state.store.clone())
        .list_applications(context.user.id)
        .await?;

    Ok(Json(ApiSuccess::new(
        applications
            .into_iter()
            .map(to_application_summary)
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/catalog",
    responses(
        (status = 200, body = ApplicationCatalogResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ApplicationCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let tags = ApplicationService::new(state.store.clone())
        .list_application_tags(context.user.id)
        .await?;

    Ok(Json(ApiSuccess::new(ApplicationCatalogResponse {
        types: application_type_catalog(),
        tags: tags
            .into_iter()
            .map(to_application_tag_catalog_entry)
            .collect(),
    })))
}

#[utoipa::path(
    post,
    path = "/api/console/applications",
    request_body = CreateApplicationBody,
    responses(
        (status = 201, body = ApplicationDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_application(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateApplicationBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ApplicationDetailResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let application_type = parse_application_type(&body.application_type)?;
    let workflow_trigger_type =
        parse_workflow_trigger_type(application_type, body.workflow_trigger_type.as_deref())?;
    let created = ApplicationService::new(state.store.clone())
        .create_application(CreateApplicationCommand {
            actor_user_id: context.user.id,
            application_type,
            workflow_trigger_type,
            workflow_trigger_config: parse_create_workflow_trigger_config(
                workflow_trigger_type,
                body.workflow_trigger_config,
            )?,
            name: body.name,
            description: body.description,
            icon: body.icon,
            icon_type: body.icon_type,
            icon_background: body.icon_background,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_application_detail(created))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/tags",
    request_body = CreateApplicationTagBody,
    responses(
        (status = 201, body = ApplicationTagCatalogResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn create_application_tag(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateApplicationTagBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ApplicationTagCatalogResponse>>), ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let created = ApplicationService::new(state.store.clone())
        .create_application_tag(CreateApplicationTagCommand {
            actor_user_id: context.user.id,
            name: body.name,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_application_tag_catalog_entry(created))),
    ))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = ApplicationDetailResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_application(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<ApplicationDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let application = ApplicationService::new(state.store.clone())
        .get_application(context.user.id, id)
        .await?;

    Ok(Json(ApiSuccess::new(to_application_detail(application))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/environment-variables",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = [ApplicationEnvironmentVariableResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_environment_variables(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Vec<ApplicationEnvironmentVariableResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let variables = ApplicationService::new(state.store.clone())
        .list_application_environment_variables(context.user.id, id)
        .await?;

    Ok(Json(ApiSuccess::new(
        variables
            .into_iter()
            .map(to_application_environment_variable)
            .collect(),
    )))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{id}/environment-variables",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    request_body = ReplaceApplicationEnvironmentVariablesBody,
    responses(
        (status = 200, body = [ApplicationEnvironmentVariableResponse]),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn replace_application_environment_variables(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ReplaceApplicationEnvironmentVariablesBody>,
) -> Result<Json<ApiSuccess<Vec<ApplicationEnvironmentVariableResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let variables = body
        .variables
        .into_iter()
        .map(|variable| ApplicationEnvironmentVariableInput {
            name: variable.name,
            value_type: variable.value_type,
            value: variable.value,
            description: variable.description,
        })
        .collect();
    let replaced = ApplicationService::new(state.store.clone())
        .replace_application_environment_variables(ReplaceApplicationEnvironmentVariablesCommand {
            actor_user_id: context.user.id,
            application_id: id,
            variables,
        })
        .await?;

    Ok(Json(ApiSuccess::new(
        replaced
            .into_iter()
            .map(to_application_environment_variable)
            .collect(),
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/js-dependencies",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = [ApplicationJsDependencySelectionResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_application_js_dependency_selections(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<Vec<ApplicationJsDependencySelectionResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let selections = ApplicationJsDependencyService::new(state.store.clone())
        .list_application_js_dependency_selections(context.user.id, id)
        .await?;

    Ok(Json(ApiSuccess::new(
        selections
            .into_iter()
            .map(to_application_js_dependency_selection)
            .collect(),
    )))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{id}/js-dependencies",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    request_body = ReplaceApplicationJsDependencySelectionBody,
    responses(
        (status = 200, body = ApplicationJsDependencySelectionResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn replace_application_js_dependency_selection(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ReplaceApplicationJsDependencySelectionBody>,
) -> Result<Json<ApiSuccess<ApplicationJsDependencySelectionResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let installation_id = body
        .installation_id
        .parse::<Uuid>()
        .map_err(|_| ControlPlaneError::InvalidInput("installation_id"))?;

    let selection = ApplicationJsDependencyService::new(state.store.clone())
        .replace_application_js_dependency_selection(
            ReplaceApplicationJsDependencySelectionCommand {
                actor_user_id: context.user.id,
                application_id: id,
                installation_id,
                alias: body.alias,
                target: body.target,
            },
        )
        .await?;

    Ok(Json(ApiSuccess::new(
        to_application_js_dependency_selection(selection),
    )))
}

#[utoipa::path(
    patch,
    path = "/api/console/applications/{id}",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    request_body = PatchApplicationBody,
    responses(
        (status = 200, body = ApplicationDetailResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn patch_application(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<PatchApplicationBody>,
) -> Result<Json<ApiSuccess<ApplicationDetailResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    let tag_ids = body
        .tag_ids
        .into_iter()
        .map(|value| {
            value
                .parse::<Uuid>()
                .map_err(|_| ControlPlaneError::InvalidInput("tag_ids"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let updated = ApplicationService::new(state.store.clone())
        .update_application(UpdateApplicationCommand {
            actor_user_id: context.user.id,
            application_id: id,
            name: body.name,
            description: body.description,
            tag_ids,
            icon: body.icon,
            icon_type: body.icon_type,
            icon_background: body.icon_background,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_application_detail(updated))))
}

#[utoipa::path(
    delete,
    path = "/api/console/applications/{id}",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 204),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn delete_application(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;

    ApplicationService::new(state.store.clone())
        .delete_application(DeleteApplicationCommand {
            actor_user_id: context.user.id,
            application_id: id,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
