use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};

use axum::{
    body::Bytes,
    extract::{OriginalUri, Path, State},
    http::{HeaderMap, Method},
    response::Response,
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::{ApiError, ApiServiceUnavailable},
};
use control_plane::resource_crud::parse_resource_filter_expr;
use control_plane::{
    audit::audit_log,
    ports::{AuthRepository, CacheStore, OrchestrationRuntimeRepository},
};
use interface_runtime::{UserCredentialKind, UserPrincipal};
use runtime_core::runtime_engine::RuntimeEngine;
use storage_durable_postgres::MainDurableStore;

mod error_mapping;
mod interface;

use error_mapping::{map_runtime_error, runtime_acl_denial_reason};

#[derive(Debug, Clone)]
struct ResolvedRuntimeOperation {
    handler: ResolvedRuntimeOperationHandler,
    operation_code: String,
    data_action: runtime_core::runtime_acl::RuntimeDataAction,
    path_parameters: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
enum ResolvedRuntimeOperationHandler {
    Core(runtime_core::general_data_model_template::CoreGeneralOperationHandler),
    Ordered(runtime_core::ordered_tree_template::CoreOrderedTreeOperationHandler),
    External,
}

impl ResolvedRuntimeOperation {
    fn audit_action(&self) -> &str {
        match &self.handler {
            ResolvedRuntimeOperationHandler::Core(handler) => handler.audit_action(),
            ResolvedRuntimeOperationHandler::Ordered(handler) => handler.audit_action(),
            ResolvedRuntimeOperationHandler::External => &self.operation_code,
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

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub(crate) fn route_assembly(
) -> crate::external_route_assembly::ExternalRouteAssembly<Arc<ApiState>> {
    crate::external_route_assembly::ExternalRouteAssembly::new().route(
        "/models/:model_code/*operation_path",
        crate::external_route_assembly::any(dispatch_runtime_operation),
    )
}

struct RuntimeModelOperationAdapter {
    store: MainDurableStore,
    runtime_engine: Arc<RuntimeEngine>,
    cache_store: Arc<dyn CacheStore>,
}

pub(crate) fn runtime_model_operation_port(
    store: MainDurableStore,
    runtime_engine: Arc<RuntimeEngine>,
    cache_store: Arc<dyn CacheStore>,
) -> Arc<dyn interface::RuntimeModelOperationPort> {
    Arc::new(RuntimeModelOperationAdapter {
        store,
        runtime_engine,
        cache_store,
    })
}

pub(crate) fn compile_runtime_model_interface_registry(
    port: Arc<dyn interface::RuntimeModelOperationPort>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    interface::compile_registry(port)
}

#[cfg(test)]
pub(crate) fn compile_runtime_model_interface_registry_for_test() -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    interface::compile_registry_for_test()
}

impl interface::RuntimeModelOperationPort for RuntimeModelOperationAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: interface::RuntimeModelOperationInput,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<
                        interface::RuntimeModelOperationOutput,
                        interface::RuntimeModelOperationTargetError,
                    >,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            execute_runtime_model_operation(self, principal, input)
                .await
                .map_err(interface::RuntimeModelOperationTargetError)
        })
    }
}

async fn execute_runtime_model_operation(
    adapter: &RuntimeModelOperationAdapter,
    principal: &UserPrincipal,
    input: interface::RuntimeModelOperationInput,
) -> Result<interface::RuntimeModelOperationOutput, ApiError> {
    let resolved = resolve_runtime_operation_for_dependencies(
        adapter,
        principal.actor(),
        input.method,
        &input.model_code,
        &input.path,
    )
    .map_err(map_runtime_error)?;
    let credential = RuntimeCredential::from_principal(principal);
    let record_id = match resolved.handler {
        ResolvedRuntimeOperationHandler::Core(
            runtime_core::general_data_model_template::CoreGeneralOperationHandler::GetRecord
            | runtime_core::general_data_model_template::CoreGeneralOperationHandler::UpdateRecord
            | runtime_core::general_data_model_template::CoreGeneralOperationHandler::DeleteRecord,
        )
        | ResolvedRuntimeOperationHandler::Ordered(
            runtime_core::ordered_tree_template::CoreOrderedTreeOperationHandler::GetRecord,
        ) => Some(resolved.path_parameters.get("id").cloned().ok_or(
            control_plane::errors::ControlPlaneError::InvalidInput("runtime_path"),
        )?),
        _ => None,
    };
    let status_and_data = match resolved.handler {
        ResolvedRuntimeOperationHandler::Core(
            runtime_core::general_data_model_template::CoreGeneralOperationHandler::ListRecords,
        )
        | ResolvedRuntimeOperationHandler::Ordered(
            runtime_core::ordered_tree_template::CoreOrderedTreeOperationHandler::ListRecords,
        ) => {
            let query = parse_runtime_list_query(input.query.as_deref())?;
            let data = serde_json::to_value(
                list_records(adapter, credential, input.model_code, query, &resolved).await?,
            )?;
            (interface::RuntimeModelOperationStatus::Ok, data)
        }
        ResolvedRuntimeOperationHandler::Core(
            runtime_core::general_data_model_template::CoreGeneralOperationHandler::GetRecord,
        )
        | ResolvedRuntimeOperationHandler::Ordered(
            runtime_core::ordered_tree_template::CoreOrderedTreeOperationHandler::GetRecord,
        ) => (
            interface::RuntimeModelOperationStatus::Ok,
            get_record(
                adapter,
                credential,
                input.model_code,
                record_id.unwrap_or_default(),
                &resolved,
            )
            .await?,
        ),
        ResolvedRuntimeOperationHandler::Core(
            runtime_core::general_data_model_template::CoreGeneralOperationHandler::CreateRecord,
        ) => (
            interface::RuntimeModelOperationStatus::Created,
            create_record(
                adapter,
                credential,
                input.model_code,
                parse_runtime_payload(&input.body)?,
                &resolved,
            )
            .await?,
        ),
        ResolvedRuntimeOperationHandler::Core(
            runtime_core::general_data_model_template::CoreGeneralOperationHandler::UpdateRecord,
        ) => (
            interface::RuntimeModelOperationStatus::Ok,
            update_record(
                adapter,
                credential,
                input.model_code,
                record_id.unwrap_or_default(),
                parse_runtime_payload(&input.body)?,
                &resolved,
            )
            .await?,
        ),
        ResolvedRuntimeOperationHandler::Core(
            runtime_core::general_data_model_template::CoreGeneralOperationHandler::DeleteRecord,
        ) => (
            interface::RuntimeModelOperationStatus::Ok,
            delete_record(
                adapter,
                credential,
                input.model_code,
                record_id.unwrap_or_default(),
                &resolved,
            )
            .await?,
        ),
        ResolvedRuntimeOperationHandler::Ordered(_) | ResolvedRuntimeOperationHandler::External => {
            execute_descriptor_model_operation(
                adapter,
                credential,
                input.model_code,
                input.query.as_deref(),
                &input.body,
                &resolved,
            )
            .await?
        }
    };
    Ok(interface::RuntimeModelOperationOutput {
        status: status_and_data.0,
        data: status_and_data.1,
    })
}

fn parse_runtime_payload(body: &[u8]) -> Result<Value, ApiError> {
    serde_json::from_slice(body)
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("payload").into())
}

fn parse_runtime_query(raw: Option<&str>) -> std::collections::BTreeMap<String, String> {
    form_urlencoded::parse(raw.unwrap_or_default().as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn parse_runtime_list_query(raw: Option<&str>) -> Result<RuntimeListQueryParams, ApiError> {
    let mut values = parse_runtime_query(raw);
    let parse_number = |value: Option<String>| -> Result<Option<i64>, ApiError> {
        value
            .map(|value| {
                value.parse::<i64>().map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput("query").into()
                })
            })
            .transpose()
    };
    Ok(RuntimeListQueryParams {
        filter: values.remove("filter"),
        sort: values.remove("sort"),
        expand: values.remove("expand"),
        page: parse_number(values.remove("page"))?,
        page_size: parse_number(values.remove("page_size"))?,
    })
}

async fn dispatch_runtime_operation(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((model_code, _operation_path)): Path<(String, String)>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Response {
    interface::invoke(state, headers, method, model_code, uri, body).await
}

fn resolve_runtime_operation_for_actor(
    state: &ApiState,
    actor: &domain::ActorContext,
    method: plugin_framework::DataModelOperationMethod,
    model_code: &str,
    path: &str,
) -> anyhow::Result<ResolvedRuntimeOperation> {
    resolve_runtime_operation_with_engine(
        state.runtime_engine.as_ref(),
        actor,
        method,
        model_code,
        path,
    )
}

fn resolve_runtime_operation_for_dependencies(
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    method: plugin_framework::DataModelOperationMethod,
    model_code: &str,
    path: &str,
) -> anyhow::Result<ResolvedRuntimeOperation> {
    resolve_runtime_operation_with_engine(
        adapter.runtime_engine.as_ref(),
        actor,
        method,
        model_code,
        path,
    )
}

fn resolve_runtime_operation_with_engine(
    runtime_engine: &RuntimeEngine,
    actor: &domain::ActorContext,
    method: plugin_framework::DataModelOperationMethod,
    model_code: &str,
    path: &str,
) -> anyhow::Result<ResolvedRuntimeOperation> {
    let template = runtime_engine.template_for_model(model_code, actor.current_workspace_id)?;
    let matched = template.match_operation(method, path).ok_or(
        control_plane::errors::ControlPlaneError::NotFound("runtime_operation"),
    )?;
    let handler = runtime_core::general_data_model_template::CoreGeneralOperationHandler::from_ref(
        &matched.operation.handler_ref,
    )
    .map(ResolvedRuntimeOperationHandler::Core)
    .or_else(|| {
        runtime_core::ordered_tree_template::CoreOrderedTreeOperationHandler::from_ref(
            &matched.operation.handler_ref,
        )
        .map(ResolvedRuntimeOperationHandler::Ordered)
    })
    .unwrap_or(ResolvedRuntimeOperationHandler::External);
    let data_action = runtime_core::general_data_model_template::runtime_data_action(
        &matched.operation.permission_action,
    )
    .ok_or(control_plane::errors::ControlPlaneError::Conflict(
        "data_model_template_permission_unavailable",
    ))?;
    Ok(ResolvedRuntimeOperation {
        handler,
        operation_code: matched.operation.code.clone(),
        data_action,
        path_parameters: matched.path_parameters,
    })
}

pub(crate) fn runtime_operation_requires_csrf(
    state: &ApiState,
    actor: &domain::ActorContext,
    method: plugin_framework::DataModelOperationMethod,
    model_code: &str,
    path: &str,
) -> bool {
    // Authentication owns credential validation, not runtime-model availability projection.
    // When resolution rejects the operation, the typed Handler repeats the same pure lookup and
    // returns the established model/operation error before any side effect can execute.
    resolve_runtime_operation_for_actor(state, actor, method, model_code, path).is_ok_and(
        |operation| operation.data_action != runtime_core::runtime_acl::RuntimeDataAction::View,
    )
}

async fn execute_descriptor_model_operation(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: String,
    raw_query: Option<&str>,
    body: &[u8],
    operation: &ResolvedRuntimeOperation,
) -> Result<(interface::RuntimeModelOperationStatus, Value), ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(adapter, credential, &model_code, operation).await?;
    let payload = if body.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(body)
            .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("payload"))?
    };
    let query = serde_json::to_value(parse_runtime_query(raw_query))?;
    let result = adapter
        .runtime_engine
        .execute_model_operation(runtime_core::runtime_engine::RuntimeModelOperationInput {
            actor: credential.actor().clone(),
            model_code: model_code.clone(),
            operation_code: operation.operation_code.clone(),
            payload,
            path: serde_json::to_value(&operation.path_parameters)?,
            query,
            scope_grant,
        })
        .await;
    if let Err(error) = &result {
        append_api_key_engine_acl_denied_audit(adapter, &credential, &model_code, operation, error)
            .await?;
    }
    let output = result.map_err(map_runtime_error)?;
    // Descriptor-backed writes (including ordered-tree CRUD, moves and subtree deletes)
    // share the generic get/list caches. Invalidate after commit, before optional audit work.
    if operation.data_action != runtime_core::runtime_acl::RuntimeDataAction::View {
        if let Some(metadata) =
            runtime_records_cacheable_metadata(adapter, credential.actor(), &model_code)
        {
            bump_runtime_records_cache_version(adapter, &metadata).await;
        }
    }
    append_api_key_runtime_audit(
        adapter,
        &credential,
        &model_code,
        operation,
        "state_model.api_key_runtime_operation_executed",
        None,
    )
    .await?;
    let status = if operation.operation_code == "create_record" {
        interface::RuntimeModelOperationStatus::Created
    } else {
        interface::RuntimeModelOperationStatus::Ok
    };
    Ok((status, output))
}

fn descriptor_method(method: &Method) -> Option<plugin_framework::DataModelOperationMethod> {
    match *method {
        Method::GET => Some(plugin_framework::DataModelOperationMethod::Get),
        Method::POST => Some(plugin_framework::DataModelOperationMethod::Post),
        Method::PUT => Some(plugin_framework::DataModelOperationMethod::Put),
        Method::PATCH => Some(plugin_framework::DataModelOperationMethod::Patch),
        Method::DELETE => Some(plugin_framework::DataModelOperationMethod::Delete),
        _ => None,
    }
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
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    data_model_id: uuid::Uuid,
    action: runtime_core::runtime_acl::RuntimeDataAction,
) -> Result<Option<runtime_core::runtime_acl::RuntimeScopeGrant>, ApiError> {
    Ok(
        control_plane::model_definition::ModelDefinitionService::new(adapter.store.clone())
            .load_runtime_scope_grant(actor, data_model_id, action)
            .await?,
    )
}

fn resolve_runtime_model(
    adapter: &RuntimeModelOperationAdapter,
    actor: &domain::ActorContext,
    model_code: &str,
) -> Option<runtime_core::model_metadata::ModelMetadata> {
    adapter
        .runtime_engine
        .registry()
        .get(
            domain::DataModelScopeKind::Workspace,
            actor.current_workspace_id,
            model_code,
        )
        .or_else(|| {
            adapter.runtime_engine.registry().get(
                domain::DataModelScopeKind::System,
                domain::SYSTEM_SCOPE_ID,
                model_code,
            )
        })
}

enum RuntimeCredential {
    Session(domain::ActorContext),
    ApiKey {
        api_key_id: Uuid,
        actor: domain::ActorContext,
    },
}

impl RuntimeCredential {
    fn from_principal(principal: &UserPrincipal) -> Self {
        match principal.credential_kind() {
            UserCredentialKind::UserApiKey { api_key_id } => Self::ApiKey {
                api_key_id,
                actor: principal.actor().clone(),
            },
            UserCredentialKind::CookieSession | UserCredentialKind::ServerDelegation => {
                Self::Session(principal.actor().clone())
            }
        }
    }

    fn actor(&self) -> &domain::ActorContext {
        match self {
            Self::Session(actor) | Self::ApiKey { actor, .. } => actor,
        }
    }

    fn cache_identity(&self) -> serde_json::Value {
        let actor = self.actor();
        let mut permissions = actor.permissions.iter().cloned().collect::<Vec<_>>();
        permissions.sort();

        match self {
            Self::Session(_) => serde_json::json!({
                "kind": "session",
                "user_id": actor.user_id,
                "tenant_id": actor.tenant_id,
                "workspace_id": actor.current_workspace_id,
                "role": actor.effective_display_role,
                "is_root": actor.is_root,
                "permissions": permissions,
            }),
            Self::ApiKey { api_key_id, .. } => serde_json::json!({
                "kind": "api_key",
                "api_key_id": api_key_id,
                "key_kind": "user_api_key",
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

async fn append_api_key_runtime_audit(
    adapter: &RuntimeModelOperationAdapter,
    credential: &RuntimeCredential,
    model_code: &str,
    operation: &ResolvedRuntimeOperation,
    event_code: &str,
    reason: Option<&str>,
) -> Result<(), ApiError> {
    let RuntimeCredential::ApiKey { api_key_id, actor } = credential else {
        return Ok(());
    };
    let model_id =
        resolve_runtime_model(adapter, credential.actor(), model_code).map(|model| model.model_id);
    let workspace_id = if actor.current_workspace_id == domain::SYSTEM_SCOPE_ID {
        None
    } else {
        Some(actor.current_workspace_id)
    };
    AuthRepository::append_audit_log(
        &adapter.store,
        &audit_log(
            workspace_id,
            Some(actor.user_id),
            "state_model",
            model_id,
            event_code,
            serde_json::json!({
                "api_key_id": api_key_id,
                "model_code": model_code,
                "action": operation.audit_action(),
                "scope_kind": if actor.current_workspace_id == domain::SYSTEM_SCOPE_ID { "system" } else { "workspace" },
                "scope_id": actor.current_workspace_id,
                "reason": reason,
            }),
        ),
    )
    .await?;
    Ok(())
}

async fn append_api_key_engine_acl_denied_audit(
    adapter: &RuntimeModelOperationAdapter,
    credential: &RuntimeCredential,
    model_code: &str,
    operation: &ResolvedRuntimeOperation,
    error: &anyhow::Error,
) -> Result<(), ApiError> {
    if let Some(reason) = runtime_acl_denial_reason(error) {
        append_api_key_runtime_audit(
            adapter,
            credential,
            model_code,
            operation,
            "state_model.api_key_runtime_access_denied",
            Some(reason),
        )
        .await?;
    }

    Ok(())
}

async fn runtime_authorization(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: &str,
    operation: &ResolvedRuntimeOperation,
) -> Result<
    (
        RuntimeCredential,
        Option<runtime_core::runtime_acl::RuntimeScopeGrant>,
    ),
    ApiError,
> {
    let Some(model) = resolve_runtime_model(adapter, credential.actor(), model_code) else {
        return Ok((credential, None));
    };
    let scope_grant = load_runtime_scope_grant(
        adapter,
        credential.actor(),
        model.model_id,
        operation.data_action,
    )
    .await?;
    Ok((credential, scope_grant))
}

async fn list_records(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: String,
    query: RuntimeListQueryParams,
    operation: &ResolvedRuntimeOperation,
) -> Result<RuntimeListResponse, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(adapter, credential, &model_code, operation).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(adapter, credential.actor(), &model_code);
    let cache_key = if let Some(metadata) = &cache_metadata {
        let version = runtime_records_cache_version(adapter, metadata).await;
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
        if let Some(response) = cached_runtime_list_response(adapter, cache_key).await {
            let mut response = runtime_list_response(&model_code, response.items, response.total);
            enrich_application_run_count_tokens_results(adapter, &model_code, &mut response.items)
                .await?;
            return Ok(response);
        }
    }
    let filter = parse_filter(query.filter.as_deref())?;
    let sorts = parse_sorts(query.sort.as_deref())?;
    let expand_relations = parse_expand(query.expand.as_deref());
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    let result = adapter
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
                adapter,
                &credential,
                &model_code,
                operation,
                &error,
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    let mut response = runtime_list_response(&model_code, result.items, result.total);
    enrich_application_run_count_tokens_results(adapter, &model_code, &mut response.items).await?;
    if let Some(cache_key) = &cache_key {
        cache_runtime_list_response(adapter, cache_key, &response).await;
    }

    Ok(response)
}

async fn get_record(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: String,
    record_id: String,
    operation: &ResolvedRuntimeOperation,
) -> Result<Value, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(adapter, credential, &model_code, operation).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(adapter, credential.actor(), &model_code);
    let cache_key = if let Some(metadata) = &cache_metadata {
        let version = runtime_records_cache_version(adapter, metadata).await;
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
        if let Some(record) = cached_runtime_record(adapter, cache_key).await {
            let mut record = runtime_record_response(&model_code, record);
            enrich_application_run_count_tokens_results(
                adapter,
                &model_code,
                std::slice::from_mut(&mut record),
            )
            .await?;
            return Ok(record);
        }
    }
    let record = adapter
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
                adapter,
                &credential,
                &model_code,
                operation,
                &error,
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };
    let mut record = runtime_record_response(&model_code, record);
    enrich_application_run_count_tokens_results(
        adapter,
        &model_code,
        std::slice::from_mut(&mut record),
    )
    .await?;
    if let Some(cache_key) = &cache_key {
        cache_runtime_record(adapter, cache_key, &record).await;
    }

    Ok(record)
}

async fn create_record(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: String,
    payload: Value,
    operation: &ResolvedRuntimeOperation,
) -> Result<Value, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(adapter, credential, &model_code, operation).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(adapter, credential.actor(), &model_code);

    let result = adapter
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
                adapter,
                &credential,
                &model_code,
                operation,
                "state_model.api_key_runtime_write_succeeded",
                None,
            )
            .await?;
            if let Some(metadata) = &cache_metadata {
                bump_runtime_records_cache_version(adapter, metadata).await;
            }
            record
        }
        Err(error) => {
            let reason = error.to_string();
            append_api_key_engine_acl_denied_audit(
                adapter,
                &credential,
                &model_code,
                operation,
                &error,
            )
            .await?;
            append_api_key_runtime_audit(
                adapter,
                &credential,
                &model_code,
                operation,
                "state_model.api_key_runtime_write_failed",
                Some(&reason),
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    Ok(record)
}

async fn update_record(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: String,
    record_id: String,
    payload: Value,
    operation: &ResolvedRuntimeOperation,
) -> Result<Value, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(adapter, credential, &model_code, operation).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(adapter, credential.actor(), &model_code);

    let result = adapter
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
                adapter,
                &credential,
                &model_code,
                operation,
                "state_model.api_key_runtime_write_succeeded",
                None,
            )
            .await?;
            if let Some(metadata) = &cache_metadata {
                bump_runtime_records_cache_version(adapter, metadata).await;
            }
            record
        }
        Err(error) => {
            let reason = error.to_string();
            append_api_key_engine_acl_denied_audit(
                adapter,
                &credential,
                &model_code,
                operation,
                &error,
            )
            .await?;
            append_api_key_runtime_audit(
                adapter,
                &credential,
                &model_code,
                operation,
                "state_model.api_key_runtime_write_failed",
                Some(&reason),
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    Ok(record)
}

async fn delete_record(
    adapter: &RuntimeModelOperationAdapter,
    credential: RuntimeCredential,
    model_code: String,
    record_id: String,
    operation: &ResolvedRuntimeOperation,
) -> Result<Value, ApiError> {
    let (credential, scope_grant) =
        runtime_authorization(adapter, credential, &model_code, operation).await?;
    let cache_metadata =
        runtime_records_cacheable_metadata(adapter, credential.actor(), &model_code);

    let delete_result = adapter
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
                adapter,
                &credential,
                &model_code,
                operation,
                "state_model.api_key_runtime_write_succeeded",
                None,
            )
            .await?;
            if let Some(metadata) = &cache_metadata {
                bump_runtime_records_cache_version(adapter, metadata).await;
            }
            result
        }
        Err(error) => {
            let reason = error.to_string();
            append_api_key_engine_acl_denied_audit(
                adapter,
                &credential,
                &model_code,
                operation,
                &error,
            )
            .await?;
            append_api_key_runtime_audit(
                adapter,
                &credential,
                &model_code,
                operation,
                "state_model.api_key_runtime_write_failed",
                Some(&reason),
            )
            .await?;
            return Err(map_runtime_error(error));
        }
    };

    Ok(result)
}

#[cfg(test)]
mod tests;

#[path = "runtime_models/record_cache.rs"]
mod record_cache;

#[path = "runtime_models/run_observability.rs"]
mod run_observability;

use record_cache::*;

use run_observability::*;
