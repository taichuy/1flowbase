use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, patch, post, put},
    Json, Router,
};
use control_plane::{
    errors::ControlPlaneError,
    flow::{
        AgentFlowTemplateDependency, AgentFlowTemplateDependencyStatus, AgentFlowTemplatePackage,
        AgentFlowTemplatePreview, AgentFlowTemplateUnresolvedNode, FlowService,
        ImportAgentFlowTemplateCommand, ImportAgentFlowTemplateResult,
        PreviewAgentFlowTemplateCommand, SaveFlowDraftCommand, UpdateFlowVersionMetadataCommand,
    },
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
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveDraftBody {
    pub document: serde_json::Value,
    pub change_kind: String,
    pub summary: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateVersionBody {
    pub summary: Option<String>,
    pub summary_is_custom: Option<bool>,
    pub is_protected: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AgentFlowTemplatePreviewBody {
    #[schema(value_type = Object)]
    pub template: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportAgentFlowTemplateBody {
    #[schema(value_type = Object)]
    pub template: serde_json::Value,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowVersionResponse {
    pub id: String,
    pub sequence: i64,
    pub trigger: String,
    pub change_kind: String,
    pub summary: String,
    pub summary_is_custom: bool,
    pub is_protected: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlowDraftResponse {
    pub id: String,
    pub flow_id: String,
    pub document: serde_json::Value,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OrchestrationStateResponse {
    pub flow_id: String,
    pub draft: FlowDraftResponse,
    pub versions: Vec<FlowVersionResponse>,
    pub autosave_interval_seconds: u16,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateApplicationResponse {
    pub application_type: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateDependencyResponse {
    pub kind: String,
    pub node_id: Option<String>,
    pub node_type: Option<String>,
    pub config_version: Option<i64>,
    pub provider_code: Option<String>,
    pub model_id: Option<String>,
    pub plugin_id: Option<String>,
    pub plugin_version: Option<String>,
    pub contribution_code: Option<String>,
    pub node_shell: Option<String>,
    pub schema_version: Option<String>,
    pub plugin_unique_identifier: Option<String>,
    pub package_id: Option<String>,
    pub contribution_checksum: Option<String>,
    pub compiled_contribution_hash: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplatePackageResponse {
    pub schema_version: String,
    pub application: AgentFlowTemplateApplicationResponse,
    #[schema(value_type = Object)]
    pub flow_document: serde_json::Value,
    pub dependencies: Vec<AgentFlowTemplateDependencyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateDependencyStatusResponse {
    pub dependency: AgentFlowTemplateDependencyResponse,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateUnresolvedNodeResponse {
    pub node_id: String,
    pub alias: String,
    pub original_type: String,
    pub dependency_status: String,
    pub reason: String,
    #[schema(value_type = Object)]
    pub original_node: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplatePreviewResponse {
    pub schema_version: String,
    pub application: AgentFlowTemplateApplicationResponse,
    pub dependencies: Vec<AgentFlowTemplateDependencyStatusResponse>,
    pub unresolved_nodes: Vec<AgentFlowTemplateUnresolvedNodeResponse>,
    #[schema(value_type = Object)]
    pub document: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AgentFlowTemplateImportedApplicationResponse {
    pub id: String,
    pub application_type: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportAgentFlowTemplateResponse {
    pub application: AgentFlowTemplateImportedApplicationResponse,
    pub orchestration: OrchestrationStateResponse,
    pub preview: AgentFlowTemplatePreviewResponse,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/applications/:id/orchestration", get(get_orchestration))
        .route("/applications/:id/orchestration/draft", put(save_draft))
        .route(
            "/applications/:id/orchestration/template",
            get(export_agent_flow_template),
        )
        .route(
            "/applications/orchestration/template/preview",
            post(preview_agent_flow_template),
        )
        .route(
            "/applications/orchestration/template/import",
            post(import_agent_flow_template),
        )
        .route(
            "/applications/:id/orchestration/versions/:version_id/restore",
            post(restore_version),
        )
        .route(
            "/applications/:id/orchestration/versions/:version_id",
            patch(update_version),
        )
}

fn to_response(state: domain::FlowEditorState) -> OrchestrationStateResponse {
    OrchestrationStateResponse {
        flow_id: state.flow.id.to_string(),
        draft: FlowDraftResponse {
            id: state.draft.id.to_string(),
            flow_id: state.draft.flow_id.to_string(),
            document: state.draft.document,
            updated_at: state.draft.updated_at.format(&Rfc3339).unwrap(),
        },
        versions: state
            .versions
            .into_iter()
            .map(|version| FlowVersionResponse {
                id: version.id.to_string(),
                sequence: version.sequence,
                trigger: version.trigger.as_str().to_string(),
                change_kind: version.change_kind.as_str().to_string(),
                summary: version.summary,
                summary_is_custom: version.summary_is_custom,
                is_protected: version.is_protected,
                created_at: version.created_at.format(&Rfc3339).unwrap(),
            })
            .collect(),
        autosave_interval_seconds: state.autosave_interval_seconds,
    }
}

fn to_template_application_response(
    application: control_plane::flow::AgentFlowTemplateApplication,
) -> AgentFlowTemplateApplicationResponse {
    AgentFlowTemplateApplicationResponse {
        application_type: application.application_type,
        name: application.name,
        description: application.description,
        icon: application.icon,
        icon_type: application.icon_type,
        icon_background: application.icon_background,
    }
}

fn to_template_dependency_response(
    dependency: AgentFlowTemplateDependency,
) -> AgentFlowTemplateDependencyResponse {
    AgentFlowTemplateDependencyResponse {
        kind: dependency.kind,
        node_id: dependency.node_id,
        node_type: dependency.node_type,
        config_version: dependency.config_version,
        provider_code: dependency.provider_code,
        model_id: dependency.model_id,
        plugin_id: dependency.plugin_id,
        plugin_version: dependency.plugin_version,
        contribution_code: dependency.contribution_code,
        node_shell: dependency.node_shell,
        schema_version: dependency.schema_version,
        plugin_unique_identifier: dependency.plugin_unique_identifier,
        package_id: dependency.package_id,
        contribution_checksum: dependency.contribution_checksum,
        compiled_contribution_hash: dependency.compiled_contribution_hash,
    }
}

fn to_template_package_response(
    template: AgentFlowTemplatePackage,
) -> AgentFlowTemplatePackageResponse {
    AgentFlowTemplatePackageResponse {
        schema_version: template.schema_version,
        application: to_template_application_response(template.application),
        flow_document: template.flow_document,
        dependencies: template
            .dependencies
            .into_iter()
            .map(to_template_dependency_response)
            .collect(),
    }
}

fn to_template_dependency_status_response(
    dependency: AgentFlowTemplateDependencyStatus,
) -> AgentFlowTemplateDependencyStatusResponse {
    AgentFlowTemplateDependencyStatusResponse {
        dependency: to_template_dependency_response(dependency.dependency),
        status: dependency.status,
        reason: dependency.reason,
    }
}

fn to_unresolved_node_response(
    unresolved_node: AgentFlowTemplateUnresolvedNode,
) -> AgentFlowTemplateUnresolvedNodeResponse {
    AgentFlowTemplateUnresolvedNodeResponse {
        node_id: unresolved_node.node_id,
        alias: unresolved_node.alias,
        original_type: unresolved_node.original_type,
        dependency_status: unresolved_node.dependency_status,
        reason: unresolved_node.reason,
        original_node: unresolved_node.original_node,
    }
}

fn to_template_preview_response(
    preview: AgentFlowTemplatePreview,
) -> AgentFlowTemplatePreviewResponse {
    AgentFlowTemplatePreviewResponse {
        schema_version: preview.schema_version,
        application: to_template_application_response(preview.application),
        dependencies: preview
            .dependencies
            .into_iter()
            .map(to_template_dependency_status_response)
            .collect(),
        unresolved_nodes: preview
            .unresolved_nodes
            .into_iter()
            .map(to_unresolved_node_response)
            .collect(),
        document: preview.document,
    }
}

fn to_import_response(imported: ImportAgentFlowTemplateResult) -> ImportAgentFlowTemplateResponse {
    ImportAgentFlowTemplateResponse {
        application: AgentFlowTemplateImportedApplicationResponse {
            id: imported.application.id.to_string(),
            application_type: imported.application.application_type.as_str().to_string(),
            name: imported.application.name,
            description: imported.application.description,
            icon: imported.application.icon,
            icon_type: imported.application.icon_type,
            icon_background: imported.application.icon_background,
            created_by: imported.application.created_by.to_string(),
            updated_at: match imported.application.updated_at.format(&Rfc3339) {
                Ok(value) => value,
                Err(_) => imported.application.updated_at.to_string(),
            },
        },
        orchestration: to_response(imported.orchestration),
        preview: to_template_preview_response(imported.preview),
    }
}

fn parse_template(value: serde_json::Value) -> Result<AgentFlowTemplatePackage, ApiError> {
    serde_json::from_value(value).map_err(|_| ControlPlaneError::InvalidInput("template").into())
}

fn parse_change_kind(value: &str) -> Result<domain::FlowChangeKind, ApiError> {
    match value {
        "layout" => Ok(domain::FlowChangeKind::Layout),
        "logical" => Ok(domain::FlowChangeKind::Logical),
        _ => Err(ControlPlaneError::InvalidInput("change_kind").into()),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_orchestration(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let flow_state = FlowService::new(state.store.clone())
        .get_or_create_editor_state(context.user.id, id)
        .await?;

    Ok(Json(ApiSuccess::new(to_response(flow_state))))
}

#[utoipa::path(
    put,
    path = "/api/console/applications/{id}/orchestration/draft",
    request_body = SaveDraftBody,
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn save_draft(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveDraftBody>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;

    let flow_state = FlowService::new(state.store.clone())
        .save_draft(SaveFlowDraftCommand {
            actor_user_id: context.user.id,
            application_id: id,
            document: body.document,
            change_kind: parse_change_kind(&body.change_kind)?,
            summary: body.summary,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_response(flow_state))))
}

#[utoipa::path(
    get,
    path = "/api/console/applications/{id}/orchestration/template",
    params(
        ("id" = String, Path, description = "Application id")
    ),
    responses(
        (status = 200, body = AgentFlowTemplatePackageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn export_agent_flow_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiSuccess<AgentFlowTemplatePackageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let template = FlowService::new(state.store.clone())
        .export_agent_flow_template(context.user.id, id)
        .await?;

    Ok(Json(ApiSuccess::new(to_template_package_response(
        template,
    ))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/orchestration/template/preview",
    request_body = AgentFlowTemplatePreviewBody,
    responses(
        (status = 200, body = AgentFlowTemplatePreviewResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn preview_agent_flow_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<AgentFlowTemplatePreviewBody>,
) -> Result<Json<ApiSuccess<AgentFlowTemplatePreviewResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let service = FlowService::new(state.store.clone());
    let resources = service
        .load_agent_flow_template_resources(context.user.id)
        .await?;
    let preview = service
        .preview_agent_flow_template(PreviewAgentFlowTemplateCommand {
            actor_user_id: context.user.id,
            template: parse_template(body.template)?,
            resources,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_template_preview_response(preview))))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/orchestration/template/import",
    request_body = ImportAgentFlowTemplateBody,
    responses(
        (status = 201, body = ImportAgentFlowTemplateResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn import_agent_flow_template(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ImportAgentFlowTemplateBody>,
) -> Result<
    (
        StatusCode,
        Json<ApiSuccess<ImportAgentFlowTemplateResponse>>,
    ),
    ApiError,
> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;

    let service = FlowService::new(state.store.clone());
    let resources = service
        .load_agent_flow_template_resources(context.user.id)
        .await?;
    let imported = service
        .import_agent_flow_template(ImportAgentFlowTemplateCommand {
            actor_user_id: context.user.id,
            template: parse_template(body.template)?,
            name: body.name,
            description: body.description,
            resources,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_import_response(imported))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/console/applications/{id}/orchestration/versions/{version_id}/restore",
    params(
        ("id" = String, Path, description = "Application id"),
        ("version_id" = String, Path, description = "Flow version id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn restore_version(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, version_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;

    let flow_state = FlowService::new(state.store.clone())
        .restore_version(context.user.id, id, version_id)
        .await?;

    Ok(Json(ApiSuccess::new(to_response(flow_state))))
}

#[utoipa::path(
    patch,
    path = "/api/console/applications/{id}/orchestration/versions/{version_id}",
    request_body = UpdateVersionBody,
    params(
        ("id" = String, Path, description = "Application id"),
        ("version_id" = String, Path, description = "Flow version id")
    ),
    responses(
        (status = 200, body = OrchestrationStateResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn update_version(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((id, version_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateVersionBody>,
) -> Result<Json<ApiSuccess<OrchestrationStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context.session)?;

    let flow_state = FlowService::new(state.store.clone())
        .update_version_metadata(UpdateFlowVersionMetadataCommand {
            actor_user_id: context.user.id,
            application_id: id,
            version_id,
            summary: body.summary,
            summary_is_custom: body.summary_is_custom,
            is_protected: body.is_protected,
        })
        .await?;

    Ok(Json(ApiSuccess::new(to_response(flow_state))))
}
