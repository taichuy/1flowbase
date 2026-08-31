use std::sync::Arc;

use access_control::ConsoleRouteOwnership::ConsoleOperation;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use control_plane::model_definition::{
    AddModelFieldCommand, BatchDeleteModelDefinitionsCommand, CreateModelDefinitionCommand,
    CreateScopeDataModelGrantCommand, DeleteModelDefinitionCommand, DeleteModelFieldCommand,
    ModelDefinitionService, UpdateModelDefinitionCommand, UpdateModelDefinitionStatusCommand,
    UpdateModelFieldCommand, UpdateScopeDataModelGrantCommand,
};
use control_plane::resource_crud::{
    parse_resource_filter, ResourceBatchSelection, ResourceCrudDescriptor,
};
use control_plane::runtime_registry_sync::ModelDefinitionMutationService;
use control_plane::{
    file_management::project_attachments_model_titles, i18n_catalog::CatalogResolver,
    system_metadata::project_system_metadata_titles,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::{
        console_route_assembly::{console_get, ConsoleRouteAssembly},
        helpers,
    },
    runtime_registry_sync::ApiRuntimeRegistrySync,
};

#[path = "model_definitions_interface.rs"]
pub(crate) mod interface;

const STATE_MODEL_RESOURCE: ResourceCrudDescriptor =
    ResourceCrudDescriptor::new("state_model", "id");

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateModelDefinitionBody {
    pub scope_kind: String,
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateModelDefinitionBody {
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::routes::helpers::deserialize_present_optional"
    )]
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub external_table_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateModelFieldBody {
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub external_field_key: Option<String>,
    pub field_kind: String,
    #[serde(default)]
    pub is_required: bool,
    pub api_required: Option<bool>,
    #[serde(default)]
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    #[serde(default = "empty_json_object")]
    pub display_options: serde_json::Value,
    pub relation_target_model_id: Option<String>,
    #[serde(default = "empty_json_object")]
    pub relation_options: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateModelFieldBody {
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub is_required: bool,
    pub api_required: Option<bool>,
    #[serde(default)]
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    #[serde(default = "empty_json_object")]
    pub display_options: serde_json::Value,
    #[serde(default = "empty_json_object")]
    pub relation_options: serde_json::Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateScopeGrantBody {
    pub scope_kind: String,
    pub scope_id: Uuid,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub permission_profile: String,
    #[serde(default)]
    pub confirm_unsafe_external_source_system_all: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateScopeGrantBody {
    pub enabled: Option<bool>,
    pub permission_profile: Option<String>,
    #[serde(default)]
    pub confirm_unsafe_external_source_system_all: bool,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmationQuery {
    pub confirmed: Option<bool>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListModelsQuery {
    pub data_source_id: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct CompatibleTemplateCatalogQuery {
    pub data_source_id: String,
    pub resource_key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CompatibleTemplateCatalogEntryResponse {
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
    pub summary: String,
    pub description: String,
    pub system_fields: Vec<CompatibleTemplateSystemFieldResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CompatibleTemplateSystemFieldResponse {
    pub code: String,
    pub summary: String,
    pub description: String,
    pub field_kind: String,
    pub required: bool,
}

fn compatible_template_field_kind(value_schema: &serde_json::Value) -> &'static str {
    if value_schema
        .get("format")
        .and_then(serde_json::Value::as_str)
        == Some("date-time")
    {
        return "datetime";
    }

    if let Some(non_null_schema) = value_schema
        .get("anyOf")
        .and_then(serde_json::Value::as_array)
        .and_then(|schemas| {
            schemas.iter().find(|schema| {
                schema.get("type").and_then(serde_json::Value::as_str) != Some("null")
            })
        })
    {
        return compatible_template_field_kind(non_null_schema);
    }

    match value_schema.get("type").and_then(serde_json::Value::as_str) {
        Some("boolean") => "boolean",
        Some("integer" | "number") => "number",
        Some("string") => "string",
        Some("array" | "object") | None => "json",
        Some(_) => "json",
    }
}

fn compatible_template_response(
    descriptor: &plugin_framework::DataModelTemplateDescriptor,
) -> CompatibleTemplateCatalogEntryResponse {
    CompatibleTemplateCatalogEntryResponse {
        template_provider: descriptor.identity.provider.clone(),
        template_code: descriptor.identity.code.clone(),
        template_version: descriptor.identity.version.clone(),
        summary: descriptor.summary.clone(),
        description: descriptor.description.clone(),
        system_fields: descriptor
            .system_fields
            .iter()
            .map(|field| CompatibleTemplateSystemFieldResponse {
                code: field.code.clone(),
                summary: field.summary.clone(),
                description: field.description.clone(),
                field_kind: compatible_template_field_kind(&field.value_schema).to_owned(),
                required: field.required,
            })
            .collect(),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchDeleteModelDefinitionsBody {
    #[serde(rename = "filterByTk")]
    #[schema(value_type = Object)]
    pub filter_by_tk: Option<serde_json::Value>,
    #[schema(value_type = Object)]
    pub filter: Option<serde_json::Value>,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataModelRecordCapabilitiesResponse {
    pub can_list: bool,
    pub can_get: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataModelCapabilitiesResponse {
    pub can_delete: bool,
    pub can_add_user_field: bool,
    pub can_update_lifecycle_status: bool,
    pub record: DataModelRecordCapabilitiesResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelFieldCapabilitiesResponse {
    pub ownership: String,
    pub can_update_presentation_metadata: bool,
    pub can_update_physical_metadata: bool,
    pub can_delete: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelFieldResponse {
    pub id: String,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub physical_column_name: String,
    pub external_field_key: Option<String>,
    pub field_kind: String,
    pub is_system: bool,
    pub is_writable: bool,
    pub is_required: bool,
    pub api_required: bool,
    pub is_unique: bool,
    #[schema(value_type = Value)]
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    #[schema(value_type = Value)]
    pub display_options: serde_json::Value,
    pub relation_target_model_id: Option<String>,
    #[schema(value_type = Value)]
    pub relation_options: serde_json::Value,
    pub sort_order: i32,
    pub capabilities: ModelFieldCapabilitiesResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModelDefinitionResponse {
    pub id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub runtime_availability: String,
    pub data_source_id: Option<String>,
    pub source_kind: String,
    pub external_resource_key: Option<String>,
    pub external_table_id: Option<String>,
    pub template_provider: String,
    pub template_code: String,
    pub template_version: String,
    pub template_summary: Option<String>,
    pub physical_table_name: String,
    pub acl_namespace: String,
    pub audit_namespace: String,
    pub builtin_kind: Option<String>,
    pub capabilities: DataModelCapabilitiesResponse,
    pub fields: Vec<ModelFieldResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentFlowDataModelFieldOptionResponse {
    pub code: String,
    pub title: String,
    pub value_type: String,
    pub required: bool,
    pub writable: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentFlowDataModelOptionResponse {
    pub value: String,
    pub label: String,
    pub state: String,
    pub disabled: bool,
    pub disabled_reason: Option<String>,
    pub model_id: String,
    pub model_code: String,
    pub fields: Vec<AgentFlowDataModelFieldOptionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeletedResponse {
    pub deleted: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchDeletedResponse {
    pub deleted: bool,
    pub deleted_count: usize,
    pub deleted_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScopeGrantResponse {
    pub id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub data_model_id: String,
    pub enabled: bool,
    pub permission_profile: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataModelAdvisorFindingResponse {
    pub id: String,
    pub data_model_id: String,
    pub severity: String,
    pub code: String,
    pub message: String,
    pub recommended_action: String,
    pub can_acknowledge: bool,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new().route(
        "/models/agent-flow-options",
        console_get(
            list_agent_flow_options,
            ConsoleOperation("agent_flow.data_model_options.list".to_string()),
        ),
    )
}

fn empty_json_object() -> serde_json::Value {
    serde_json::json!({})
}

fn default_true() -> bool {
    true
}

fn to_record_capabilities_response(
    capabilities: domain::DataModelRecordCapabilities,
) -> DataModelRecordCapabilitiesResponse {
    DataModelRecordCapabilitiesResponse {
        can_list: capabilities.can_list,
        can_get: capabilities.can_get,
        can_create: capabilities.can_create,
        can_update: capabilities.can_update,
        can_delete: capabilities.can_delete,
    }
}

fn to_model_capabilities_response(
    capabilities: domain::DataModelCapabilities,
) -> DataModelCapabilitiesResponse {
    DataModelCapabilitiesResponse {
        can_delete: capabilities.can_delete,
        can_add_user_field: capabilities.can_add_user_field,
        can_update_lifecycle_status: capabilities.can_update_lifecycle_status,
        record: to_record_capabilities_response(capabilities.record),
    }
}

fn to_field_capabilities_response(
    capabilities: domain::DataModelFieldCapabilities,
) -> ModelFieldCapabilitiesResponse {
    ModelFieldCapabilitiesResponse {
        ownership: capabilities.ownership.as_str().to_string(),
        can_update_presentation_metadata: capabilities.can_update_presentation_metadata,
        can_update_physical_metadata: capabilities.can_update_physical_metadata,
        can_delete: capabilities.can_delete,
    }
}

fn to_model_field_response(
    model: &domain::ModelDefinitionRecord,
    field: domain::ModelFieldRecord,
) -> ModelFieldResponse {
    let capabilities = domain::data_model_field_capabilities(model, &field);
    ModelFieldResponse {
        id: field.id.to_string(),
        code: field.code,
        title: field.title,
        description: field.description,
        physical_column_name: field.physical_column_name,
        external_field_key: field.external_field_key,
        field_kind: field.field_kind.as_str().to_string(),
        is_system: field.is_system,
        is_writable: field.is_writable,
        is_required: field.is_required,
        api_required: field.api_required,
        is_unique: field.is_unique,
        default_value: field.default_value,
        display_interface: field.display_interface,
        display_options: field.display_options,
        relation_target_model_id: field.relation_target_model_id.map(|id| id.to_string()),
        relation_options: field.relation_options,
        sort_order: field.sort_order,
        capabilities: to_field_capabilities_response(capabilities),
    }
}

pub(super) fn to_model_definition_response(
    model: domain::ModelDefinitionRecord,
    template_catalog: &runtime_core::data_model_template_registry::DataModelTemplateCatalog,
) -> ModelDefinitionResponse {
    let template_identity = plugin_framework::DataModelTemplateIdentity {
        provider: model.template_provider.clone(),
        code: model.template_code.clone(),
        version: model.template_version.clone(),
    };
    let template_summary = match template_catalog.resolve(&template_identity) {
        Ok(template) => Some(template.descriptor().summary.clone()),
        Err(error) => {
            tracing::warn!(
                model_id = %model.id,
                template_identity = %template_identity.canonical_name(),
                error = %error,
                "data model template summary is unavailable"
            );
            None
        }
    };
    let builtin_kind =
        domain::builtin_contract_for_model(&model).map(|contract| contract.kind.as_str().into());
    let capabilities = domain::data_model_capabilities(&model);
    let fields = model
        .fields
        .iter()
        .cloned()
        .map(|field| to_model_field_response(&model, field))
        .collect();
    let data_source_id = match model.source_kind {
        domain::DataModelSourceKind::MainSource => Some("main".to_string()),
        domain::DataModelSourceKind::ExternalSource => {
            model.data_source_instance_id.map(|id| id.to_string())
        }
    };

    ModelDefinitionResponse {
        id: model.id.to_string(),
        scope_kind: model.scope_kind.as_str().to_string(),
        scope_id: model.scope_id.to_string(),
        code: model.code,
        title: model.title,
        description: model.description,
        status: model.status.as_str().to_string(),
        runtime_availability: runtime_availability_for_status(model.status).to_string(),
        data_source_id,
        source_kind: model.source_kind.as_str().to_string(),
        external_resource_key: model.external_resource_key,
        external_table_id: model.external_table_id,
        template_provider: model.template_provider,
        template_code: model.template_code,
        template_version: model.template_version,
        template_summary,
        physical_table_name: model.physical_table_name,
        acl_namespace: model.acl_namespace,
        audit_namespace: model.audit_namespace,
        builtin_kind,
        capabilities: to_model_capabilities_response(capabilities),
        fields,
    }
}

fn agent_flow_option_state(
    status: domain::DataModelStatus,
) -> (&'static str, Option<&'static str>) {
    match runtime_core::runtime_model_registry::RuntimeDataModelAvailability::from_status(status) {
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::Available => {
            ("enabled", None)
        }
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::NotPublished => {
            ("unpublished", Some("Data Model is not published"))
        }
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::Disabled => {
            ("disabled", Some("Data Model is disabled"))
        }
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::Broken => {
            ("broken", Some("Data Model is broken"))
        }
    }
}

fn to_agent_flow_data_model_option_response(
    model: domain::ModelDefinitionRecord,
) -> AgentFlowDataModelOptionResponse {
    let (state, disabled_reason) = agent_flow_option_state(model.status);
    let mut fields = model.fields;
    fields.sort_by_key(|field| field.sort_order);
    let label = if model.title.is_empty() {
        model.code.clone()
    } else {
        model.title
    };

    AgentFlowDataModelOptionResponse {
        value: model.code.clone(),
        label,
        state: state.to_string(),
        disabled: state != "enabled",
        disabled_reason: disabled_reason.map(str::to_string),
        model_id: model.id.to_string(),
        model_code: model.code,
        fields: fields
            .into_iter()
            .map(|field| AgentFlowDataModelFieldOptionResponse {
                title: if field.title.is_empty() {
                    field.code.clone()
                } else {
                    field.title
                },
                code: field.code,
                value_type: field.field_kind.as_str().to_string(),
                required: field.is_required,
                writable: field.is_writable,
            })
            .collect(),
    }
}

fn to_scope_grant_response(grant: domain::ScopeDataModelGrantRecord) -> ScopeGrantResponse {
    ScopeGrantResponse {
        id: grant.id.to_string(),
        scope_kind: grant.scope_kind.as_str().to_string(),
        scope_id: grant.scope_id.to_string(),
        data_model_id: grant.data_model_id.to_string(),
        enabled: grant.enabled,
        permission_profile: grant.permission_profile.as_str().to_string(),
    }
}

fn to_advisor_finding_response(
    finding: domain::DataModelAdvisorFinding,
) -> DataModelAdvisorFindingResponse {
    DataModelAdvisorFindingResponse {
        id: finding.id,
        data_model_id: finding.data_model_id.to_string(),
        severity: finding.severity.as_str().to_string(),
        code: finding.code,
        message: finding.message,
        recommended_action: finding.recommended_action,
        can_acknowledge: finding.can_acknowledge,
    }
}

fn runtime_availability_for_status(status: domain::DataModelStatus) -> &'static str {
    match runtime_core::runtime_model_registry::RuntimeDataModelAvailability::from_status(status) {
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::Available => {
            "available"
        }
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::NotPublished => {
            "not_published"
        }
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::Disabled => "disabled",
        runtime_core::runtime_model_registry::RuntimeDataModelAvailability::Broken => "broken",
    }
}

fn parse_scope_kind(raw: &str) -> Result<domain::DataModelScopeKind, ApiError> {
    match raw {
        "workspace" => Ok(domain::DataModelScopeKind::Workspace),
        "system" => Ok(domain::DataModelScopeKind::System),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("scope_kind").into()),
    }
}

fn parse_model_status(raw: &str) -> Result<domain::DataModelStatus, ApiError> {
    match raw {
        "draft" => Ok(domain::DataModelStatus::Draft),
        "published" => Ok(domain::DataModelStatus::Published),
        "disabled" => Ok(domain::DataModelStatus::Disabled),
        "broken" => Ok(domain::DataModelStatus::Broken),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("status").into()),
    }
}

fn parse_field_kind(raw: &str) -> Result<domain::ModelFieldKind, ApiError> {
    match raw {
        "string" => Ok(domain::ModelFieldKind::String),
        "number" => Ok(domain::ModelFieldKind::Number),
        "boolean" => Ok(domain::ModelFieldKind::Boolean),
        "datetime" => Ok(domain::ModelFieldKind::Datetime),
        "enum" => Ok(domain::ModelFieldKind::Enum),
        "text" => Ok(domain::ModelFieldKind::Text),
        "json" => Ok(domain::ModelFieldKind::Json),
        "many_to_one" => Ok(domain::ModelFieldKind::ManyToOne),
        "one_to_many" => Ok(domain::ModelFieldKind::OneToMany),
        "many_to_many" => Ok(domain::ModelFieldKind::ManyToMany),
        _ => Err(control_plane::errors::ControlPlaneError::InvalidInput("field_kind").into()),
    }
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/model-definitions",
    responses((status = 200, body = [ModelDefinitionResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_models(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ListModelsQuery>,
) -> Result<helpers::ApiJson<Vec<ModelDefinitionResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::ModelDefinitionsInput::List { query, locale },
    )
    .await?;
    let interface::ModelDefinitionsOutput::Models(models) = output else {
        unreachable!("model definitions list binding returned a different output")
    };
    Ok(helpers::ok(models))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/model-templates",
    params(CompatibleTemplateCatalogQuery),
    responses((status = 200, body = [CompatibleTemplateCatalogEntryResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn list_compatible_templates(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<CompatibleTemplateCatalogQuery>,
) -> Result<Json<ApiSuccess<Vec<CompatibleTemplateCatalogEntryResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.templates.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::ModelDefinitionsInput::ListCompatibleTemplates(query),
    )
    .await?;
    let interface::ModelDefinitionsOutput::Templates(templates) = output else {
        unreachable!("model templates binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(templates)))
}

#[utoipa::path(
    get,
    path = "/api/console/models/agent-flow-options",
    responses((status = 200, body = [AgentFlowDataModelOptionResponse]), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_agent_flow_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<Vec<AgentFlowDataModelOptionResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.agent-flow-options.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::ModelDefinitionsInput::ListAgentFlowOptions,
    )
    .await?;
    let interface::ModelDefinitionsOutput::AgentFlowOptions(options) = output else {
        unreachable!("model agent-flow options binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(options)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/model-definitions",
    request_body = CreateModelDefinitionBody,
    responses((status = 201, body = ModelDefinitionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn create_model(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<CreateModelDefinitionBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ModelDefinitionResponse>>), ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::Create(body),
    )
    .await?;
    let interface::ModelDefinitionsOutput::Model(model) = output else {
        unreachable!("model create binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(model))))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/model-definitions/{id}/advisor-findings",
    params(("id" = String, Path, description = "Model definition id")),
    responses((status = 200, body = [DataModelAdvisorFindingResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_advisor_findings(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<DataModelAdvisorFindingResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.advisor-findings.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::ModelDefinitionsInput::AdvisorFindings { model_id },
    )
    .await?;
    let interface::ModelDefinitionsOutput::AdvisorFindings(findings) = output else {
        unreachable!("model advisor binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(findings)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/data-models/model-definitions/{id}/scope-grants",
    params(("id" = String, Path, description = "Model definition id")),
    responses((status = 200, body = [ScopeGrantResponse]), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn list_scope_grants(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<ScopeGrantResponse>>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.scope-grants.list.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        interface::ModelDefinitionsInput::ListScopeGrants { model_id },
    )
    .await?;
    let interface::ModelDefinitionsOutput::ScopeGrants(grants) = output else {
        unreachable!("model scope-grants binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(grants)))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/data-models/model-definitions/{id}",
    request_body = UpdateModelDefinitionBody,
    params(("id" = String, Path, description = "Model definition id")),
    responses((status = 200, body = ModelDefinitionResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_model(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
    Json(body): Json<UpdateModelDefinitionBody>,
) -> Result<Json<ApiSuccess<ModelDefinitionResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::Update { model_id, body },
    )
    .await?;
    let interface::ModelDefinitionsOutput::Model(model) = output else {
        unreachable!("model update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(model)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/data-models/model-definitions/{id}",
    params(
        ("id" = String, Path, description = "Model definition id"),
        ("confirmed" = Option<bool>, Query, description = "Must be true to confirm deletion")
    ),
    responses((status = 200, body = DeletedResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_model(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
    Query(query): Query<ConfirmationQuery>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::Delete {
            model_id,
            confirmed: query.confirmed.unwrap_or(false),
        },
    )
    .await?;
    let interface::ModelDefinitionsOutput::Deleted = output else {
        unreachable!("model delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(
        serde_json::json!({ "deleted": true }),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/model-definitions:batchDelete",
    request_body = BatchDeleteModelDefinitionsBody,
    responses((status = 200, body = BatchDeletedResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn batch_delete_models(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<BatchDeleteModelDefinitionsBody>,
) -> Result<Json<ApiSuccess<BatchDeletedResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.batch-delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::BatchDelete(body),
    )
    .await?;
    let interface::ModelDefinitionsOutput::BatchDeleted(response) = output else {
        unreachable!("model batch-delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/model-definitions/{id}/fields",
    request_body = CreateModelFieldBody,
    params(("id" = String, Path, description = "Model definition id")),
    responses((status = 201, body = ModelFieldResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn create_field(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
    Json(body): Json<CreateModelFieldBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ModelFieldResponse>>), ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.fields.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::CreateField { model_id, body },
    )
    .await?;
    let interface::ModelDefinitionsOutput::Field(field) = output else {
        unreachable!("model field create binding returned a different output")
    };

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(field))))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/data-models/model-definitions/{id}/fields/{field_id}",
    request_body = UpdateModelFieldBody,
    params(
        ("id" = String, Path, description = "Model definition id"),
        ("field_id" = String, Path, description = "Model field id")
    ),
    responses((status = 200, body = ModelFieldResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_field(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_id, field_id)): Path<(String, String)>,
    Json(body): Json<UpdateModelFieldBody>,
) -> Result<Json<ApiSuccess<ModelFieldResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.fields.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::UpdateField {
            model_id,
            field_id,
            body,
        },
    )
    .await?;
    let interface::ModelDefinitionsOutput::Field(field) = output else {
        unreachable!("model field update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(field)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/data-models/model-definitions/{id}/fields/{field_id}",
    params(
        ("id" = String, Path, description = "Model definition id"),
        ("field_id" = String, Path, description = "Model field id"),
        ("confirmed" = Option<bool>, Query, description = "Must be true to confirm deletion")
    ),
    responses((status = 200, body = DeletedResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_field(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_id, field_id)): Path<(String, String)>,
    Query(query): Query<ConfirmationQuery>,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.fields.delete.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::DeleteField {
            model_id,
            field_id,
            confirmed: query.confirmed.unwrap_or(false),
        },
    )
    .await?;
    let interface::ModelDefinitionsOutput::Deleted = output else {
        unreachable!("model field delete binding returned a different output")
    };

    Ok(Json(ApiSuccess::new(
        serde_json::json!({ "deleted": true }),
    )))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/data-models/model-definitions/{id}/scope-grants",
    request_body = CreateScopeGrantBody,
    params(("id" = String, Path, description = "Model definition id")),
    responses((status = 201, body = ScopeGrantResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn create_scope_grant(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_id): Path<String>,
    Json(body): Json<CreateScopeGrantBody>,
) -> Result<(StatusCode, Json<ApiSuccess<ScopeGrantResponse>>), ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.scope-grants.create.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::CreateScopeGrant { model_id, body },
    )
    .await?;
    let interface::ModelDefinitionsOutput::ScopeGrant(grant) = output else {
        unreachable!("scope grant create binding returned a different output")
    };

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(grant))))
}

#[utoipa::path(
    patch,
    path = "/api/console/settings/data-models/model-definitions/{id}/scope-grants/{grant_id}",
    request_body = UpdateScopeGrantBody,
    params(
        ("id" = String, Path, description = "Model definition id"),
        ("grant_id" = String, Path, description = "Scope grant id")
    ),
    responses((status = 200, body = ScopeGrantResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn update_scope_grant(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_id, grant_id)): Path<(String, String)>,
    Json(body): Json<UpdateScopeGrantBody>,
) -> Result<Json<ApiSuccess<ScopeGrantResponse>>, ApiError> {
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-definitions.scope-grants.update.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf { state, headers },
        interface::ModelDefinitionsInput::UpdateScopeGrant {
            model_id,
            grant_id,
            body,
        },
    )
    .await?;
    let interface::ModelDefinitionsOutput::ScopeGrant(grant) = output else {
        unreachable!("scope grant update binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(grant)))
}
