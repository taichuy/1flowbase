use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{
        require_csrf::require_csrf,
        require_session::{require_session, RequestContext},
    },
    response::ApiSuccess,
};
use control_plane::resource_crud::parse_resource_filter_expr;
use control_plane::{audit::audit_log, ports::AuthRepository};

fn map_runtime_error(error: anyhow::Error) -> ApiError {
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return control_plane::errors::ControlPlaneError::PermissionDenied(reason).into();
    }

    if error.to_string().contains("runtime record not found") {
        return control_plane::errors::ControlPlaneError::NotFound("runtime_record").into();
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
                return control_plane::errors::ControlPlaneError::InvalidInput("api_required")
                    .into();
            }
        };
        return control_plane::errors::ControlPlaneError::Conflict(code).into();
    }

    error.into()
}

fn runtime_acl_denial_reason(error: &anyhow::Error) -> Option<&'static str> {
    if let Some(runtime_core::runtime_acl::RuntimeAclError::PermissionDenied(reason)) =
        error.downcast_ref::<runtime_core::runtime_acl::RuntimeAclError>()
    {
        return Some(reason);
    }

    None
}

#[derive(Debug, Clone, Copy)]
enum RuntimeModelAction {
    List,
    Get,
    Create,
    Update,
    Delete,
}

impl RuntimeModelAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Get => "get",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    fn data_action(self) -> runtime_core::runtime_acl::RuntimeDataAction {
        match self {
            Self::List | Self::Get => runtime_core::runtime_acl::RuntimeDataAction::View,
            Self::Create => runtime_core::runtime_acl::RuntimeDataAction::Create,
            Self::Update => runtime_core::runtime_acl::RuntimeDataAction::Update,
            Self::Delete => runtime_core::runtime_acl::RuntimeDataAction::Delete,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct RuntimeListQueryParams {
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub expand: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, ToSchema)]
#[schema(value_type = Object)]
pub struct RuntimeRecordEnvelope(#[allow(dead_code)] Value);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RuntimeListResponse {
    #[schema(value_type = Vec<RuntimeRecordEnvelope>)]
    pub items: Vec<Value>,
    pub total: i64,
}

fn round_cache_hit_rate(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then(|| (value * 10_000.0).round() / 10_000.0)
}

fn application_log_cache_hit_rate_for_response(record: &Value) -> Option<f64> {
    let total_tokens = record.get("total_tokens").and_then(Value::as_f64);
    let input_cache_hit_tokens = record.get("input_cache_hit_tokens").and_then(Value::as_f64);

    if let (Some(total_tokens), Some(input_cache_hit_tokens)) =
        (total_tokens, input_cache_hit_tokens)
    {
        if total_tokens > 0.0 && total_tokens.is_finite() && input_cache_hit_tokens.is_finite() {
            return round_cache_hit_rate(input_cache_hit_tokens / total_tokens);
        }
    }

    None
}

fn normalize_application_log_cache_hit_rate(record: &mut Value) {
    let value = application_log_cache_hit_rate_for_response(record)
        .map_or(serde_json::Value::Null, serde_json::Value::from);

    if let Some(object) = record.as_object_mut() {
        object.insert("input_cache_hit_rate".to_string(), value);
    }
}

fn runtime_record_response(model_code: &str, mut record: Value) -> Value {
    if model_code == "application_run_log_summaries" {
        normalize_application_log_cache_hit_rate(&mut record);
    }

    record
}

fn runtime_list_response(model_code: &str, items: Vec<Value>, total: i64) -> RuntimeListResponse {
    RuntimeListResponse {
        items: items
            .into_iter()
            .map(|record| runtime_record_response(model_code, record))
            .collect(),
        total,
    }
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/models/:model_code/list", get(list_records))
        .route("/models/:model_code/get/:id", get(get_record))
        .route("/models/:model_code/create", post(create_record))
        .route("/models/:model_code/update/:id", patch(update_record))
        .route("/models/:model_code/delete/:id", delete(delete_record))
}

fn parse_filter(filter: Option<&str>) -> Result<domain::ResourceFilterExpr, ApiError> {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(domain::ResourceFilterExpr::All(vec![]));
    };
    let filter: Value = serde_json::from_str(filter)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("filter"))?;
    parse_resource_filter_expr(&filter).map_err(Into::into)
}

fn parse_sorts(
    sort: Option<&str>,
) -> Result<Vec<runtime_core::runtime_engine::RuntimeSortInput>, ApiError> {
    let Some(sort) = sort else {
        return Ok(vec![]);
    };
    let mut parts = sort.splitn(2, ':');
    let field_code = parts
        .next()
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "sort",
        ))?;
    let direction = parts
        .next()
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
            "sort",
        ))?;

    Ok(vec![runtime_core::runtime_engine::RuntimeSortInput {
        field_code: field_code.to_string(),
        direction: direction.to_string(),
    }])
}

fn parse_expand(expand: Option<&str>) -> Vec<String> {
    expand
        .map(|expand| {
            expand
                .split(',')
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

async fn load_runtime_scope_grant(
    state: &ApiState,
    actor: &domain::ActorContext,
    data_model_id: uuid::Uuid,
    action: runtime_core::runtime_acl::RuntimeDataAction,
) -> Result<Option<runtime_core::runtime_acl::RuntimeScopeGrant>, ApiError> {
    Ok(
        control_plane::model_definition::ModelDefinitionService::new(state.store.clone())
            .load_runtime_scope_grant(actor, data_model_id, action)
            .await?,
    )
}

fn resolve_runtime_model(
    state: &ApiState,
    actor: &domain::ActorContext,
    model_code: &str,
) -> Option<runtime_core::model_metadata::ModelMetadata> {
    state
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
        })
}

enum RuntimeCredential {
    Session(Box<RequestContext>),
    ApiKey(Box<control_plane::auth::ApiKeyActor>),
}

impl RuntimeCredential {
    fn actor(&self) -> &domain::ActorContext {
        match self {
            Self::Session(context) => &context.actor,
            Self::ApiKey(context) => &context.actor,
        }
    }

    fn cache_identity(&self) -> serde_json::Value {
        let actor = self.actor();
        let mut permissions = actor.permissions.iter().cloned().collect::<Vec<_>>();
        permissions.sort();

        match self {
            Self::Session(_context) => serde_json::json!({
                "kind": "session",
                "user_id": actor.user_id,
                "tenant_id": actor.tenant_id,
                "workspace_id": actor.current_workspace_id,
                "role": actor.effective_display_role,
                "is_root": actor.is_root,
                "permissions": permissions,
            }),
            Self::ApiKey(context) => serde_json::json!({
                "kind": "api_key",
                "api_key_id": context.api_key.id,
                "key_kind": context.api_key.key_kind.as_str(),
                "user_id": actor.user_id,
                "tenant_id": actor.tenant_id,
                "workspace_id": actor.current_workspace_id,
                "role": actor.effective_display_role,
                "is_root": actor.is_root,
                "permissions": permissions,
            }),
        }
    }
}

fn runtime_records_cacheable_metadata(
    state: &ApiState,
    actor: &domain::ActorContext,
    model_code: &str,
) -> Option<runtime_core::model_metadata::ModelMetadata> {
    let runtime_model = state
        .runtime_engine
        .registry()
        .get_runtime_model(
            domain::DataModelScopeKind::Workspace,
            actor.current_workspace_id,
            model_code,
        )
        .or_else(|| {
            state.runtime_engine.registry().get_runtime_model(
                domain::DataModelScopeKind::System,
                domain::SYSTEM_SCOPE_ID,
                model_code,
            )
        })?;
    runtime_core::runtime_engine::ensure_runtime_model_available(
        &runtime_model.metadata.model_code,
        runtime_model.availability,
    )
    .ok()?;
    (runtime_model.metadata.source_kind == domain::DataModelSourceKind::MainSource)
        .then_some(runtime_model.metadata)
}

fn runtime_records_version_key(metadata: &runtime_core::model_metadata::ModelMetadata) -> String {
    format!("runtime-records:version:v1:{}", metadata.model_id)
}

async fn runtime_records_cache_version(
    state: &ApiState,
    metadata: &runtime_core::model_metadata::ModelMetadata,
) -> String {
    let key = runtime_records_version_key(metadata);
    state
        .infrastructure
        .cache_store()
        .get_json(&key)
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "0".to_string())
}

async fn bump_runtime_records_cache_version(
    state: &ApiState,
    metadata: &runtime_core::model_metadata::ModelMetadata,
) {
    let key = runtime_records_version_key(metadata);
    let _ = state
        .infrastructure
        .cache_store()
        .set_json(
            &key,
            serde_json::json!(uuid::Uuid::now_v7().to_string()),
            None,
        )
        .await;
}

fn runtime_scope_grant_cache_value(
    grant: Option<&runtime_core::runtime_acl::RuntimeScopeGrant>,
) -> serde_json::Value {
    match grant {
        Some(grant) => serde_json::json!({
            "data_model_id": grant.data_model_id,
            "scope_kind": grant.scope_kind.as_str(),
            "scope_id": grant.scope_id,
            "enabled": grant.enabled,
            "permission_profile": grant.permission_profile.as_str(),
        }),
        None => serde_json::Value::Null,
    }
}

fn runtime_model_cache_fingerprint(
    metadata: &runtime_core::model_metadata::ModelMetadata,
) -> serde_json::Value {
    let fields = metadata
        .fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "id": field.id,
                "code": field.code,
                "physical_column_name": field.physical_column_name,
                "field_kind": field.field_kind.as_str(),
                "is_system": field.is_system,
                "is_writable": field.is_writable,
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "model_id": metadata.model_id,
        "model_code": metadata.model_code,
        "scope_kind": metadata.scope_kind.as_str(),
        "scope_id": metadata.scope_id,
        "source_kind": metadata.source_kind.as_str(),
        "physical_table_name": metadata.physical_table_name,
        "scope_column_name": metadata.scope_column_name,
        "fields": fields,
    })
}

fn runtime_cache_digest(value: &serde_json::Value) -> String {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(value)
        .expect("runtime cache key payload should serialize")
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn runtime_records_list_cache_key(
    metadata: &runtime_core::model_metadata::ModelMetadata,
    credential: &RuntimeCredential,
    scope_grant: Option<&runtime_core::runtime_acl::RuntimeScopeGrant>,
    query: &RuntimeListQueryParams,
    version: &str,
) -> String {
    let payload = serde_json::json!({
        "model": runtime_model_cache_fingerprint(metadata),
        "credential": credential.cache_identity(),
        "scope_grant": runtime_scope_grant_cache_value(scope_grant),
        "query": {
            "filter": query.filter,
            "sort": query.sort,
            "expand": query.expand,
            "page": query.page.unwrap_or(1),
            "page_size": query.page_size.unwrap_or(20),
        },
        "version": version,
    });
    format!(
        "runtime-records:list:v1:{}:{}",
        metadata.model_id,
        runtime_cache_digest(&payload)
    )
}

fn runtime_records_get_cache_key(
    metadata: &runtime_core::model_metadata::ModelMetadata,
    credential: &RuntimeCredential,
    scope_grant: Option<&runtime_core::runtime_acl::RuntimeScopeGrant>,
    record_id: &str,
    version: &str,
) -> String {
    let payload = serde_json::json!({
        "model": runtime_model_cache_fingerprint(metadata),
        "credential": credential.cache_identity(),
        "scope_grant": runtime_scope_grant_cache_value(scope_grant),
        "record_id": record_id,
        "version": version,
    });
    format!(
        "runtime-records:get:v1:{}:{}",
        metadata.model_id,
        runtime_cache_digest(&payload)
    )
}

async fn cached_runtime_list_response(state: &ApiState, key: &str) -> Option<RuntimeListResponse> {
    state
        .infrastructure
        .cache_store()
        .get_json(key)
        .await
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value(value).ok())
}

async fn cache_runtime_list_response(state: &ApiState, key: &str, response: &RuntimeListResponse) {
    let Ok(value) = serde_json::to_value(response) else {
        return;
    };
    let _ = state
        .infrastructure
        .cache_store()
        .set_json(key, value, Some(time::Duration::seconds(30)))
        .await;
}

async fn cached_runtime_record(state: &ApiState, key: &str) -> Option<Value> {
    state
        .infrastructure
        .cache_store()
        .get_json(key)
        .await
        .ok()
        .flatten()
}

async fn cache_runtime_record(state: &ApiState, key: &str, record: &Value) {
    let _ = state
        .infrastructure
        .cache_store()
        .set_json(key, record.clone(), Some(time::Duration::seconds(60)))
        .await;
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ")
}

async fn authenticate_runtime_request(
    state: &ApiState,
    headers: &HeaderMap,
) -> Result<RuntimeCredential, ApiError> {
    if let Some(token) = bearer_token(headers) {
        let api_key = control_plane::auth::ApiKeyService::new(state.store.clone())
            .authenticate_bearer_token(token)
            .await?;
        return Ok(RuntimeCredential::ApiKey(Box::new(api_key)));
    }

    Ok(RuntimeCredential::Session(Box::new(
        require_session(state, headers).await?,
    )))
}

async fn append_api_key_runtime_audit(
    state: &ApiState,
    credential: &RuntimeCredential,
    model_code: &str,
    action: RuntimeModelAction,
    event_code: &str,
    reason: Option<&str>,
) -> Result<(), ApiError> {
    let RuntimeCredential::ApiKey(api_key) = credential else {
        return Ok(());
    };
    let model_id =
        resolve_runtime_model(state, credential.actor(), model_code).map(|model| model.model_id);
    let workspace_id = if api_key.actor.current_workspace_id == domain::SYSTEM_SCOPE_ID {
        None
    } else {
        Some(api_key.actor.current_workspace_id)
    };
    AuthRepository::append_audit_log(
        &state.store,
        &audit_log(
            workspace_id,
            Some(api_key.actor.user_id),
            "state_model",
            model_id,
            event_code,
            serde_json::json!({
                "api_key_id": api_key.api_key.id,
                "model_code": model_code,
                "action": action.as_str(),
                "scope_kind": api_key.api_key.scope_kind.as_str(),
                "scope_id": api_key.api_key.scope_id,
                "reason": reason,
            }),
        ),
    )
    .await?;
    Ok(())
}

async fn append_api_key_engine_acl_denied_audit(
    state: &ApiState,
    credential: &RuntimeCredential,
    model_code: &str,
    action: RuntimeModelAction,
    error: &anyhow::Error,
) -> Result<(), ApiError> {
    if let Some(reason) = runtime_acl_denial_reason(error) {
        append_api_key_runtime_audit(
            state,
            credential,
            model_code,
            action,
            "state_model.api_key_runtime_access_denied",
            Some(reason),
        )
        .await?;
    }

    Ok(())
}

async fn runtime_authorization(
    state: &ApiState,
    headers: &HeaderMap,
    model_code: &str,
    action: RuntimeModelAction,
) -> Result<
    (
        RuntimeCredential,
        Option<runtime_core::runtime_acl::RuntimeScopeGrant>,
    ),
    ApiError,
> {
    let credential = authenticate_runtime_request(state, headers).await?;
    let Some(model) = resolve_runtime_model(state, credential.actor(), model_code) else {
        return Ok((credential, None));
    };
    let scope_grant = load_runtime_scope_grant(
        state,
        credential.actor(),
        model.model_id,
        action.data_action(),
    )
    .await?;
    Ok((credential, scope_grant))
}

fn require_session_csrf_for_write(
    headers: &HeaderMap,
    credential: &RuntimeCredential,
) -> Result<(), ApiError> {
    if let RuntimeCredential::Session(context) = credential {
        require_csrf(headers, context)?;
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/runtime/models/{model_code}/list",
    params(("model_code" = String, Path, description = "Runtime model code")),
    responses((status = 200, body = RuntimeListResponse), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn list_records(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_code): Path<String>,
    Query(query): Query<RuntimeListQueryParams>,
) -> Result<Json<ApiSuccess<RuntimeListResponse>>, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(&state, &headers, &model_code, RuntimeModelAction::List).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(&state, credential.actor(), &model_code);
    let cache_key = if let Some(metadata) = &cache_metadata {
        let version = runtime_records_cache_version(&state, metadata).await;
        Some(runtime_records_list_cache_key(
            metadata,
            &credential,
            scope_grant.as_ref(),
            &query,
            &version,
        ))
    } else {
        None
    };
    if let Some(cache_key) = &cache_key {
        if let Some(response) = cached_runtime_list_response(&state, cache_key).await {
            return Ok(Json(ApiSuccess::new(runtime_list_response(
                &model_code,
                response.items,
                response.total,
            ))));
        }
    }
    let filter = parse_filter(query.filter.as_deref())?;
    let sorts = parse_sorts(query.sort.as_deref())?;
    let expand_relations = parse_expand(query.expand.as_deref());
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    let result = state
        .runtime_engine
        .list_records(runtime_core::runtime_engine::RuntimeListInput {
            actor: credential.actor().clone(),
            model_code: model_code.clone(),
            scope_grant,
            filter,
            sorts,
            expand_relations,
            page,
            page_size,
        })
        .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            append_api_key_engine_acl_denied_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::List,
                &error,
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    let response = runtime_list_response(&model_code, result.items, result.total);
    if let Some(cache_key) = &cache_key {
        cache_runtime_list_response(&state, cache_key, &response).await;
    }

    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/runtime/models/{model_code}/get/{id}",
    params(
        ("model_code" = String, Path, description = "Runtime model code"),
        ("id" = String, Path, description = "Runtime record id")
    ),
    responses((status = 200, body = RuntimeRecordEnvelope), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn get_record(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_code, record_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(&state, &headers, &model_code, RuntimeModelAction::Get).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(&state, credential.actor(), &model_code);
    let cache_key = if let Some(metadata) = &cache_metadata {
        let version = runtime_records_cache_version(&state, metadata).await;
        Some(runtime_records_get_cache_key(
            metadata,
            &credential,
            scope_grant.as_ref(),
            &record_id,
            &version,
        ))
    } else {
        None
    };
    if let Some(cache_key) = &cache_key {
        if let Some(record) = cached_runtime_record(&state, cache_key).await {
            return Ok(Json(ApiSuccess::new(runtime_record_response(
                &model_code,
                record,
            ))));
        }
    }
    let record = state
        .runtime_engine
        .get_record(runtime_core::runtime_engine::RuntimeGetInput {
            actor: credential.actor().clone(),
            model_code: model_code.clone(),
            record_id,
            scope_grant,
        })
        .await;
    let record = match record {
        Ok(record) => record.ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "runtime_record",
        ))?,
        Err(error) => {
            append_api_key_engine_acl_denied_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Get,
                &error,
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };
    let record = runtime_record_response(&model_code, record);
    if let Some(cache_key) = &cache_key {
        cache_runtime_record(&state, cache_key, &record).await;
    }

    Ok(Json(ApiSuccess::new(record)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runtime_record_response_rounds_application_log_cache_hit_rate() {
        let record = runtime_record_response(
            "application_run_log_summaries",
            json!({
                "id": "run-1",
                "total_tokens": 49901,
                "input_cache_hit_tokens": 49063,
                "input_cache_hit_rate": 0.9505703422053232
            }),
        );

        assert_eq!(record["input_cache_hit_rate"], json!(0.9832));
    }

    #[test]
    fn runtime_record_response_does_not_fall_back_to_projected_cache_hit_rate() {
        let record = runtime_record_response(
            "application_run_log_summaries",
            json!({
                "id": "run-1",
                "input_cache_hit_rate": 1.0
            }),
        );

        assert_eq!(record["input_cache_hit_rate"], Value::Null);
    }

    #[test]
    fn runtime_record_response_leaves_other_models_unchanged() {
        let record = runtime_record_response(
            "orders",
            json!({
                "id": "order-1",
                "input_cache_hit_rate": 0.9505703422053232
            }),
        );

        assert_eq!(record["input_cache_hit_rate"], json!(0.9505703422053232));
    }
}

#[utoipa::path(
    post,
    path = "/api/runtime/models/{model_code}/create",
    request_body = RuntimeRecordEnvelope,
    params(("model_code" = String, Path, description = "Runtime model code")),
    responses((status = 201, body = RuntimeRecordEnvelope), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn create_record(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(model_code): Path<String>,
    Json(payload): Json<Value>,
) -> Result<(StatusCode, Json<ApiSuccess<Value>>), ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(&state, &headers, &model_code, RuntimeModelAction::Create).await?;
    require_session_csrf_for_write(&headers, &credential)?;
    let cache_metadata =
        runtime_records_cacheable_metadata(&state, credential.actor(), &model_code);

    let result = state
        .runtime_engine
        .create_record(runtime_core::runtime_engine::RuntimeCreateInput {
            actor: credential.actor().clone(),
            model_code: model_code.clone(),
            payload,
            scope_grant,
        })
        .await;
    let record = match result {
        Ok(record) => {
            append_api_key_runtime_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Create,
                "state_model.api_key_runtime_write_succeeded",
                None,
            )
            .await?;
            if let Some(metadata) = &cache_metadata {
                bump_runtime_records_cache_version(&state, metadata).await;
            }
            record
        }
        Err(error) => {
            let reason = error.to_string();
            append_api_key_engine_acl_denied_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Create,
                &error,
            )
            .await?;
            append_api_key_runtime_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Create,
                "state_model.api_key_runtime_write_failed",
                Some(&reason),
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    Ok((StatusCode::CREATED, Json(ApiSuccess::new(record))))
}

#[utoipa::path(
    patch,
    path = "/api/runtime/models/{model_code}/update/{id}",
    request_body = RuntimeRecordEnvelope,
    params(
        ("model_code" = String, Path, description = "Runtime model code"),
        ("id" = String, Path, description = "Runtime record id")
    ),
    responses((status = 200, body = RuntimeRecordEnvelope), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody))
)]
pub async fn update_record(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_code, record_id)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(&state, &headers, &model_code, RuntimeModelAction::Update).await?;
    require_session_csrf_for_write(&headers, &credential)?;
    let cache_metadata =
        runtime_records_cacheable_metadata(&state, credential.actor(), &model_code);

    let result = state
        .runtime_engine
        .update_record(runtime_core::runtime_engine::RuntimeUpdateInput {
            actor: credential.actor().clone(),
            model_code: model_code.clone(),
            record_id,
            payload,
            scope_grant,
        })
        .await;
    let record = match result {
        Ok(record) => {
            append_api_key_runtime_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Update,
                "state_model.api_key_runtime_write_succeeded",
                None,
            )
            .await?;
            if let Some(metadata) = &cache_metadata {
                bump_runtime_records_cache_version(&state, metadata).await;
            }
            record
        }
        Err(error) => {
            let reason = error.to_string();
            append_api_key_engine_acl_denied_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Update,
                &error,
            )
            .await?;
            append_api_key_runtime_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Update,
                "state_model.api_key_runtime_write_failed",
                Some(&reason),
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    Ok(Json(ApiSuccess::new(record)))
}

#[utoipa::path(
    delete,
    path = "/api/runtime/models/{model_code}/delete/{id}",
    params(
        ("model_code" = String, Path, description = "Runtime model code"),
        ("id" = String, Path, description = "Runtime record id")
    ),
    responses((status = 200, body = RuntimeRecordEnvelope), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn delete_record(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_code, record_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<Value>>, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(&state, &headers, &model_code, RuntimeModelAction::Delete).await?;
    require_session_csrf_for_write(&headers, &credential)?;
    let cache_metadata =
        runtime_records_cacheable_metadata(&state, credential.actor(), &model_code);

    let delete_result = state
        .runtime_engine
        .delete_record(runtime_core::runtime_engine::RuntimeDeleteInput {
            actor: credential.actor().clone(),
            model_code: model_code.clone(),
            record_id,
            scope_grant,
        })
        .await;
    let result = match delete_result {
        Ok(result) => {
            append_api_key_runtime_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Delete,
                "state_model.api_key_runtime_write_succeeded",
                None,
            )
            .await?;
            if let Some(metadata) = &cache_metadata {
                bump_runtime_records_cache_version(&state, metadata).await;
            }
            result
        }
        Err(error) => {
            let reason = error.to_string();
            append_api_key_engine_acl_denied_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Delete,
                &error,
            )
            .await?;
            append_api_key_runtime_audit(
                &state,
                &credential,
                &model_code,
                RuntimeModelAction::Delete,
                "state_model.api_key_runtime_write_failed",
                Some(&reason),
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    Ok(Json(ApiSuccess::new(result)))
}
