use std::{collections::BTreeMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    frontstage::{FrontstageBlockScopeCommand, FrontstagePageService},
};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    openapi_interface::{
        get_openapi_capability, get_openapi_capability_by_route, query_openapi_capability_catalog,
        DispatchArguments, DispatchError, OpenApiCapabilityCatalogEntry,
        OpenApiCapabilityCatalogQuery, OpenApiCapabilitySource, OpenApiInterfaceCatalogEntry,
        OpenApiParameterLocation,
    },
    response::ApiSuccess,
};

const INTERFACE_CAPABILITY_PAGE_SIZE: usize = 20;

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

#[derive(Debug, Deserialize, IntoParams)]
pub struct FrontstageInterfaceCapabilityQuery {
    /// Comma-separated canonical path prefixes combined with OR semantics.
    #[param(value_type = String)]
    #[serde(default, deserialize_with = "deserialize_path_prefixes")]
    pub path_prefixes: Vec<String>,
    pub path_query: Option<String>,
    pub adapter_id: Option<String>,
    pub method: Option<String>,
    #[param(minimum = 0)]
    pub offset: Option<usize>,
    #[param(minimum = 1, maximum = 20)]
    pub limit: Option<usize>,
}

fn deserialize_path_prefixes<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    let mut prefixes = std::collections::BTreeSet::new();
    for prefix in value
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
    {
        if !prefix.starts_with('/')
            || !prefix.ends_with('/')
            || prefix.contains("..")
            || prefix.contains('?')
            || prefix.contains('#')
        {
            return Err(D::Error::custom(
                "path_prefixes must contain canonical absolute path prefixes ending in /",
            ));
        }
        prefixes.insert(prefix.to_string());
    }
    Ok(prefixes.into_iter().collect())
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageInterfaceCapabilitySummaryResponse {
    pub interface_id: String,
    pub method: String,
    pub path: String,
    pub adapter_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageInterfaceCapabilityPageResponse {
    pub items: Vec<FrontstageInterfaceCapabilitySummaryResponse>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub next_offset: Option<usize>,
    pub adapter_ids: Vec<String>,
    pub methods: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DispatchFrontstageCallableBody {
    pub block_id: String,
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub request: DispatchArguments,
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/interface-capabilities",
    params(
        FrontstageInterfaceCapabilityQuery,
        ("workspace_id" = String, Path, description = "Workspace id")
    ),
    responses(
        (status = 200, body = FrontstageInterfaceCapabilityPageResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_interface_capabilities(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<FrontstageInterfaceCapabilityQuery>,
    Path(workspace_id): Path<String>,
) -> Result<Json<ApiSuccess<FrontstageInterfaceCapabilityPageResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    if context.actor.current_workspace_id != workspace_id {
        return Err(ControlPlaneError::PermissionDenied("workspace_access_denied").into());
    }
    let actor = &context.actor;
    if !actor.has_permission("frontstage.page.design") {
        return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
    }
    let page = query_openapi_capability_catalog(
        &state,
        workspace_id,
        OpenApiCapabilityCatalogQuery {
            path_prefixes: query.path_prefixes,
            path_query: query.path_query,
            adapter_id: query.adapter_id,
            method: query.method,
            offset: query.offset.unwrap_or(0),
            limit: query
                .limit
                .unwrap_or(INTERFACE_CAPABILITY_PAGE_SIZE)
                .clamp(1, INTERFACE_CAPABILITY_PAGE_SIZE),
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(
        FrontstageInterfaceCapabilityPageResponse {
            items: page
                .items
                .into_iter()
                .map(|entry| FrontstageInterfaceCapabilitySummaryResponse {
                    interface_id: entry.interface_id,
                    method: entry.method,
                    path: entry.path,
                    adapter_id: entry.source.adapter_id().to_string(),
                })
                .collect(),
            total: page.total,
            offset: page.offset,
            limit: page.limit,
            has_more: page.has_more,
            next_offset: page.next_offset,
            adapter_ids: page.adapter_ids,
            methods: page.methods,
        },
    )))
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/interface-capabilities/{interface_id}",
    params(
        ("workspace_id" = String, Path, description = "Workspace id"),
        ("interface_id" = String, Path, description = "Interface capability id")
    ),
    responses(
        (status = 200, body = FrontstageInterfaceCapabilityResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody),
        (status = 404, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_frontstage_interface_capability(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path((workspace_id, interface_id)): Path<(String, String)>,
) -> Result<Json<ApiSuccess<FrontstageInterfaceCapabilityResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    if context.actor.current_workspace_id != workspace_id {
        return Err(ControlPlaneError::PermissionDenied("workspace_access_denied").into());
    }
    let actor = &context.actor;
    if !actor.has_permission("frontstage.page.design") {
        return Err(ControlPlaneError::PermissionDenied("frontstage.page.design").into());
    }
    let entry = get_openapi_capability(&state, workspace_id, &interface_id)
        .await?
        .ok_or(ControlPlaneError::NotFound(
            "frontstage_interface_capability",
        ))?;
    Ok(Json(ApiSuccess::new(to_response(registered_callable(
        entry,
    )))))
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

    let callable = resolve_source_callable(
        &state,
        &context,
        workspace_id,
        page_id,
        tab_id,
        &body.block_id,
        &body.method,
        &body.path,
    )
    .await?;

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

#[allow(clippy::too_many_arguments)]
async fn resolve_source_callable(
    state: &ApiState,
    context: &crate::middleware::require_session::RequestContext,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    block_id: &str,
    method: &str,
    path: &str,
) -> Result<RegisteredCallable, ApiError> {
    let node = FrontstagePageService::for_actor(state.store.clone(), context.actor.clone())
        .get_block_node(FrontstageBlockScopeCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            block_id: block_id.to_owned(),
        })
        .await?;
    if node.tab_id != tab_id {
        return Err(ControlPlaneError::NotFound("frontstage_block").into());
    }
    let route = canonical_route_key(method, path)?;
    let callable = get_openapi_capability_by_route(state, workspace_id, &route.method, &route.path)
        .await?
        .map(registered_callable)
        .ok_or(ControlPlaneError::NotFound("frontstage_callable"))?;
    Ok(callable)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalRouteKey {
    method: String,
    path: String,
}

fn canonical_route_key(method: &str, path: &str) -> Result<CanonicalRouteKey, ApiError> {
    let method = method.trim().to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        return Err(ControlPlaneError::InvalidInput("method").into());
    }
    if path.is_empty()
        || path != path.trim()
        || !path.starts_with('/')
        || path.starts_with("//")
        || path.contains('?')
        || path.contains('#')
        || path.split('/').any(|segment| matches!(segment, "." | ".."))
    {
        return Err(ControlPlaneError::InvalidInput("path").into());
    }
    Ok(CanonicalRouteKey {
        method,
        path: path.to_string(),
    })
}

fn registered_callable(entry: OpenApiCapabilityCatalogEntry) -> RegisteredCallable {
    let host_injected_parameters = match entry.source {
        OpenApiCapabilitySource::StaticApiDocs
        | OpenApiCapabilitySource::ActivatedInterfaceOperation => {
            host_injected_parameters(&entry.interface)
        }
        OpenApiCapabilitySource::BuiltinDataModelCrud
        | OpenApiCapabilitySource::WorkspaceDataModelCrud => Vec::new(),
    };
    RegisteredCallable {
        interface: entry.interface,
        source: entry.source,
        bindable: entry.bindable,
        disabled_reason: entry.disabled_reason,
        host_injected_parameters,
        scope: "frontstage_page_tab",
        authorization: match entry.source {
            OpenApiCapabilitySource::StaticApiDocs
            | OpenApiCapabilitySource::ActivatedInterfaceOperation => "target_api_route_policy",
            OpenApiCapabilitySource::BuiltinDataModelCrud
            | OpenApiCapabilitySource::WorkspaceDataModelCrud => {
                "runtime_scope_grant_and_page_tab_access"
            }
        },
        risk_level: entry.risk_level,
    }
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

    #[test]
    fn ac_020_route_key_requires_a_supported_method_and_canonical_relative_path() {
        assert_eq!(
            canonical_route_key("get", "/api/console/applications/catalog").unwrap(),
            CanonicalRouteKey {
                method: "GET".to_string(),
                path: "/api/console/applications/catalog".to_string(),
            }
        );
        for path in [
            "https://example.com/api/console/applications",
            "//example.com/api/console/applications",
            "/api/console/applications?limit=20",
            "/api/console/../private",
        ] {
            assert!(canonical_route_key("GET", path).is_err(), "{path}");
        }
        assert!(canonical_route_key("TRACE", "/api/console/applications").is_err());
    }
}
