use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    openapi_interface::{
        DispatchArguments, OpenApiCapabilityCatalogEntry, OpenApiCapabilitySource,
        OpenApiInterfaceCatalogEntry, OpenApiParameterLocation,
    },
    response::ApiSuccess,
};

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
    path = "/api/console/frontstage/interface-capabilities",
    params(
        FrontstageInterfaceCapabilityQuery,
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
) -> Result<Json<ApiSuccess<FrontstageInterfaceCapabilityPageResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let crate::routes::frontstage::callable_interface_catalog::FrontstageCallableCatalogOutput::Page(page) = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.frontstage.interface-capabilities.list.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        crate::routes::frontstage::callable_interface_catalog::FrontstageCallableCatalogInput::List(query),
    ).await? else { unreachable!() };
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
    path = "/api/console/frontstage/interface-capabilities/{interface_id}",
    params(
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
    Path(interface_id): Path<String>,
) -> Result<Json<ApiSuccess<FrontstageInterfaceCapabilityResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let crate::routes::frontstage::callable_interface_catalog::FrontstageCallableCatalogOutput::Entry(entry) = crate::routes::console_interface::invoke(
        snapshot_state,
        "http.console.frontstage.interface-capabilities.detail.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        crate::routes::frontstage::callable_interface_catalog::FrontstageCallableCatalogInput::Get { interface_id },
    ).await? else { unreachable!() };
    Ok(Json(ApiSuccess::new(to_response(registered_callable(
        entry,
    )))))
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

pub(crate) fn host_injected_parameters(
    interface: &OpenApiInterfaceCatalogEntry,
) -> Vec<&'static str> {
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
