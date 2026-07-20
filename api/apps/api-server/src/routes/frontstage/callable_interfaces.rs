use std::{collections::BTreeMap, sync::Arc};

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use control_plane::{
    errors::ControlPlaneError,
    frontstage_pages::{FrontstagePageService, GetFrontstagePageDetailCommand},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    openapi_docs::DocsCatalogOperation,
    openapi_interface::{
        catalog_entry_from_operation, DispatchArguments, DispatchError,
        OpenApiInterfaceCatalogEntry, OpenApiParameterLocation,
    },
    response::ApiSuccess,
    runtime_data_model_docs::{self, RuntimeDataModelDocsOperationKind},
};

const PAGE_TAB_GET_OPERATION_ID: &str = "get_frontstage_page_detail";
const PAGE_TAB_SAVE_OPERATION_ID: &str = "save_frontstage_tab_document";

#[derive(Clone)]
enum CallableAdapter {
    RuntimeDataModel,
    PageTabGet,
    PageTabSave,
}

struct RegisteredCallable {
    interface: OpenApiInterfaceCatalogEntry,
    adapter: CallableAdapter,
    bindable: bool,
    disabled_reason: Option<&'static str>,
    host_injected_parameters: Vec<&'static str>,
    scope: &'static str,
    authorization: &'static str,
    risk_level: &'static str,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageCallableParameterResponse {
    pub name: String,
    pub field_type: String,
    pub location: String,
    pub description: Option<String>,
    pub required: bool,
    #[schema(value_type = Object)]
    pub schema: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FrontstageCallableResponse {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub name: String,
    pub description: String,
    pub parameters: Vec<FrontstageCallableParameterResponse>,
    #[schema(value_type = Object)]
    pub request_schema: Value,
    #[schema(value_type = Object)]
    pub response_schema: Value,
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
    pub operation_id: String,
    #[serde(default)]
    pub request: DispatchArguments,
    pub run_authorization: Option<FrontstageCallableRunAuthorization>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FrontstageCallableRunAuthorization {
    pub run_id: String,
    pub operation_id: String,
    pub confirmed: bool,
}

#[utoipa::path(
    get,
    path = "/api/console/frontstage/{workspace_id}/callable-interfaces",
    params(("workspace_id" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = [FrontstageCallableResponse]),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn list_frontstage_callable_interfaces(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<ApiSuccess<Vec<FrontstageCallableResponse>>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    state
        .store
        .load_actor_context_for_workspace(context.user.id, workspace_id)
        .await?;
    let entries = registered_callables(&state, context.user.id)
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
    let workspace_id = super::parse_uuid(&workspace_id, "workspace_id")?;
    let page_id = super::parse_uuid(&page_id, "page_id")?;
    let tab_id = super::parse_uuid(&tab_id, "tab_id")?;

    // The tab is the callable scope. Checking it before adapter resolution keeps failures closed.
    FrontstagePageService::new(state.store.clone())
        .get_page_detail(GetFrontstagePageDetailCommand {
            actor_user_id: context.user.id,
            workspace_id,
            page_id,
            tab_reference: tab_id.to_string(),
        })
        .await?;

    let callable = registered_callables(&state, context.user.id)
        .await?
        .into_iter()
        .find(|entry| entry.interface.operation_id == body.operation_id)
        .ok_or(ControlPlaneError::NotFound("frontstage_callable"))?;
    if !callable.bindable {
        return Err(ControlPlaneError::InvalidInput("operation_id").into());
    }
    if callable.risk_level == "high"
        && !body
            .run_authorization
            .as_ref()
            .is_some_and(|authorization| {
                authorization.confirmed
                    && !authorization.run_id.trim().is_empty()
                    && authorization.operation_id == body.operation_id
            })
    {
        return Err(ControlPlaneError::InvalidInput("run_authorization").into());
    }

    let injected_path = match callable.adapter {
        CallableAdapter::RuntimeDataModel => BTreeMap::new(),
        CallableAdapter::PageTabGet => BTreeMap::from([
            ("workspace_id".to_string(), workspace_id.to_string()),
            ("page_id".to_string(), page_id.to_string()),
            ("tab_reference".to_string(), tab_id.to_string()),
        ]),
        CallableAdapter::PageTabSave => BTreeMap::from([
            ("workspace_id".to_string(), workspace_id.to_string()),
            ("page_id".to_string(), page_id.to_string()),
            ("tab_id".to_string(), tab_id.to_string()),
        ]),
    };

    match crate::openapi_interface::dispatch(
        state,
        &headers,
        &callable.interface,
        body.request,
        injected_path,
    )
    .await
    {
        Ok(success) => Ok(Json(ApiSuccess::new(
            success.value.get("data").cloned().unwrap_or(success.value),
        ))
        .into_response()),
        Err(DispatchError::Api(error)) => Err(error.into()),
        Err(DispatchError::Target(response)) => Ok(response),
    }
}

async fn registered_callables(
    state: &ApiState,
    actor_user_id: Uuid,
) -> Result<Vec<RegisteredCallable>, ApiError> {
    let mut entries = Vec::new();
    for operation_id in [PAGE_TAB_GET_OPERATION_ID, PAGE_TAB_SAVE_OPERATION_ID] {
        if let Some((operation, spec)) = static_operation(&state.api_docs, operation_id) {
            if let Some(interface) = catalog_entry_from_operation(operation, spec) {
                let is_get = operation_id == PAGE_TAB_GET_OPERATION_ID;
                entries.push(RegisteredCallable {
                    interface,
                    adapter: if is_get {
                        CallableAdapter::PageTabGet
                    } else {
                        CallableAdapter::PageTabSave
                    },
                    bindable: true,
                    disabled_reason: None,
                    host_injected_parameters: if is_get {
                        vec!["workspace_id", "page_id", "tab_reference"]
                    } else {
                        vec!["workspace_id", "page_id", "tab_id"]
                    },
                    scope: "frontstage_page_tab",
                    authorization: "authenticated_page_tab_access",
                    risk_level: if is_get { "low" } else { "high" },
                });
            }
        }
    }

    let models = runtime_data_model_docs::ready_models(state, actor_user_id).await?;
    let operations = runtime_data_model_docs::build_category_operations(&models);
    for operation in operations.operations {
        let Ok(Some((model_id, kind))) = runtime_data_model_docs::parse_operation_id(&operation.id)
        else {
            continue;
        };
        if !matches!(
            kind,
            RuntimeDataModelDocsOperationKind::ListRecords
                | RuntimeDataModelDocsOperationKind::GetRecord
        ) {
            continue;
        }
        let Some(model) = models.iter().find(|model| model.id == model_id) else {
            continue;
        };
        let spec = runtime_data_model_docs::build_operation_openapi(model, kind);
        if let Some(interface) = catalog_entry_from_operation(&operation, &spec) {
            entries.push(RegisteredCallable {
                interface,
                adapter: CallableAdapter::RuntimeDataModel,
                bindable: true,
                disabled_reason: None,
                host_injected_parameters: Vec::new(),
                scope: "frontstage_page_tab",
                authorization: "runtime_scope_grant_and_page_tab_access",
                risk_level: "low",
            });
        }
    }
    Ok(entries)
}

fn static_operation<'a>(
    docs: &'a crate::openapi_docs::ApiDocsRegistry,
    operation_id: &str,
) -> Option<(&'a DocsCatalogOperation, &'a Value)> {
    let operation = docs
        .catalog()
        .categories
        .iter()
        .filter_map(|category| docs.category_operations(&category.id))
        .flat_map(|category| &category.operations)
        .find(|operation| operation.id == operation_id)?;
    Some((operation, docs.operation_spec(operation_id)?))
}

fn to_response(mut entry: RegisteredCallable) -> FrontstageCallableResponse {
    entry.interface.parameter_descriptors.retain(|parameter| {
        !matches!(parameter.location, OpenApiParameterLocation::Header)
            && !entry
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
    }))
    .expect("serializing OpenAPI interface schema must succeed");
    let schema_digest = format!("{:x}", Sha256::digest(digest_input));
    FrontstageCallableResponse {
        operation_id: entry.interface.operation_id,
        method: entry.interface.method,
        path: entry.interface.path,
        name: entry.interface.name,
        description: entry.interface.description,
        parameters: entry
            .interface
            .parameter_descriptors
            .into_iter()
            .map(|parameter| FrontstageCallableParameterResponse {
                name: parameter.name,
                field_type: parameter.field_type,
                location: match parameter.location {
                    OpenApiParameterLocation::Path => "path",
                    OpenApiParameterLocation::Query => "query",
                    OpenApiParameterLocation::Header => "header",
                    OpenApiParameterLocation::JsonBody => "body",
                    OpenApiParameterLocation::FormBody => "body",
                }
                .to_string(),
                description: parameter.description,
                required: parameter.required,
                schema: parameter.schema,
            })
            .collect(),
        request_schema: entry.interface.request_schema,
        response_schema: entry.interface.response_schema,
        schema_digest,
        adapter_id: match entry.adapter {
            CallableAdapter::RuntimeDataModel => "runtime_data_model",
            CallableAdapter::PageTabGet => "frontstage_page_tab_get",
            CallableAdapter::PageTabSave => "frontstage_page_tab_save",
        }
        .to_string(),
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
