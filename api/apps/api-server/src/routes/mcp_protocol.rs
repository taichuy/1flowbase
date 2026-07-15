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

use super::mcp_management::upstream_client::{
    map_proxy_arguments, map_proxy_result, McpStreamableHttpClient,
};
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

    let service = McpManagementService::new(state.store.clone());
    let catalog = service.read_workspace_catalog(context.user.id).await?;
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
            ));
        }
        "tools/list" => json!({"tools": meta_tools()}),
        "tools/call" => {
            let name = request.params.get("name").and_then(Value::as_str).ok_or(
                control_plane::errors::ControlPlaneError::InvalidInput("name"),
            )?;
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match name {
                "mcp.list" => {
                    let path = arguments.get("path").and_then(Value::as_str);
                    let path_regex = arguments.get("path_regex").and_then(Value::as_str);
                    let keywords =
                        arguments
                            .get("keywords")
                            .and_then(Value::as_array)
                            .map(|values| {
                                values
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect::<Vec<_>>()
                            });
                    let depth = arguments
                        .get("depth")
                        .and_then(Value::as_i64)
                        .and_then(|value| i32::try_from(value).ok());
                    let limit = arguments
                        .get("limit")
                        .and_then(Value::as_u64)
                        .and_then(|value| usize::try_from(value).ok());
                    let items = service
                        .list_items(
                            context.user.id,
                            Some(&instance_id),
                            path,
                            path_regex,
                            keywords.as_deref(),
                            depth,
                            limit,
                        )
                        .await?;
                    tool_result(serde_json::to_value(items).unwrap_or_else(|_| json!([])))
                }
                "mcp.get" => {
                    let Some(tool_id) = string_argument(&arguments, "tool_id") else {
                        return Ok(jsonrpc_error(request.id, -32602, "Invalid tool_id"));
                    };
                    let Some(tool) = visible_tool(&catalog, instance.id, tool_id) else {
                        return Ok(jsonrpc_error(request.id, -32602, "Tool not visible"));
                    };
                    tool_result(json!({
                        "tool_id": tool.tool_id,
                        "name": tool.name,
                        "short_description": tool.short_description,
                        "full_description": tool.full_description,
                        "input_schema": tool.parameter_schema,
                        "result_schema": tool.result_schema,
                        "risk_level": tool.risk_level,
                        "des_id": tool.des_id,
                        "des_id_required": tool.des_id_required
                    }))
                }
                "mcp.call" => {
                    let Some(tool_id) = string_argument(&arguments, "tool_id") else {
                        return Ok(jsonrpc_error(request.id, -32602, "Invalid tool_id"));
                    };
                    let Some(tool) = visible_tool(&catalog, instance.id, tool_id) else {
                        return Ok(jsonrpc_error(request.id, -32602, "Tool not visible"));
                    };
                    let des_id = arguments.get("des_id").and_then(Value::as_str);
                    if tool.des_id_required && des_id != Some(tool.des_id.as_str()) {
                        return Ok(jsonrpc_error(request.id, -32602, "Invalid des_id"));
                    }
                    let tool_arguments = arguments
                        .get("arguments")
                        .cloned()
                        .unwrap_or_else(|| json!({}));
                    match &tool.execution_target {
                        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
                            let interface = bindable_mcp_interface(
                                state.as_ref(),
                                context.user.id,
                                interface_id,
                            )
                            .await?;
                            let body = McpDebugExecuteBody {
                                interface_id: interface_id.clone(),
                                debug_response_mode: McpDebugResponseMode::ToolResult,
                                mcp_arguments: tool_arguments,
                                input_mapping: tool.input_mapping.clone(),
                                output_mapping: tool.output_mapping.clone(),
                            };
                            match super::mcp_management::debug_execute::execute(
                                state.clone(),
                                headers,
                                interface,
                                body,
                            )
                            .await
                            {
                                Ok(value) => tool_result(value),
                                Err(_) => {
                                    return Ok(jsonrpc_error(
                                        request.id,
                                        -32603,
                                        "Tool execution failed",
                                    ));
                                }
                            }
                        }
                        domain::McpToolExecutionTarget::McpProxy {
                            upstream_connection_id,
                            remote_tool_name,
                            ..
                        } => {
                            let availability = service
                                .upstream_proxy_availability(
                                    context.user.id,
                                    *upstream_connection_id,
                                    remote_tool_name,
                                )
                                .await?;
                            if availability != domain::McpToolAvailabilityStatus::Available {
                                return Ok(jsonrpc_error_data(
                                    request.id,
                                    -32603,
                                    "Upstream tool unavailable",
                                    json!({"availability_status":availability.as_str()}),
                                ));
                            }
                            let connection = service
                                .get_upstream_connection(context.user.id, *upstream_connection_id)
                                .await?;
                            let secret = service
                                .upstream_secret_for_execution(
                                    context.user.id,
                                    *upstream_connection_id,
                                    &state.provider_secret_master_key,
                                )
                                .await?;
                            let remote_arguments =
                                match map_proxy_arguments(&tool_arguments, &tool.input_mapping) {
                                    Ok(arguments) => arguments,
                                    Err(error) => {
                                        return Ok(jsonrpc_error_data(
                                            request.id,
                                            -32602,
                                            "Invalid tool arguments",
                                            json!({"reason":error.to_string()}),
                                        ));
                                    }
                                };
                            let client = match McpStreamableHttpClient::connect(
                                &connection,
                                secret.as_ref(),
                            )
                            .await
                            {
                                Ok(client) => client,
                                Err(error) => {
                                    return Ok(jsonrpc_error_data(
                                        request.id,
                                        -32603,
                                        "Upstream MCP connection failed",
                                        json!({"reason":error.to_string()}),
                                    ));
                                }
                            };
                            let upstream =
                                match client.call_tool(remote_tool_name, remote_arguments).await {
                                    Ok(result) => result,
                                    Err(error) => {
                                        return Ok(jsonrpc_error_data(
                                            request.id,
                                            -32603,
                                            "Upstream MCP tools/call failed",
                                            json!({"reason":error.to_string()}),
                                        ));
                                    }
                                };
                            let mapped = match map_proxy_result(&upstream, &tool.output_mapping) {
                                Ok(result) => result,
                                Err(error) => {
                                    return Ok(jsonrpc_error_data(
                                        request.id,
                                        -32603,
                                        "Tool result mapping failed",
                                        json!({"reason":error.to_string()}),
                                    ));
                                }
                            };
                            match serde_json::to_value(mapped) {
                                Ok(value) => value,
                                Err(error) => {
                                    return Ok(jsonrpc_error_data(
                                        request.id,
                                        -32603,
                                        "Tool result serialization failed",
                                        json!({"reason":error.to_string()}),
                                    ));
                                }
                            }
                        }
                    }
                }
                _ => return Ok(jsonrpc_error(request.id, -32601, "Tool not found")),
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

fn meta_tools() -> [Value; 3] {
    [
        json!({
            "name": "mcp.list",
            "title": "Browse MCP directory",
            "description": "Browse the current MCP instance by path before requesting full tool details.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "keywords": {"type": "array", "items": {"type": "string"}},
                    "depth": {"type": "integer", "minimum": 0},
                    "path_regex": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "mcp.get",
            "title": "Get MCP tool details",
            "description": "Get the current description, schemas, risk information, and des_id for a visible tool before calling it.",
            "inputSchema": {
                "type": "object",
                "properties": {"tool_id": {"type": "string"}},
                "required": ["tool_id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "mcp.call",
            "title": "Call MCP tool",
            "description": "Call a visible tool after mcp.get. Supply the current des_id when the tool requires description validation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_id": {"type": "string"},
                    "des_id": {"type": "string"},
                    "arguments": {"type": "object"}
                },
                "required": ["tool_id", "arguments"],
                "additionalProperties": false
            }
        }),
    ]
}

fn string_argument<'a>(arguments: &'a Value, field: &str) -> Option<&'a str> {
    arguments.get(field).and_then(Value::as_str)
}

fn visible_tool<'a>(
    catalog: &'a domain::McpCatalogSnapshot,
    instance_record_id: uuid::Uuid,
    tool_id: &str,
) -> Option<&'a domain::McpToolRecord> {
    let binding = catalog.bindings.iter().find(|binding| {
        binding.instance_record_id == instance_record_id
            && binding.visible
            && binding.tool_id == tool_id
    })?;
    catalog
        .tools
        .iter()
        .find(|tool| tool.id == binding.tool_record_id && tool.status == McpToolStatus::Enabled)
}

fn tool_result(value: Value) -> Value {
    json!({
        "content": [{"type":"text","text":serde_json::to_string(&value).unwrap_or_default()}],
        "structuredContent": value,
        "isError": false
    })
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

fn jsonrpc_error_data(
    id: Option<Value>,
    code: i32,
    message: &'static str,
    data: Value,
) -> (StatusCode, Json<JsonRpcResponse>) {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(json!({"code":code,"message":message,"data":data})),
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
}
