use std::{collections::BTreeMap, sync::Arc};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    frontstage::{FrontstagePageService, GetFrontstagePageDetailCommand},
    ports::FrontstagePageRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    openapi_interface::{
        build_openapi_capability_catalog, DispatchArguments, DispatchError,
        OpenApiCapabilitySource, OpenApiInterfaceCatalogEntry, OpenApiParameterLocation,
    },
    response::ApiSuccess,
};

const WRITE_GRANT_TTL: Duration = Duration::minutes(5);
const WRITE_GRANT_LOCK_TTL: Duration = Duration::seconds(10);
const WRITE_GRANT_CACHE_PREFIX: &str = "frontstage:callable-write-grant:";
const WRITE_GRANT_LOCK_PREFIX: &str = "frontstage:callable-write-grant-lock:";

#[cfg(test)]
const PAGE_TAB_GET_OPERATION_ID: &str = "get_frontstage_page_detail";
#[cfg(test)]
const PAGE_TAB_SAVE_OPERATION_ID: &str = "save_frontstage_tab_document";

#[derive(Clone)]
struct RegisteredCallable {
    interface: OpenApiInterfaceCatalogEntry,
    source: OpenApiCapabilitySource,
    bindable: bool,
    disabled_reason: Option<&'static str>,
    host_injected_parameters: Vec<&'static str>,
    scope: &'static str,
    authorization: &'static str,
    risk_level: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageInterfaceCapabilityResponse {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub short_description: String,
    #[schema(value_type = Object)]
    pub parameter_schema: Value,
    #[schema(value_type = Object)]
    pub result_schema: Value,
    pub request_media_type: Option<String>,
    pub response_media_type: Option<String>,
    pub schema_digest: String,
    pub adapter_id: String,
    pub host_injected_parameters: Vec<String>,
    pub scope: String,
    pub risk_level: String,
    pub authorization: String,
    pub bindable: bool,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DispatchFrontstageCallableBody {
    pub block_id: String,
    pub binding_alias: String,
    pub schema_digest: String,
    pub run_id: String,
    pub draft_hash: String,
    #[serde(default)]
    pub request: DispatchArguments,
    pub write_grant: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct IssueFrontstageCallableWriteGrantBody {
    pub block_id: String,
    pub binding_alias: String,
    pub schema_digest: String,
    pub run_id: String,
    pub draft_hash: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageCallableWriteGrantResponse {
    pub grant_token: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FrontstageCallableWriteGrant {
    actor_user_id: Uuid,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: String,
    binding_alias: String,
    run_id: String,
    draft_hash: String,
    operation_id: String,
    expires_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
struct FrontstageCallableBinding {
    alias: String,
    operation_id: String,
    schema_digest: String,
    scope: String,
    risk_level: String,
}

#[derive(Debug, Deserialize)]
struct FrontstageCallableBlock {
    id: String,
    #[serde(default)]
    interfaces: Vec<FrontstageCallableBinding>,
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/interface-capabilities",
    params(("workspace_id" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = [FrontstageInterfaceCapabilityResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_interface_capabilities(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<FrontstageInterfaceCapabilityResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    let actor = state
        .store
        .load_actor_context_for_workspace(context.user.id, workspace_id)
        .await?;
    if !actor.has_permission("frontstage.page.design") {
        return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
    }
    let entries = registered_callables(&state, workspace_id)
        .await?
        .into_iter()
        .map(to_response)
        .collect();
    Ok(Json(ApiSuccess::new(entries)))
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch",
    request_body = DispatchFrontstageCallableBody,
    params(
        ("workspace_id" = String, Path, description = "Workspace id"),
        ("page_id" = String, Path, description = "Page id"),
        ("tab_id" = String, Path, description = "Tab id")
    ),
    responses(
        (status = 200, body = Object),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn dispatch_frontstage_callable_interface(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, tab_id)): Path<(String, String, String)>,
    Json(body): Json<DispatchFrontstageCallableBody>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    let page_id = super::parse_uuid(&page_id, "page_id")?;
    let tab_id = super::parse_uuid(&tab_id, "tab_id")?;

    let detail = FrontstagePageService::new(state.store.clone())
        .get_page_detail(GetFrontstagePageDetailCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            tab_reference: tab_id.to_string(),
        })
        .await?;

    let callable = resolve_bound_callable(
        &state,
        workspace_id,
        &detail.document.payload,
        &body.block_id,
        &body.binding_alias,
        &body.schema_digest,
    )
    .await?;
    if callable.risk_level != "low" {
        let Some(grant_token) = body.write_grant.as_deref() else {
            return Err(ControlPlaneError::InvalidInput("write_grant").into());
        };
        consume_write_grant(
            &state,
            grant_token,
            &FrontstageCallableWriteGrant {
                actor_user_id: context.user.id,
                workspace_id,
                page_id,
                tab_id,
                block_id: body.block_id.clone(),
                binding_alias: body.binding_alias.clone(),
                run_id: body.run_id.clone(),
                draft_hash: body.draft_hash.clone(),
                operation_id: callable.interface.operation_id.clone(),
                expires_at: OffsetDateTime::UNIX_EPOCH,
            },
        )
        .await?;
    }

    let injected_path = injected_path_parameters(
        &callable.host_injected_parameters,
        workspace_id,
        page_id,
        tab_id,
    );

    match crate::openapi_interface::dispatch(
        state,
        &headers,
        &callable.interface,
        body.request,
        injected_path,
    )
    .await
    {
        Ok(crate::openapi_interface::DispatchSuccess::Json(value)) => {
            Ok(Json(ApiSuccess::new(value.get("data").cloned().unwrap_or(value))).into_response())
        }
        Ok(crate::openapi_interface::DispatchSuccess::NoContent) => {
            Ok(StatusCode::NO_CONTENT.into_response())
        }
        Ok(crate::openapi_interface::DispatchSuccess::Media(response)) => Ok(response),
        Err(DispatchError::Api(error)) => Err(error.into()),
        Err(DispatchError::Target(response)) => Ok(response),
    }
}

#[utoipa::path(
    post,
    path = "/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/callable-interfaces/write-grants",
    request_body = IssueFrontstageCallableWriteGrantBody,
    params(
        ("workspace_id" = String, Path, description = "Workspace id"),
        ("page_id" = String, Path, description = "Page id"),
        ("tab_id" = String, Path, description = "Tab id")
    ),
    responses(
        (status = 200, body = FrontstageCallableWriteGrantResponse),
        (status = 400, body = crate::error_response::ErrorBody),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn issue_frontstage_callable_write_grant(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, page_id, tab_id)): Path<(String, String, String)>,
    Json(body): Json<IssueFrontstageCallableWriteGrantBody>,
) -> Result<Json<ApiSuccess<FrontstageCallableWriteGrantResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    let page_id = super::parse_uuid(&page_id, "page_id")?;
    let tab_id = super::parse_uuid(&tab_id, "tab_id")?;
    let detail = FrontstagePageService::new(state.store.clone())
        .get_page_detail(GetFrontstagePageDetailCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            tab_reference: tab_id.to_string(),
        })
        .await?;
    let callable = resolve_bound_callable(
        &state,
        workspace_id,
        &detail.document.payload,
        &body.block_id,
        &body.binding_alias,
        &body.schema_digest,
    )
    .await?;
    if callable.risk_level == "low" {
        return Err(ControlPlaneError::InvalidInput("binding_alias").into());
    }
    if body.run_id.trim().is_empty() || body.draft_hash.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("draft_run").into());
    }

    let grant_token = Uuid::new_v4().to_string();
    let expires_at = OffsetDateTime::now_utc() + WRITE_GRANT_TTL;
    let grant = FrontstageCallableWriteGrant {
        actor_user_id: context.user.id,
        workspace_id,
        page_id,
        tab_id,
        block_id: body.block_id,
        binding_alias: body.binding_alias,
        run_id: body.run_id,
        draft_hash: body.draft_hash,
        operation_id: callable.interface.operation_id,
        expires_at,
    };
    state
        .infrastructure
        .cache_store()
        .set_if_absent_json(
            &write_grant_cache_key(&grant_token),
            serde_json::to_value(grant)?,
            Some(WRITE_GRANT_TTL),
        )
        .await?
        .then_some(())
        .ok_or(ControlPlaneError::Conflict(
            "frontstage_callable_write_grant",
        ))?;

    Ok(Json(ApiSuccess::new(
        FrontstageCallableWriteGrantResponse {
            grant_token,
            expires_at: expires_at
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(anyhow::Error::from)?,
        },
    )))
}

async fn resolve_bound_callable(
    state: &ApiState,
    workspace_id: Uuid,
    document_payload: &Value,
    block_id: &str,
    binding_alias: &str,
    schema_digest: &str,
) -> Result<RegisteredCallable, ApiError> {
    let binding =
        resolve_document_binding(document_payload, block_id, binding_alias, schema_digest)?;
    let callable = registered_callables(state, workspace_id)
        .await?
        .into_iter()
        .find(|entry| entry.interface.operation_id == binding.operation_id)
        .ok_or(ControlPlaneError::NotFound("frontstage_callable"))?;
    let catalog = to_response(callable.clone());
    if !catalog.bindable
        || catalog.schema_digest != binding.schema_digest
        || catalog.scope != binding.scope
        || catalog.risk_level != binding.risk_level
    {
        return Err(ControlPlaneError::InvalidInput("frontstage_callable_binding").into());
    }
    Ok(callable)
}

fn resolve_document_binding(
    document_payload: &Value,
    block_id: &str,
    binding_alias: &str,
    schema_digest: &str,
) -> Result<FrontstageCallableBinding, ApiError> {
    let blocks = document_payload
        .get("blocks")
        .and_then(Value::as_array)
        .ok_or(ControlPlaneError::InvalidInput(
            "frontstage_document_blocks",
        ))?;
    let block = blocks
        .iter()
        .find(|block| block.get("id").and_then(Value::as_str) == Some(block_id))
        .ok_or(ControlPlaneError::NotFound("frontstage_block"))?;
    let block: FrontstageCallableBlock = serde_json::from_value(block.clone())
        .map_err(|_| ControlPlaneError::InvalidInput("frontstage_block_interfaces"))?;
    let binding = block
        .interfaces
        .into_iter()
        .find(|binding| binding.alias == binding_alias)
        .ok_or(ControlPlaneError::NotFound("frontstage_callable_binding"))?;
    if block.id != block_id || binding.schema_digest != schema_digest {
        return Err(ControlPlaneError::InvalidInput("schema_digest").into());
    }
    Ok(binding)
}

async fn consume_write_grant(
    state: &ApiState,
    grant_token: &str,
    expected: &FrontstageCallableWriteGrant,
) -> Result<(), ApiError> {
    let lock_key = write_grant_lock_key(grant_token);
    let lock_owner = Uuid::new_v4().to_string();
    let lock = state.infrastructure.distributed_lock();
    if !lock
        .acquire(&lock_key, &lock_owner, WRITE_GRANT_LOCK_TTL)
        .await?
    {
        return Err(ControlPlaneError::Conflict("frontstage_callable_write_grant").into());
    }

    let cache = state.infrastructure.cache_store();
    let cache_key = write_grant_cache_key(grant_token);
    let result = async {
        let value = cache
            .get_json(&cache_key)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("write_grant"))?;
        let grant: FrontstageCallableWriteGrant = serde_json::from_value(value)
            .map_err(|_| ControlPlaneError::InvalidInput("write_grant"))?;
        if grant.expires_at <= OffsetDateTime::now_utc() || !grant_matches(&grant, expected) {
            return Err(ControlPlaneError::InvalidInput("write_grant").into());
        }
        cache.delete(&cache_key).await?;
        Ok(())
    }
    .await;
    let _ = lock.release(&lock_key, &lock_owner).await;
    result
}

fn grant_matches(
    grant: &FrontstageCallableWriteGrant,
    expected: &FrontstageCallableWriteGrant,
) -> bool {
    grant.actor_user_id == expected.actor_user_id
        && grant.workspace_id == expected.workspace_id
        && grant.page_id == expected.page_id
        && grant.tab_id == expected.tab_id
        && grant.block_id == expected.block_id
        && grant.binding_alias == expected.binding_alias
        && grant.run_id == expected.run_id
        && grant.draft_hash == expected.draft_hash
        && grant.operation_id == expected.operation_id
}

fn write_grant_cache_key(grant_token: &str) -> String {
    format!(
        "{WRITE_GRANT_CACHE_PREFIX}{}",
        grant_token_digest(grant_token)
    )
}

fn write_grant_lock_key(grant_token: &str) -> String {
    format!(
        "{WRITE_GRANT_LOCK_PREFIX}{}",
        grant_token_digest(grant_token)
    )
}

fn grant_token_digest(grant_token: &str) -> String {
    format!("{:x}", Sha256::digest(grant_token.as_bytes()))
}

async fn registered_callables(
    state: &ApiState,
    workspace_id: Uuid,
) -> Result<Vec<RegisteredCallable>, ApiError> {
    Ok(build_openapi_capability_catalog(state, workspace_id)
        .await?
        .into_iter()
        .filter(|entry| entry.bindable)
        .map(|entry| {
            let host_injected_parameters = match entry.source {
                OpenApiCapabilitySource::StaticApiDocs => {
                    host_injected_parameters(&entry.interface)
                }
                OpenApiCapabilitySource::RuntimeDataModelCrud => Vec::new(),
            };
            RegisteredCallable {
                interface: entry.interface,
                source: entry.source,
                bindable: entry.bindable,
                disabled_reason: entry.disabled_reason,
                host_injected_parameters,
                scope: "frontstage_page_tab",
                authorization: match entry.source {
                    OpenApiCapabilitySource::StaticApiDocs => "target_api_route_policy",
                    OpenApiCapabilitySource::RuntimeDataModelCrud => {
                        "runtime_scope_grant_and_page_tab_access"
                    }
                },
                risk_level: entry.risk_level,
            }
        })
        .collect())
}

fn to_response(mut entry: RegisteredCallable) -> FrontstageInterfaceCapabilityResponse {
    entry.interface.parameter_descriptors.retain(|parameter| {
        !entry
            .host_injected_parameters
            .contains(&parameter.name.as_str())
    });
    strip_injected_path_parameters(
        &mut entry.interface.request_schema,
        &entry.host_injected_parameters,
    );
    let digest_input = serde_json::to_vec(&serde_json::json!({
        "operation_id": entry.interface.operation_id,
        "request_schema": entry.interface.request_schema,
        "response_schema": entry.interface.response_schema,
        "request_media_type": entry.interface.request_media_type,
        "response_media_type": entry.interface.response_media_type,
    }))
    .expect("serializing OpenAPI interface schema must succeed");
    let schema_digest = format!("{:x}", Sha256::digest(digest_input));
    FrontstageInterfaceCapabilityResponse {
        interface_id: entry.interface.operation_id,
        method: entry.interface.method,
        path: entry.interface.path,
        name: entry.interface.name,
        short_description: entry.interface.description,
        parameter_schema: entry.interface.request_schema,
        result_schema: entry.interface.response_schema,
        request_media_type: entry.interface.request_media_type,
        response_media_type: entry.interface.response_media_type,
        schema_digest,
        adapter_id: entry.source.adapter_id().to_string(),
        host_injected_parameters: entry
            .host_injected_parameters
            .into_iter()
            .map(str::to_string)
            .collect(),
        scope: entry.scope.to_string(),
        risk_level: entry.risk_level.to_string(),
        authorization: entry.authorization.to_string(),
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason.map(str::to_string),
    }
}

fn host_injected_parameters(interface: &OpenApiInterfaceCatalogEntry) -> Vec<&'static str> {
    interface
        .parameter_descriptors
        .iter()
        .filter(|parameter| matches!(parameter.location, OpenApiParameterLocation::Path))
        .filter_map(|parameter| match parameter.name.as_str() {
            "workspace_id" => Some("workspace_id"),
            "page_id" => Some("page_id"),
            "tab_id" => Some("tab_id"),
            "tab_reference" => Some("tab_reference"),
            _ => None,
        })
        .collect()
}

fn injected_path_parameters(
    parameters: &[&str],
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
) -> BTreeMap<String, String> {
    parameters
        .iter()
        .filter_map(|parameter| {
            let value = match *parameter {
                "workspace_id" => workspace_id,
                "page_id" => page_id,
                "tab_id" | "tab_reference" => tab_id,
                _ => return None,
            };
            Some(((*parameter).to_string(), value.to_string()))
        })
        .collect()
}

fn strip_injected_path_parameters(schema: &mut Value, injected: &[&str]) {
    let path_empty = {
        let Some(path_schema) = schema.pointer_mut("/properties/path") else {
            return;
        };
        if let Some(properties) = path_schema
            .get_mut("properties")
            .and_then(Value::as_object_mut)
        {
            properties.retain(|name, _| !injected.contains(&name.as_str()));
        }
        if let Some(required) = path_schema
            .get_mut("required")
            .and_then(Value::as_array_mut)
        {
            required.retain(|name| name.as_str().is_none_or(|name| !injected.contains(&name)));
        }
        path_schema
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|properties| properties.is_empty())
    };
    if path_empty {
        if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
            properties.remove("path");
        }
        if let Some(required) = schema.get_mut("required").and_then(Value::as_array_mut) {
            required.retain(|name| name.as_str() != Some("path"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> FrontstageCallableWriteGrant {
        FrontstageCallableWriteGrant {
            actor_user_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
            page_id: Uuid::new_v4(),
            tab_id: Uuid::new_v4(),
            block_id: "block-1".to_string(),
            binding_alias: "savePage".to_string(),
            run_id: "run-1".to_string(),
            draft_hash: "draft-1".to_string(),
            operation_id: PAGE_TAB_SAVE_OPERATION_ID.to_string(),
            expires_at: OffsetDateTime::now_utc() + Duration::minutes(1),
        }
    }

    #[test]
    fn ac_004_write_grant_is_bound_to_the_complete_draft_call_identity() {
        let expected = grant();
        assert!(grant_matches(&expected, &expected));

        let mutations: Vec<Box<dyn Fn(&mut FrontstageCallableWriteGrant)>> = vec![
            Box::new(|value| value.actor_user_id = Uuid::new_v4()),
            Box::new(|value| value.workspace_id = Uuid::new_v4()),
            Box::new(|value| value.page_id = Uuid::new_v4()),
            Box::new(|value| value.tab_id = Uuid::new_v4()),
            Box::new(|value| value.block_id = "block-2".to_string()),
            Box::new(|value| value.binding_alias = "otherAlias".to_string()),
            Box::new(|value| value.run_id = "run-2".to_string()),
            Box::new(|value| value.draft_hash = "draft-2".to_string()),
            Box::new(|value| value.operation_id = PAGE_TAB_GET_OPERATION_ID.to_string()),
        ];
        for mutate in mutations {
            let mut replay = expected.clone();
            mutate(&mut replay);
            assert!(!grant_matches(&replay, &expected));
        }
    }

    #[test]
    fn write_grant_cache_key_does_not_expose_the_bearer_token() {
        let token = "secret-grant-token";
        let key = write_grant_cache_key(token);
        assert!(key.starts_with(WRITE_GRANT_CACHE_PREFIX));
        assert!(!key.contains(token));
        let lock_key = write_grant_lock_key(token);
        assert!(lock_key.starts_with(WRITE_GRANT_LOCK_PREFIX));
        assert!(!lock_key.contains(token));
    }

    #[test]
    fn ac_004_document_binding_is_scoped_to_one_block_alias_and_digest() {
        let document = serde_json::json!({
            "blocks": [
                {
                    "id": "block-1",
                    "interfaces": [{
                        "alias": "listRecords",
                        "operation_id": "list_records",
                        "schema_digest": "digest-1",
                        "scope": "frontstage_page_tab",
                        "risk_level": "low"
                    }]
                },
                {
                    "id": "block-2",
                    "interfaces": [{
                        "alias": "saveRecords",
                        "operation_id": "save_records",
                        "schema_digest": "digest-2",
                        "scope": "frontstage_page_tab",
                        "risk_level": "high"
                    }]
                }
            ]
        });
        let binding = resolve_document_binding(&document, "block-1", "listRecords", "digest-1")
            .expect("bound alias must resolve");
        assert_eq!(binding.operation_id, "list_records");
        assert!(resolve_document_binding(&document, "block-1", "saveRecords", "digest-2").is_err());
        assert!(
            resolve_document_binding(&document, "block-1", "listRecords", "digest-stale").is_err()
        );
        assert!(
            resolve_document_binding(&document, "missing-block", "listRecords", "digest-1")
                .is_err()
        );
    }
}
