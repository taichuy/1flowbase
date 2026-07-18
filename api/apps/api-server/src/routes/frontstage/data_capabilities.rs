use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use control_plane::errors::ControlPlaneError;
use control_plane::frontstage::{FrontstagePageService, GetFrontstagePageDetailCommand};
use control_plane::model_definition::ModelDefinitionService;
use control_plane::ports::FrontstagePageRepository;
use control_plane::resource_action::{ActionDefinition, ResourceActionKernel};
use control_plane::resource_crud::parse_resource_filter_expr;
use runtime_core::runtime_acl::RuntimeDataAction;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
    response::ApiSuccess,
};

use super::FrontstageCapabilityInput;

pub const FRONTSTAGE_DATA_MODEL_QUERY_IDS: [&str; 2] = [
    "frontstage.data_model.record.list",
    "frontstage.data_model.record.get",
];

pub const FRONTSTAGE_DATA_MODEL_ACTION_IDS: [&str; 3] = [
    "frontstage.data_model.record.create",
    "frontstage.data_model.record.update",
    "frontstage.data_model.record.delete",
];

#[derive(Debug, Deserialize)]
struct DataModelCapabilityParams {
    model: String,
    #[serde(default)]
    record_id: Option<String>,
    #[serde(default)]
    values: Option<Value>,
    #[serde(default)]
    filter: Option<Value>,
    #[serde(default)]
    sort: Option<Value>,
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    page_size: Option<i64>,
}

fn parse_capability_params(params: Value) -> Result<DataModelCapabilityParams, anyhow::Error> {
    serde_json::from_value(params)
        .map_err(|_| ControlPlaneError::InvalidInput("frontstage_data_capability_params").into())
}

fn require_record_id(params: &DataModelCapabilityParams) -> Result<String, anyhow::Error> {
    params
        .record_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ControlPlaneError::InvalidInput("record_id").into())
}

fn require_values(params: &DataModelCapabilityParams) -> Result<Value, anyhow::Error> {
    match &params.values {
        Some(values) if values.is_object() => Ok(values.clone()),
        _ => Err(ControlPlaneError::InvalidInput("values").into()),
    }
}

fn parse_capability_filter(
    filter: Option<&Value>,
) -> Result<domain::ResourceFilterExpr, anyhow::Error> {
    match filter {
        None | Some(Value::Null) => Ok(domain::ResourceFilterExpr::All(vec![])),
        Some(filter) => parse_resource_filter_expr(filter)
            .map_err(|_| ControlPlaneError::InvalidInput("filter").into()),
    }
}

fn parse_capability_sorts(
    sort: Option<&Value>,
) -> Result<Vec<runtime_core::runtime_engine::RuntimeSortInput>, anyhow::Error> {
    let Some(sort) = sort else {
        return Ok(vec![]);
    };
    match sort {
        Value::Null => Ok(vec![]),
        Value::Object(entry) => {
            let field = entry
                .get("field")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("sort"))?;
            let direction = match entry.get("direction").and_then(Value::as_str) {
                None | Some("asc") => "asc",
                Some("desc") => "desc",
                Some(_) => return Err(ControlPlaneError::InvalidInput("sort").into()),
            };
            Ok(vec![runtime_core::runtime_engine::RuntimeSortInput {
                field_code: field.to_string(),
                direction: direction.to_string(),
            }])
        }
        _ => Err(ControlPlaneError::InvalidInput("sort").into()),
    }
}

struct DataCapabilityContext {
    actor: domain::ActorContext,
    params: DataModelCapabilityParams,
}

async fn resolve_data_capability_context(
    state: &ApiState,
    input: FrontstageCapabilityInput,
) -> Result<DataCapabilityContext, anyhow::Error> {
    // Re-check tab visibility on every dispatch: the tab is the capability scope.
    FrontstagePageService::new(state.store.clone())
        .get_page_detail(GetFrontstagePageDetailCommand {
            actor_user_id: input.actor_user_id,
            workspace_id: input.workspace_id,
            page_id: input.page_id,
            tab_id: input.tab_id,
        })
        .await?;
    let actor = state
        .store
        .load_actor_context_for_workspace(input.actor_user_id, input.workspace_id)
        .await?;
    Ok(DataCapabilityContext {
        actor,
        params: parse_capability_params(input.params)?,
    })
}

async fn load_scope_grant(
    state: &ApiState,
    actor: &domain::ActorContext,
    model_code: &str,
    action: RuntimeDataAction,
) -> Result<Option<runtime_core::runtime_acl::RuntimeScopeGrant>, anyhow::Error> {
    let model = state
        .runtime_engine
        .registry()
        .get(
            domain::DataModelScopeKind::Workspace,
            actor.current_workspace_id,
            model_code,
        )
        .or_else(|| {
            state.runtime_engine.registry().get(
                domain::DataModelScopeKind::System,
                domain::SYSTEM_SCOPE_ID,
                model_code,
            )
        });
    let Some(model) = model else {
        return Ok(None);
    };
    ModelDefinitionService::new(state.store.clone())
        .load_runtime_scope_grant(actor, model.model_id, action)
        .await
}

pub(crate) fn register_data_model_query_capabilities(
    registry: &mut control_plane::resource_action::ResourceActionRegistry,
    resource_code: &'static str,
) -> Result<(), anyhow::Error> {
    for query_id in FRONTSTAGE_DATA_MODEL_QUERY_IDS {
        registry.register_action(ActionDefinition::core(resource_code, query_id))?;
    }
    Ok(())
}

pub(crate) fn register_data_model_action_capabilities(
    registry: &mut control_plane::resource_action::ResourceActionRegistry,
    resource_code: &'static str,
) -> Result<(), anyhow::Error> {
    for action_id in FRONTSTAGE_DATA_MODEL_ACTION_IDS {
        registry.register_action(ActionDefinition::core(resource_code, action_id))?;
    }
    Ok(())
}

pub(crate) fn register_data_model_query_handlers(
    kernel: &mut ResourceActionKernel,
    resource_code: &'static str,
    state: Arc<ApiState>,
) -> Result<(), anyhow::Error> {
    let list_state = state.clone();
    kernel.register_json_handler(
        resource_code,
        "frontstage.data_model.record.list",
        move |input| {
            let state = list_state.clone();
            async move {
                let input: FrontstageCapabilityInput = serde_json::from_value(input)
                    .map_err(|_| ControlPlaneError::InvalidInput("frontstage_capability_input"))?;
                let context = resolve_data_capability_context(&state, input).await?;
                let scope_grant = load_scope_grant(
                    &state,
                    &context.actor,
                    &context.params.model,
                    RuntimeDataAction::View,
                )
                .await?;
                let result = state
                    .runtime_engine
                    .list_records(runtime_core::runtime_engine::RuntimeListInput {
                        actor: context.actor,
                        model_code: context.params.model.clone(),
                        scope_grant,
                        filter: parse_capability_filter(context.params.filter.as_ref())?,
                        sorts: parse_capability_sorts(context.params.sort.as_ref())?,
                        expand_relations: vec![],
                        page: context.params.page.unwrap_or(1),
                        page_size: context.params.page_size.unwrap_or(20),
                    })
                    .await
                    .map_err(map_engine_error)?;
                Ok(json!({
                    "model": context.params.model,
                    "items": result.items,
                    "total": result.total,
                }))
            }
        },
    )?;

    let get_state = state;
    kernel.register_json_handler(
        resource_code,
        "frontstage.data_model.record.get",
        move |input| {
            let state = get_state.clone();
            async move {
                let input: FrontstageCapabilityInput = serde_json::from_value(input)
                    .map_err(|_| ControlPlaneError::InvalidInput("frontstage_capability_input"))?;
                let context = resolve_data_capability_context(&state, input).await?;
                let record_id = require_record_id(&context.params)?;
                let scope_grant = load_scope_grant(
                    &state,
                    &context.actor,
                    &context.params.model,
                    RuntimeDataAction::View,
                )
                .await?;
                let record = state
                    .runtime_engine
                    .get_record(runtime_core::runtime_engine::RuntimeGetInput {
                        actor: context.actor,
                        model_code: context.params.model.clone(),
                        record_id,
                        scope_grant,
                    })
                    .await
                    .map_err(map_engine_error)?
                    .ok_or(ControlPlaneError::NotFound("runtime_record"))?;
                Ok(json!({
                    "model": context.params.model,
                    "record": record,
                }))
            }
        },
    )?;
    Ok(())
}

pub(crate) fn register_data_model_action_handlers(
    kernel: &mut ResourceActionKernel,
    resource_code: &'static str,
    state: Arc<ApiState>,
) -> Result<(), anyhow::Error> {
    let create_state = state.clone();
    kernel.register_json_handler(
        resource_code,
        "frontstage.data_model.record.create",
        move |input| {
            let state = create_state.clone();
            async move {
                let input: FrontstageCapabilityInput = serde_json::from_value(input)
                    .map_err(|_| ControlPlaneError::InvalidInput("frontstage_capability_input"))?;
                let context = resolve_data_capability_context(&state, input).await?;
                let values = require_values(&context.params)?;
                let scope_grant = load_scope_grant(
                    &state,
                    &context.actor,
                    &context.params.model,
                    RuntimeDataAction::Create,
                )
                .await?;
                let record = state
                    .runtime_engine
                    .create_record(runtime_core::runtime_engine::RuntimeCreateInput {
                        actor: context.actor,
                        model_code: context.params.model.clone(),
                        payload: values,
                        scope_grant,
                    })
                    .await
                    .map_err(map_engine_error)?;
                Ok(json!({
                    "model": context.params.model,
                    "record": record,
                }))
            }
        },
    )?;

    let update_state = state.clone();
    kernel.register_json_handler(
        resource_code,
        "frontstage.data_model.record.update",
        move |input| {
            let state = update_state.clone();
            async move {
                let input: FrontstageCapabilityInput = serde_json::from_value(input)
                    .map_err(|_| ControlPlaneError::InvalidInput("frontstage_capability_input"))?;
                let context = resolve_data_capability_context(&state, input).await?;
                let record_id = require_record_id(&context.params)?;
                let values = require_values(&context.params)?;
                let scope_grant = load_scope_grant(
                    &state,
                    &context.actor,
                    &context.params.model,
                    RuntimeDataAction::Update,
                )
                .await?;
                let record = state
                    .runtime_engine
                    .update_record(runtime_core::runtime_engine::RuntimeUpdateInput {
                        actor: context.actor,
                        model_code: context.params.model.clone(),
                        record_id,
                        payload: values,
                        scope_grant,
                    })
                    .await
                    .map_err(map_engine_error)?;
                Ok(json!({
                    "model": context.params.model,
                    "record": record,
                }))
            }
        },
    )?;

    let delete_state = state;
    kernel.register_json_handler(
        resource_code,
        "frontstage.data_model.record.delete",
        move |input| {
            let state = delete_state.clone();
            async move {
                let input: FrontstageCapabilityInput = serde_json::from_value(input)
                    .map_err(|_| ControlPlaneError::InvalidInput("frontstage_capability_input"))?;
                let context = resolve_data_capability_context(&state, input).await?;
                let record_id = require_record_id(&context.params)?;
                let scope_grant = load_scope_grant(
                    &state,
                    &context.actor,
                    &context.params.model,
                    RuntimeDataAction::Delete,
                )
                .await?;
                let record = state
                    .runtime_engine
                    .delete_record(runtime_core::runtime_engine::RuntimeDeleteInput {
                        actor: context.actor,
                        model_code: context.params.model.clone(),
                        record_id,
                        scope_grant,
                    })
                    .await
                    .map_err(map_engine_error)?;
                Ok(json!({
                    "model": context.params.model,
                    "record": record,
                }))
            }
        },
    )?;
    Ok(())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageDataCapabilityFieldResponse {
    pub code: String,
    pub title: String,
    pub field_kind: String,
    pub is_required: bool,
    pub is_writable: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageDataCapabilityModelResponse {
    pub code: String,
    pub scope_kind: String,
    pub fields: Vec<FrontstageDataCapabilityFieldResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageDataCapabilityDescriptorResponse {
    pub id: String,
    pub kind: String,
    pub params_schema: Value,
    pub result_schema: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageDataCapabilitiesResponse {
    pub queries: Vec<FrontstageDataCapabilityDescriptorResponse>,
    pub actions: Vec<FrontstageDataCapabilityDescriptorResponse>,
    pub models: Vec<FrontstageDataCapabilityModelResponse>,
}

fn model_param_schema(extra: Value) -> Value {
    let mut schema = json!({
        "type": "object",
        "required": ["model"],
        "properties": {
            "model": { "type": "string", "description": "Data model code" }
        }
    });
    if let (Some(base), Some(extra)) = (schema["properties"].as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            base.insert(key.clone(), value.clone());
        }
    }
    schema
}

fn record_result_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "model": { "type": "string" },
            "record": { "type": "object" }
        }
    })
}

fn capability_descriptors() -> (
    Vec<FrontstageDataCapabilityDescriptorResponse>,
    Vec<FrontstageDataCapabilityDescriptorResponse>,
) {
    let queries = vec![
        FrontstageDataCapabilityDescriptorResponse {
            id: "frontstage.data_model.record.list".to_string(),
            kind: "query".to_string(),
            params_schema: model_param_schema(json!({
                "filter": { "type": "object" },
                "sort": {
                    "type": "object",
                    "properties": {
                        "field": { "type": "string" },
                        "direction": { "type": "string", "enum": ["asc", "desc"] }
                    }
                },
                "page": { "type": "integer", "minimum": 1 },
                "page_size": { "type": "integer", "minimum": 1 }
            })),
            result_schema: json!({
                "type": "object",
                "properties": {
                    "model": { "type": "string" },
                    "items": { "type": "array", "items": { "type": "object" } },
                    "total": { "type": "integer" }
                }
            }),
        },
        FrontstageDataCapabilityDescriptorResponse {
            id: "frontstage.data_model.record.get".to_string(),
            kind: "query".to_string(),
            params_schema: model_param_schema(json!({
                "record_id": { "type": "string" }
            })),
            result_schema: record_result_schema(),
        },
    ];
    let actions = vec![
        FrontstageDataCapabilityDescriptorResponse {
            id: "frontstage.data_model.record.create".to_string(),
            kind: "action".to_string(),
            params_schema: model_param_schema(json!({
                "values": { "type": "object" }
            })),
            result_schema: record_result_schema(),
        },
        FrontstageDataCapabilityDescriptorResponse {
            id: "frontstage.data_model.record.update".to_string(),
            kind: "action".to_string(),
            params_schema: model_param_schema(json!({
                "record_id": { "type": "string" },
                "values": { "type": "object" }
            })),
            result_schema: record_result_schema(),
        },
        FrontstageDataCapabilityDescriptorResponse {
            id: "frontstage.data_model.record.delete".to_string(),
            kind: "action".to_string(),
            params_schema: model_param_schema(json!({
                "record_id": { "type": "string" }
            })),
            result_schema: record_result_schema(),
        },
    ];
    (queries, actions)
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/data-capabilities",
    params(("workspace_id" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = FrontstageDataCapabilitiesResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_data_capabilities(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<ApiSuccess<FrontstageDataCapabilitiesResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    state
        .store
        .load_actor_context_for_workspace(context.user.id, workspace_id)
        .await?;
    let models = state
        .runtime_engine
        .registry()
        .list_available_for_workspace(workspace_id)
        .into_iter()
        .map(|model| FrontstageDataCapabilityModelResponse {
            code: model.model_code,
            scope_kind: model.scope_kind.as_str().to_string(),
            fields: model
                .fields
                .iter()
                .map(|field| FrontstageDataCapabilityFieldResponse {
                    code: field.code.clone(),
                    title: field.title.clone(),
                    field_kind: field.field_kind.as_str().to_string(),
                    is_required: field.is_required,
                    is_writable: field.is_writable,
                })
                .collect(),
        })
        .collect();
    let (queries, actions) = capability_descriptors();
    Ok(Json(ApiSuccess::new(FrontstageDataCapabilitiesResponse {
        queries,
        actions,
        models,
    })))
}

fn map_engine_error(error: anyhow::Error) -> anyhow::Error {
    if error
        .downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
        .is_some()
    {
        if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
            error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
        {
            return ControlPlaneError::PermissionDenied(reason).into();
        }
    }

    if error.to_string().contains("runtime record not found") {
        return ControlPlaneError::NotFound("runtime_record").into();
    }

    if let Some(model_error) =
        error.downcast_ref::<runtime_core::runtime_engine::RuntimeModelError>()
    {
        let code = match model_error {
            runtime_core::runtime_engine::RuntimeModelError::Unavailable(_) => {
                "runtime_model_unavailable"
            }
            runtime_core::runtime_engine::RuntimeModelError::NotPublished(_) => {
                "model_not_published"
            }
            runtime_core::runtime_engine::RuntimeModelError::Disabled(_) => "model_disabled",
            runtime_core::runtime_engine::RuntimeModelError::Broken(_) => "model_broken",
            runtime_core::runtime_engine::RuntimeModelError::RecordActionNotAllowed { .. } => {
                "runtime_model_record_action_not_allowed"
            }
            runtime_core::runtime_engine::RuntimeModelError::MissingCreateRequiredFields {
                ..
            } => {
                return ControlPlaneError::InvalidInput("api_required").into();
            }
        };
        return ControlPlaneError::Conflict(code).into();
    }

    error
}
