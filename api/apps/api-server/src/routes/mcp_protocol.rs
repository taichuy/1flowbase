use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{
        header::{AUTHORIZATION, COOKIE},
        HeaderMap, StatusCode,
    },
    routing::post,
    Json, Router,
};
use control_plane::{mcp_management::McpManagementService, ports::AuthRepository};
use domain::mcp_management::McpInstanceStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, InvocationEnvelope,
    InvocationId, InvocationLineage,
};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::{with_server_delegated_request_context, RequestContext},
};
use interface_operation::{
    McpForwardHeader, McpInvocationInput, McpInvocationOutput, McpInvocationTargetError,
    McpToolCallPort, McpToolInvocationContext,
};

mod input_schema;
mod interface_operation;
pub(crate) mod result_delivery;
pub(crate) mod virtual_ui;

pub(crate) const JSON_RPC_RESPONSE_MAX_BYTES: usize = 256 * 1024;

pub(crate) struct McpToolArguments(Value);

impl McpToolArguments {
    fn from_protocol(value: Value) -> Result<Self, ()> {
        if serde_json::to_vec(&value).map_or(true, |bytes| bytes.len() > 2 * 1024 * 1024) {
            return Err(());
        }
        Ok(Self(value))
    }

    fn into_value(self) -> Value {
        self.0
    }
}

pub(crate) enum McpCallOutcome {
    Success(Value),
    Error {
        code: i32,
        message: &'static str,
        data: Option<Value>,
    },
}

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
    let boot_snapshot = state
        .extension_boot_snapshot
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("extension boot snapshot is unavailable"))?;
    let snapshot = boot_snapshot
        .interface_registry()
        .ok_or_else(|| anyhow::anyhow!("interface registry is unavailable"))?
        .snapshot();
    let binding_id = interface_runtime::BindingId::new("mcp.user-api-key.invoke.v1")
        .expect("static binding id is valid");
    let activated_authentication = snapshot
        .authentication(&binding_id)
        .ok_or_else(|| anyhow::anyhow!("MCP authentication activation is unavailable"))?;
    let principal: interface_runtime::UserPrincipal = boot_snapshot
        .authenticate(
            activated_authentication,
            crate::extension_bus::McpUserApiKeyAuthenticationCredential {
                state: Arc::clone(&state),
                headers: headers.clone(),
            },
        )
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    let actor = principal.actor().clone();
    let user = state
        .store
        .find_user_by_id(actor.user_id)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?;
    if request.jsonrpc != "2.0" {
        return Ok(jsonrpc_error(request.id, -32600, "Invalid Request"));
    }

    let catalog = McpManagementService::new(state.store.clone())
        .read_catalog_for_actor(&actor)
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
    let mut input = match request.method.as_str() {
        "initialize" => McpInvocationInput::Initialize {
            instance_name: instance.name.clone(),
        },
        "notifications/initialized" => McpInvocationInput::InitializedNotification,
        "tools/list" => McpInvocationInput::ToolsList {
            path_regex_enabled: scope.path_regex_enabled(&catalog),
        },
        "tools/call" => {
            let Some(name) = request.params.get("name").and_then(Value::as_str) else {
                return Ok(jsonrpc_error(request.id, -32602, "Invalid name"));
            };
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let Ok(arguments) = McpToolArguments::from_protocol(arguments) else {
                return Ok(jsonrpc_error(request.id, -32602, "Invalid arguments"));
            };
            McpInvocationInput::ToolCall {
                name: name.to_string(),
                arguments,
                context: McpToolInvocationContext {
                    headers: Vec::new(),
                    user: user.clone(),
                    actor: actor.clone(),
                    catalog: catalog.clone(),
                    scope: scope.clone(),
                },
            }
        }
        _ => return Ok(jsonrpc_error(request.id, -32601, "Method not found")),
    };
    let mut sanitized_headers = headers;
    sanitized_headers.remove(AUTHORIZATION);
    sanitized_headers.remove(COOKIE);
    sanitized_headers.remove("x-csrf-token");
    if let McpInvocationInput::ToolCall { context, .. } = &mut input {
        context.headers = sanitized_headers
            .iter()
            .map(|(name, value)| McpForwardHeader {
                name: name.as_str().to_string(),
                value: value.as_bytes().to_vec(),
            })
            .collect();
    }
    let authentication_activation = activated_authentication.activation().clone();
    let outcome =
        InterfaceInvocationKernel::new(Arc::new(interface_operation::McpInvocationAuthorization))
            .invoke::<McpInvocationInput, McpInvocationOutput, McpInvocationTargetError>(
                snapshot,
                InvocationEnvelope::with_principal(
                    InvocationLineage::root(InvocationId::now_v7()),
                    binding_id,
                    InterfaceProtocol::Mcp,
                    interface_runtime::AuthenticationAdapterReference::new(
                        "api-server.user-api-key",
                    )
                    .expect("static adapter is valid"),
                    authentication_activation,
                    principal,
                    None,
                    input,
                ),
            )
            .await
            .map_err(|failure| mcp_interface_error(failure.into_error()))?;
    let _receipt = outcome.receipt().clone().projected();
    match outcome.into_value() {
        McpInvocationOutput::Initialized { instance_name } => Ok(bounded_jsonrpc_response(
            StatusCode::OK,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: Some(
                    json!({"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":instance_name,"version":env!("CARGO_PKG_VERSION")}}),
                ),
                error: None,
            },
        )),
        McpInvocationOutput::NotificationAccepted => Ok(bounded_jsonrpc_response(
            StatusCode::ACCEPTED,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: None,
                error: None,
            },
        )),
        McpInvocationOutput::ToolsListed { path_regex_enabled } => Ok(bounded_jsonrpc_response(
            StatusCode::OK,
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: Some(json!({"tools": virtual_ui::meta_tools(path_regex_enabled)})),
                error: None,
            },
        )),
        McpInvocationOutput::ToolCalled(McpCallOutcome::Success(result)) => {
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
        McpInvocationOutput::ToolCalled(McpCallOutcome::Error {
            code,
            message,
            data,
        }) => Ok(match data {
            Some(data) => jsonrpc_error_data(request.id, code, message, data),
            None => jsonrpc_error(request.id, code, message),
        }),
    }
}

struct McpToolCallAdapter {
    state: std::sync::Weak<ApiState>,
}

impl McpToolCallPort for McpToolCallAdapter {
    fn call(
        &self,
        name: String,
        arguments: McpToolArguments,
        context: McpToolInvocationContext,
    ) -> interface_operation::McpCallFuture<'_> {
        let state = self.state.clone();
        Box::pin(async move {
            let state = state
                .upgrade()
                .ok_or_else(|| anyhow::anyhow!("API state is unavailable"))?;
            let mut headers = HeaderMap::new();
            for header in context.headers {
                let name = axum::http::HeaderName::from_bytes(header.name.as_bytes())?;
                let value = axum::http::HeaderValue::from_bytes(&header.value)?;
                headers.append(name, value);
            }
            let request_context =
                RequestContext::server_delegation(context.user, context.actor.clone());
            match with_server_delegated_request_context(
                request_context,
                virtual_ui::dispatch(
                    &state,
                    &headers,
                    &context.actor,
                    &context.catalog,
                    &context.scope,
                    &name,
                    arguments.into_value(),
                    None,
                ),
            )
            .await?
            {
                virtual_ui::VirtualToolOutcome::Success(value) => {
                    Ok(McpCallOutcome::Success(value))
                }
                virtual_ui::VirtualToolOutcome::Error {
                    code,
                    message,
                    data,
                } => Ok(McpCallOutcome::Error {
                    code,
                    message,
                    data,
                }),
            }
        })
    }
}

pub(crate) fn compile_mcp_interface_registry(
    state: std::sync::Weak<ApiState>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    interface_operation::compile_registry(Arc::new(McpToolCallAdapter { state }))
}

fn mcp_interface_error(error: InterfaceInvocationError) -> ApiError {
    match error {
        InterfaceInvocationError::TargetFailed(error) => error
            .into_source::<McpInvocationTargetError>()
            .map(|error| error.0)
            .unwrap_or_else(|| anyhow::anyhow!("MCP interface target failed").into()),
        InterfaceInvocationError::AuthorizationRejected(_) => {
            control_plane::errors::ControlPlaneError::NotAuthenticated.into()
        }
        error => anyhow::anyhow!(error.to_string()).into(),
    }
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
