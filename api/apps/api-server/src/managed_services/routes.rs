use super::{
    interface::{ManagedServiceInput, ManagedServiceOutput},
    ManagedServiceRegistration,
};
use crate::{
    app_state::ApiState, error_response::ApiError, response::ApiSuccess,
    routes::console_route_assembly::*,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use std::{collections::BTreeMap, sync::Arc};

pub(crate) fn route_assembly(
    services: &[ManagedServiceRegistration],
) -> ConsoleRouteAssembly<Arc<ApiState>> {
    let mut assembly = ConsoleRouteAssembly::new();
    for service in services {
        for operation in &service.declaration.operations {
            let operation = Arc::new(operation.clone());
            let owner = access_control::ConsoleRouteOwnership::ConsoleOperation(
                operation.interface_id.clone(),
            );
            let method = operation.method.clone();
            // Axum 0.7 uses :parameter, canonical catalogs and manifests use {parameter}.
            let path = operation
                .path
                .strip_prefix("/api/console")
                .expect("validated managed service route")
                .split('/')
                .map(|s| {
                    if s.starts_with('{') && s.ends_with('}') {
                        format!(":{}", &s[1..s.len() - 1])
                    } else {
                        s.to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            let handler = move |State(state): State<Arc<ApiState>>,
                                Path(path): Path<BTreeMap<String, String>>,
                                Query(query): Query<BTreeMap<String, String>>,
                                headers: HeaderMap,
                                body: Bytes| {
                let operation = operation.clone();
                async move { invoke(state, operation, path, query, headers, body).await }
            };
            let router = match method.as_str() {
                "GET" => console_get(handler, owner),
                "POST" => console_post(handler, owner),
                "PUT" => console_put(handler, owner),
                "PATCH" => console_patch(handler, owner),
                "DELETE" => console_delete(handler, owner),
                _ => unreachable!("validated managed service method"),
            };
            assembly = assembly.route(&path, router);
        }
    }
    assembly
}
async fn invoke(
    state: Arc<ApiState>,
    operation: Arc<plugin_framework::ManagedServiceOperation>,
    path: BTreeMap<String, String>,
    query: BTreeMap<String, String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ApiSuccess<serde_json::Value>>, ApiError> {
    use control_plane::errors::ControlPlaneError as Error;
    let body = if body.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&body).map_err(|_| Error::InvalidInput("managed_service_body"))?
    };
    let mut query_value = serde_json::Map::new();
    for (key, value) in query {
        let field_type = operation
            .input_schema
            .pointer(&format!("/properties/query/properties/{key}/type"))
            .and_then(serde_json::Value::as_str);
        let value = match field_type {
            Some("integer") => serde_json::Value::from(
                value
                    .parse::<i64>()
                    .map_err(|_| Error::InvalidInput("managed_service_query"))?,
            ),
            Some("number") => serde_json::Value::from(
                value
                    .parse::<f64>()
                    .map_err(|_| Error::InvalidInput("managed_service_query"))?,
            ),
            Some("boolean") => serde_json::Value::from(
                value
                    .parse::<bool>()
                    .map_err(|_| Error::InvalidInput("managed_service_query"))?,
            ),
            _ => serde_json::Value::String(value),
        };
        query_value.insert(key, value);
    }
    let credential = if operation.method == "GET" {
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol {
            state: state.clone(),
            headers,
        }
    } else {
        crate::extension_bus::ConsoleAuthenticationCredential::ProtocolWithCsrf {
            state: state.clone(),
            headers,
        }
    };
    let ManagedServiceOutput(output) = crate::routes::console_interface::invoke(
        state,
        &format!("http.{}.v1", operation.interface_id),
        credential,
        ManagedServiceInput {
            interface_id: operation.interface_id.clone(),
            payload: serde_json::json!({"path":path,"query":query_value,"body":body}),
        },
    )
    .await?;
    Ok(Json(ApiSuccess::new(output)))
}
