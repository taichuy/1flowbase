use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use control_plane::mcp_management::McpManagementService;
use domain::mcp_management::{McpInstanceStatus, McpToolStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::mcp_management::{bindable_mcp_interface, McpDebugExecuteBody, McpDebugResponseMode};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::{require_session, RequestCredential},
};

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
        .read_workspace_catalog(context.user.id)
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
    let result = match request.method.as_str() {
        "initialize" => {
            json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":instance.name,"version":env!("CARGO_PKG_VERSION")}})
        }
        "notifications/initialized" => {
            return Ok((
                StatusCode::ACCEPTED,
                Json(JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: request.id,
                    result: None,
                    error: None,
                }),
            ))
        }
        "tools/list" => {
            let tools = catalog
                .bindings
                .iter()
                .filter(|binding| binding.instance_record_id == instance.id && binding.visible)
                .filter_map(|binding| {
                    catalog
                        .tools
                        .iter()
                        .find(|tool| {
                            tool.id == binding.tool_record_id
                                && tool.status == McpToolStatus::Enabled
                        })
                        .map(|tool| {
                            json!({
                                "name": tool.tool_id,
                                "title": binding.display_alias.as_ref().unwrap_or(&tool.name),
                                "description": tool.full_description,
                                "inputSchema": tool.parameter_schema
                            })
                        })
                })
                .collect::<Vec<_>>();
            json!({"tools":tools})
        }
        "tools/call" => {
            let name = request.params.get("name").and_then(Value::as_str).ok_or(
                control_plane::errors::ControlPlaneError::InvalidInput("name"),
            )?;
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let binding = catalog
                .bindings
                .iter()
                .find(|binding| {
                    binding.instance_record_id == instance.id
                        && binding.visible
                        && binding.tool_id == name
                })
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "mcp_tool",
                ))?;
            let tool = catalog
                .tools
                .iter()
                .find(|tool| {
                    tool.id == binding.tool_record_id && tool.status == McpToolStatus::Enabled
                })
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "mcp_tool",
                ))?;
            let interface =
                bindable_mcp_interface(state.as_ref(), context.user.id, &tool.interface_id).await?;
            let body = McpDebugExecuteBody {
                interface_id: tool.interface_id.clone(),
                debug_response_mode: McpDebugResponseMode::ToolResult,
                mcp_arguments: arguments,
                input_mapping: tool.input_mapping.clone(),
                output_mapping: tool.output_mapping.clone(),
            };
            match super::mcp_management::debug_execute::execute(state, headers, interface, body)
                .await
            {
                Ok(value) => {
                    json!({"content":[{"type":"text","text":serde_json::to_string(&value).unwrap_or_default()}],"structuredContent":value,"isError":false})
                }
                Err(_) => return Ok(jsonrpc_error(request.id, -32603, "Tool execution failed")),
            }
        }
        _ => return Ok(jsonrpc_error(request.id, -32601, "Method not found")),
    };
    Ok((
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id: request.id,
            result: Some(result),
            error: None,
        }),
    ))
}

fn jsonrpc_error(
    id: Option<Value>,
    code: i32,
    message: &'static str,
) -> (StatusCode, Json<JsonRpcResponse>) {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(json!({"code":code,"message":message})),
        }),
    )
}
