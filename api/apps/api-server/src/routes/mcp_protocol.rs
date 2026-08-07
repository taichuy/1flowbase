use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use control_plane::mcp_management::McpManagementService;
use domain::mcp_management::McpInstanceStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::{require_session, RequestCredential},
};

mod input_schema;
pub(crate) mod result_delivery;
pub(crate) mod virtual_ui;

pub(crate) const JSON_RPC_RESPONSE_MAX_BYTES: usize = 256 * 1024;

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

pub fn router() -> Router<Arc<ApiState>> {
    Router::new().route("/mcp/:instance_id", post(handle_mcp_request))
}

async fn handle_mcp_request(
    State(state): State<Arc<ApiState>>,
    Path(instance_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> Result<(StatusCode, Json<JsonRpcResponse>), ApiError> {
    let context = require_session(&state, &headers).await?;
    if !matches!(context.credential, RequestCredential::UserApiKey { .. }) {
        return Err(control_plane::errors::ControlPlaneError::NotAuthenticated.into());
    }
    if request.jsonrpc != "2.0" {
        return Ok(jsonrpc_error(request.id, -32600, "Invalid Request"));
    }

    let catalog = McpManagementService::new(state.store.clone())
        .read_catalog_for_actor(&context.actor)
        .await?;
    let instance = catalog
        .instances
        .iter()
        .find(|candidate| {
            candidate.instance_id == instance_id && candidate.status == McpInstanceStatus::Enabled
        })
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "mcp_instance",
        ))?;
    let scope = virtual_ui::VirtualMcpScope::single(instance_id);

    let result = match request.method.as_str() {
        "initialize" => {
            json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":instance.name,"version":env!("CARGO_PKG_VERSION")}})
        }
        "notifications/initialized" => {
            return Ok(bounded_jsonrpc_response(
                StatusCode::ACCEPTED,
                JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: request.id,
                    result: None,
                    error: None,
                },
            ));
        }
        "tools/list" => json!({"tools": virtual_ui::meta_tools(
            scope.path_regex_enabled(&catalog)
        )}),
        "tools/call" => {
            let Some(name) = request.params.get("name").and_then(Value::as_str) else {
                return Ok(jsonrpc_error(request.id, -32602, "Invalid name"));
            };
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match virtual_ui::dispatch(
                &state,
                &headers,
                &context.actor,
                &catalog,
                &scope,
                name,
                arguments,
            )
            .await?
            {
                virtual_ui::VirtualToolOutcome::Success(result) => result,
                virtual_ui::VirtualToolOutcome::Error {
                    code,
                    message,
                    data,
                } => {
                    return Ok(match data {
                        Some(data) => jsonrpc_error_data(request.id, code, message, data),
                        None => jsonrpc_error(request.id, code, message),
                    });
                }
            }
        }
        _ => return Ok(jsonrpc_error(request.id, -32601, "Method not found")),
    };

    Ok(bounded_jsonrpc_response(
        StatusCode::OK,
        JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: Some(result),
            error: None,
        },
    ))
}

fn jsonrpc_error(
    id: Option<Value>,
    code: i32,
    message: &'static str,
) -> (StatusCode, Json<JsonRpcResponse>) {
    bounded_jsonrpc_response(
        StatusCode::OK,
        JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(json!({"code":code,"message":message})),
        },
    )
}

fn jsonrpc_error_data(
    id: Option<Value>,
    code: i32,
    message: &'static str,
    data: Value,
) -> (StatusCode, Json<JsonRpcResponse>) {
    bounded_jsonrpc_response(
        StatusCode::OK,
        JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(json!({"code":code,"message":message,"data":data})),
        },
    )
}

fn bounded_jsonrpc_response(
    status: StatusCode,
    response: JsonRpcResponse,
) -> (StatusCode, Json<JsonRpcResponse>) {
    if serde_json::to_vec(&response).is_ok_and(|bytes| bytes.len() <= JSON_RPC_RESPONSE_MAX_BYTES) {
        return (status, Json(response));
    }
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id: response.id,
            result: None,
            error: Some(json!({
                "code": -32603,
                "message": "Response exceeds server limit",
                "data": {
                    "category": "response_size_limit",
                    "max_bytes": JSON_RPC_RESPONSE_MAX_BYTES
                }
            })),
        }),
    )
}

#[cfg(test)]
mod issue_1246_tests {
    use super::*;

    #[test]
    fn issue_1246_ac_012_runtime_error_preserves_safe_diagnostic_data() {
        let (_, response) = jsonrpc_error_data(
            Some(json!("call-1")),
            -32603,
            "Upstream MCP tools/call failed",
            json!({"reason":"upstream protocol error: tool unavailable"}),
        );
        assert_eq!(
            response.0.error.unwrap()["data"]["reason"],
            json!("upstream protocol error: tool unavailable")
        );
    }

    #[test]
    fn interface_wrapper_errors_expose_only_stable_safe_classification() {
        for (field, code, category) in [
            ("mcp_arguments", -32602, "invalid_tool_arguments"),
            ("request_schema", -32602, "invalid_tool_arguments"),
            ("input_mapping", -32603, "invalid_tool_configuration"),
            ("response_schema", -32603, "invalid_tool_configuration"),
        ] {
            let error: anyhow::Error =
                control_plane::errors::ControlPlaneError::InvalidInput(field).into();
            let virtual_ui::VirtualToolOutcome::Error {
                code: actual, data, ..
            } = virtual_ui::interface_error(&error)
            else {
                panic!("interface failures must remain protocol errors");
            };
            let data = data.unwrap();
            assert_eq!(actual, code);
            assert_eq!(data["category"], json!(category));
            assert_eq!(data["field"], json!(field));
            assert!(!data.to_string().contains("secret-marker"));
        }
    }

    #[test]
    fn root_1569_ac_006_final_jsonrpc_response_has_a_hard_byte_limit() {
        let (_, response) = bounded_jsonrpc_response(
            StatusCode::OK,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: Some(json!("oversized")),
                result: Some(json!({
                    "content": "界".repeat(JSON_RPC_RESPONSE_MAX_BYTES)
                })),
                error: None,
            },
        );
        let bytes = serde_json::to_vec(&response.0).unwrap();
        assert!(bytes.len() <= JSON_RPC_RESPONSE_MAX_BYTES);
        assert_eq!(
            response.0.error.unwrap()["data"]["category"],
            json!("response_size_limit")
        );
    }

    #[test]
    fn root_1569_ac_007_dispatch_unknown_is_distinct_from_target_failure() {
        let virtual_ui::VirtualToolOutcome::Error { data, .. } =
            virtual_ui::interface_error(&anyhow::anyhow!("dispatch connection closed"))
        else {
            panic!("interface failures must remain protocol errors");
        };
        let data = data.unwrap();
        assert_eq!(data["category"], json!("interface_dispatch"));
        assert_eq!(data["outcome"], json!("unknown"));
        assert_eq!(data["retry_original"], json!(false));
    }
}
