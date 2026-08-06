use std::{collections::HashSet, sync::Arc};

use axum::http::HeaderMap;
use control_plane::mcp_management::McpManagementService;
use domain::mcp_management::{McpInstanceStatus, McpToolStatus};
use serde_json::{json, Value};

use super::{input_schema, result_delivery};
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    routes::mcp_management::{
        bindable_mcp_interface,
        debug_execute::{self, McpServerBoundInputs},
        upstream_client::{map_proxy_arguments, map_proxy_result, McpStreamableHttpClient},
        McpDebugExecuteBody, McpDebugResponseMode,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct VirtualMcpScope {
    instance_ids: Vec<String>,
}

impl VirtualMcpScope {
    pub(crate) fn selected(
        catalog: &domain::McpCatalogSnapshot,
        selected_instance_ids: &[String],
    ) -> Self {
        let selected = selected_instance_ids.iter().collect::<HashSet<_>>();
        Self {
            instance_ids: catalog
                .instances
                .iter()
                .filter(|instance| {
                    instance.status == McpInstanceStatus::Enabled
                        && selected.contains(&instance.instance_id)
                })
                .map(|instance| instance.instance_id.clone())
                .collect(),
        }
    }

    pub(crate) fn single(instance_id: String) -> Self {
        Self {
            instance_ids: vec![instance_id],
        }
    }

    fn contains(&self, instance: &domain::McpInstanceRecord) -> bool {
        self.instance_ids.contains(&instance.instance_id)
    }
}

pub(crate) enum VirtualToolOutcome {
    Success(Value),
    Error {
        code: i32,
        message: &'static str,
        data: Option<Value>,
    },
}

impl VirtualToolOutcome {
    fn invalid(message: &'static str) -> Self {
        Self::Error {
            code: -32602,
            message,
            data: None,
        }
    }

    fn failed(message: &'static str, data: Value) -> Self {
        Self::Error {
            code: -32603,
            message,
            data: Some(data),
        }
    }
}

pub(crate) fn meta_tools() -> [Value; 4] {
    [
        json!({
            "name": "mcp.list",
            "title": "Browse MCP directory",
            "description": "Browse the selected MCP instances by path before requesting full tool details.",
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
            "name": "mcp.result",
            "title": "Continue MCP result detail",
            "description": "Read a cached page of result detail. Missing detail never authorizes retrying the original operation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "result_ref": {"type": "string", "format": "uuid"},
                    "cursor": {"type": "string"},
                    "max_inline_chars": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": result_delivery::MAX_INLINE_CHARS
                    }
                },
                "required": ["result_ref"],
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
                    "arguments": {"type": "object"},
                    "max_inline_chars": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": result_delivery::MAX_INLINE_CHARS,
                        "default": result_delivery::DEFAULT_INLINE_CHARS
                    }
                },
                "required": ["tool_id", "arguments"],
                "additionalProperties": false
            }
        }),
    ]
}

pub(crate) fn provider_tools() -> Vec<Value> {
    meta_tools()
        .into_iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool["name"],
                    "description": tool["description"],
                    "parameters": tool["inputSchema"],
                }
            })
        })
        .collect()
}

pub(crate) async fn dispatch(
    state: &Arc<ApiState>,
    headers: &HeaderMap,
    actor: &domain::ActorContext,
    catalog: &domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    name: &str,
    arguments: Value,
) -> Result<VirtualToolOutcome, ApiError> {
    match name {
        "mcp.list" => list(state, actor, scope, &arguments).await,
        "mcp.get" => Ok(get(catalog, scope, &arguments)),
        "mcp.result" => result(state, actor, &arguments).await,
        "mcp.call" => call(state, headers, actor, catalog, scope, &arguments).await,
        _ => Ok(VirtualToolOutcome::Error {
            code: -32601,
            message: "Tool not found",
            data: None,
        }),
    }
}

async fn list(
    state: &Arc<ApiState>,
    actor: &domain::ActorContext,
    scope: &VirtualMcpScope,
    arguments: &Value,
) -> Result<VirtualToolOutcome, ApiError> {
    let path = arguments.get("path").and_then(Value::as_str);
    let path_regex = arguments.get("path_regex").and_then(Value::as_str);
    let keywords = arguments
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
    let service = McpManagementService::new(state.store.clone());
    let mut items = Vec::new();
    for instance_id in &scope.instance_ids {
        items.extend(
            service
                .list_items_for_actor(
                    actor,
                    Some(instance_id),
                    path,
                    path_regex,
                    keywords.as_deref(),
                    depth,
                    limit,
                )
                .await?,
        );
    }
    if let Some(limit) = limit {
        items.truncate(limit);
    }
    Ok(VirtualToolOutcome::Success(result_delivery::tool_result(
        serde_json::to_value(items).unwrap_or_else(|_| json!([])),
    )))
}

fn get(
    catalog: &domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    arguments: &Value,
) -> VirtualToolOutcome {
    let Some(tool_id) = string_argument(arguments, "tool_id") else {
        return VirtualToolOutcome::invalid("Invalid tool_id");
    };
    let Some((_, tool)) = visible_tool(catalog, scope, tool_id) else {
        return VirtualToolOutcome::invalid("Tool not visible");
    };
    VirtualToolOutcome::Success(result_delivery::tool_result(json!({
        "tool_id": tool.tool_id,
        "name": tool.name,
        "short_description": tool.short_description,
        "full_description": tool.full_description,
        "input_schema": input_schema::mapped_schema(&tool.parameter_schema, &tool.input_mapping),
        "result_schema": tool.result_schema,
        "risk_level": tool.risk_level,
        "des_id": tool.des_id,
        "des_id_required": tool.des_id_required
    })))
}

async fn result(
    state: &Arc<ApiState>,
    actor: &domain::ActorContext,
    arguments: &Value,
) -> Result<VirtualToolOutcome, ApiError> {
    let Some(result_ref) = string_argument(arguments, "result_ref")
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    else {
        return Ok(VirtualToolOutcome::invalid("Invalid result_ref"));
    };
    let cursor = match arguments.get("cursor") {
        Some(Value::String(value)) => match value.parse::<usize>() {
            Ok(cursor) => cursor,
            Err(_) => return Ok(VirtualToolOutcome::invalid("Invalid cursor")),
        },
        None => 0,
        Some(_) => return Ok(VirtualToolOutcome::invalid("Invalid cursor")),
    };
    let inline_chars = match result_delivery::inline_limit(arguments) {
        Ok(limit) => limit,
        Err(message) => return Ok(VirtualToolOutcome::invalid(message)),
    };
    Ok(VirtualToolOutcome::Success(
        result_delivery::read_continuation(state.as_ref(), actor, result_ref, cursor, inline_chars)
            .await,
    ))
}

async fn call(
    state: &Arc<ApiState>,
    headers: &HeaderMap,
    actor: &domain::ActorContext,
    catalog: &domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    arguments: &Value,
) -> Result<VirtualToolOutcome, ApiError> {
    let Some(tool_id) = string_argument(arguments, "tool_id") else {
        return Ok(VirtualToolOutcome::invalid("Invalid tool_id"));
    };
    let Some((instance, tool)) = visible_tool(catalog, scope, tool_id) else {
        return Ok(VirtualToolOutcome::invalid("Tool not visible"));
    };
    let des_id = arguments.get("des_id").and_then(Value::as_str);
    if tool.des_id_required && des_id != Some(tool.des_id.as_str()) {
        return Ok(VirtualToolOutcome::invalid("Invalid des_id"));
    }
    let inline_chars = match result_delivery::inline_limit(arguments) {
        Ok(limit) => limit,
        Err(message) => return Ok(VirtualToolOutcome::invalid(message)),
    };
    let tool_arguments = arguments
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match &tool.execution_target {
        domain::McpToolExecutionTarget::InterfaceWrapper { interface_id } => {
            let interface = match bindable_mcp_interface(state.as_ref(), actor, interface_id).await
            {
                Ok(interface) => interface,
                Err(error) => return Ok(interface_error(&error.0)),
            };
            let operation = if matches!(interface.method.as_str(), "GET" | "HEAD" | "OPTIONS") {
                result_delivery::CompletedOperation::Read {
                    operation_id: interface_id,
                }
            } else {
                result_delivery::CompletedOperation::Write {
                    operation_id: interface_id,
                }
            };
            let body = McpDebugExecuteBody {
                interface_id: interface_id.clone(),
                debug_response_mode: McpDebugResponseMode::ToolResult,
                mcp_arguments: tool_arguments,
                input_mapping: tool.input_mapping.clone(),
                output_mapping: tool.output_mapping.clone(),
            };
            match debug_execute::execute_with_server_bindings(
                state.clone(),
                headers.clone(),
                interface,
                body,
                McpServerBoundInputs {
                    workspace_id: instance.workspace_id,
                },
            )
            .await
            {
                Ok(value) if result_delivery::exceeds_inline_limit(&value, inline_chars) => {
                    Ok(VirtualToolOutcome::Success(
                        result_delivery::deliver_oversized_result(
                            state.as_ref(),
                            actor,
                            operation,
                            value,
                        )
                        .await,
                    ))
                }
                Ok(value) => Ok(VirtualToolOutcome::Success(result_delivery::tool_result(
                    value,
                ))),
                Err(debug_execute::McpDebugExecuteError::Api(error)) => Ok(interface_error(&error)),
                Err(debug_execute::McpDebugExecuteError::TargetResponse(response)) => {
                    Ok(VirtualToolOutcome::failed(
                        "Tool execution failed",
                        json!({
                            "category": "target_interface",
                            "http_status": response.status().as_u16(),
                            "outcome": "failed",
                            "retry_original": false
                        }),
                    ))
                }
            }
        }
        domain::McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name,
            ..
        } => {
            let service = McpManagementService::new(state.store.clone());
            let availability = service
                .upstream_proxy_availability_for_actor(
                    actor,
                    *upstream_connection_id,
                    remote_tool_name,
                )
                .await?;
            if availability != domain::McpToolAvailabilityStatus::Available {
                return Ok(VirtualToolOutcome::failed(
                    "Upstream tool unavailable",
                    json!({"availability_status": availability.as_str()}),
                ));
            }
            let connection = service
                .get_upstream_connection_for_actor(actor, *upstream_connection_id)
                .await?;
            let secret = service
                .upstream_secret_for_actor(
                    actor,
                    *upstream_connection_id,
                    &state.provider_secret_master_key,
                )
                .await?;
            let remote_arguments = match map_proxy_arguments(&tool_arguments, &tool.input_mapping) {
                Ok(arguments) => arguments,
                Err(error) => {
                    return Ok(VirtualToolOutcome::Error {
                        code: -32602,
                        message: "Invalid tool arguments",
                        data: Some(json!({"reason": error.to_string()})),
                    })
                }
            };
            let client = match McpStreamableHttpClient::connect(&connection, secret.as_ref()).await
            {
                Ok(client) => client,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Upstream MCP connection failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let upstream = match client.call_tool(remote_tool_name, remote_arguments).await {
                Ok(result) => result,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Upstream MCP tools/call failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let mapped = match map_proxy_result(&upstream, &tool.output_mapping) {
                Ok(result) => result,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Tool result mapping failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let value = match serde_json::to_value(mapped) {
                Ok(value) => value,
                Err(error) => {
                    return Ok(VirtualToolOutcome::failed(
                        "Tool result serialization failed",
                        json!({"reason": error.to_string()}),
                    ))
                }
            };
            let detail = value
                .get("structuredContent")
                .cloned()
                .unwrap_or_else(|| value.clone());
            if result_delivery::exceeds_inline_limit(&detail, inline_chars) {
                Ok(VirtualToolOutcome::Success(
                    result_delivery::deliver_oversized_result(
                        state.as_ref(),
                        actor,
                        result_delivery::CompletedOperation::Write {
                            operation_id: &tool.tool_id,
                        },
                        detail,
                    )
                    .await,
                ))
            } else {
                Ok(VirtualToolOutcome::Success(value))
            }
        }
    }
}

fn string_argument<'a>(arguments: &'a Value, field: &str) -> Option<&'a str> {
    arguments.get(field).and_then(Value::as_str)
}

fn visible_tool<'a>(
    catalog: &'a domain::McpCatalogSnapshot,
    scope: &VirtualMcpScope,
    tool_id: &str,
) -> Option<(&'a domain::McpInstanceRecord, &'a domain::McpToolRecord)> {
    let mut matches = catalog.bindings.iter().filter_map(|binding| {
        if !binding.visible || binding.tool_id != tool_id {
            return None;
        }
        let instance = catalog.instances.iter().find(|instance| {
            instance.id == binding.instance_record_id
                && instance.status == McpInstanceStatus::Enabled
                && scope.contains(instance)
        })?;
        catalog.groups.iter().find(|group| {
            group.instance_record_id == binding.instance_record_id
                && group.path == binding.group_path
                && group.enabled
        })?;
        let tool = catalog.tools.iter().find(|tool| {
            tool.id == binding.tool_record_id && tool.status == McpToolStatus::Enabled
        })?;
        Some((instance, tool))
    });
    let first = matches.next()?;
    if matches.any(|(instance, tool)| instance.id != first.0.id || tool.id != first.1.id) {
        return None;
    }
    Some(first)
}

pub(crate) fn interface_error(error: &anyhow::Error) -> VirtualToolOutcome {
    let (code, category, field) =
        match error.downcast_ref::<control_plane::errors::ControlPlaneError>() {
            Some(control_plane::errors::ControlPlaneError::InvalidInput("mcp_arguments")) => {
                (-32602, "invalid_tool_arguments", Some("mcp_arguments"))
            }
            Some(control_plane::errors::ControlPlaneError::InvalidInput("request_schema")) => {
                (-32602, "invalid_tool_arguments", Some("request_schema"))
            }
            Some(control_plane::errors::ControlPlaneError::InvalidInput("input_mapping")) => {
                (-32603, "invalid_tool_configuration", Some("input_mapping"))
            }
            Some(control_plane::errors::ControlPlaneError::InvalidInput("response_schema")) => (
                -32603,
                "invalid_tool_configuration",
                Some("response_schema"),
            ),
            Some(control_plane::errors::ControlPlaneError::InvalidInput("interface_id"))
            | Some(control_plane::errors::ControlPlaneError::NotFound("mcp_interface")) => {
                (-32603, "interface_catalog", Some("interface_id"))
            }
            _ => (-32603, "interface_dispatch", None),
        };
    let mut data = serde_json::Map::from_iter([
        ("category".to_string(), Value::String(category.to_string())),
        (
            "outcome".to_string(),
            Value::String(
                if category == "interface_dispatch" {
                    "unknown"
                } else {
                    "not_started"
                }
                .to_string(),
            ),
        ),
        ("retry_original".to_string(), Value::Bool(false)),
    ]);
    if let Some(field) = field {
        data.insert("field".to_string(), Value::String(field.to_string()));
    }
    VirtualToolOutcome::Error {
        code,
        message: "Tool execution failed",
        data: Some(Value::Object(data)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog_with_server_bound_workspace() -> domain::McpCatalogSnapshot {
        let now = time::OffsetDateTime::UNIX_EPOCH;
        let workspace_id = uuid::Uuid::from_u128(10);
        let instance_id = uuid::Uuid::from_u128(11);
        let tool_id = uuid::Uuid::from_u128(12);
        domain::McpCatalogSnapshot {
            instances: vec![domain::McpInstanceRecord {
                id: instance_id,
                workspace_id,
                instance_id: "selected".to_string(),
                name: "Selected".to_string(),
                description_short: None,
                status: McpInstanceStatus::Enabled,
                default_entry_path: "/".to_string(),
                created_by: workspace_id,
                updated_by: workspace_id,
                created_at: now,
                updated_at: now,
            }],
            groups: vec![domain::McpGroupRecord {
                id: uuid::Uuid::from_u128(13),
                instance_record_id: instance_id,
                path: "/".to_string(),
                display_name: "Root".to_string(),
                description_short: None,
                enabled: true,
                sort_order: 0,
                created_by: workspace_id,
                updated_by: workspace_id,
                created_at: now,
                updated_at: now,
            }],
            tools: vec![domain::McpToolRecord {
                id: tool_id,
                workspace_id,
                tool_id: "lookup".to_string(),
                name: "Lookup".to_string(),
                short_description: "Lookup".to_string(),
                full_description: "Lookup".to_string(),
                execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                    interface_id: "lookup".to_string(),
                },
                parameter_schema: json!({
                    "type": "object",
                    "properties": {"workspace_id": {"type": "string"}, "query": {"type": "string"}},
                    "required": ["workspace_id", "query"]
                }),
                result_schema: json!({}),
                input_mapping: json!({"mappings": [
                    {"interface_param":"workspace_id","source":{"kind":"server_binding","binding":"workspace_id"},"required":true},
                    {"interface_param":"query","source":{"kind":"mcp_argument","path":"query"},"required":true}
                ]}),
                output_mapping: json!({"mappings": []}),
                permission_code: None,
                risk_level: domain::McpRiskLevel::Low,
                des_id: "revision".to_string(),
                des_id_required: false,
                status: McpToolStatus::Enabled,
                revision: 1,
                created_by: workspace_id,
                updated_by: workspace_id,
                created_at: now,
                updated_at: now,
            }],
            bindings: vec![domain::McpToolBindingRecord {
                id: uuid::Uuid::from_u128(14),
                instance_record_id: instance_id,
                tool_record_id: tool_id,
                group_path: "/".to_string(),
                tool_id: "lookup".to_string(),
                display_alias: None,
                visible: true,
                sort_order: 0,
                created_by: workspace_id,
                updated_by: workspace_id,
                created_at: now,
                updated_at: now,
            }],
            discovery_policies: Vec::new(),
        }
    }

    #[test]
    fn assistant_mcp_get_uses_mapped_schema_without_server_bound_workspace() {
        let catalog = catalog_with_server_bound_workspace();
        let scope = VirtualMcpScope::selected(&catalog, &["selected".to_string()]);
        let VirtualToolOutcome::Success(result) =
            get(&catalog, &scope, &json!({"tool_id": "lookup"}))
        else {
            panic!("selected tool should be visible");
        };
        let schema = &result["structuredContent"]["input_schema"];
        assert!(schema["properties"].get("workspace_id").is_none());
        assert_eq!(schema["required"], json!(["query"]));
    }

    #[test]
    fn assistant_mcp_scope_rejects_unselected_instance() {
        let catalog = catalog_with_server_bound_workspace();
        let scope = VirtualMcpScope::selected(&catalog, &[]);
        assert!(matches!(
            get(&catalog, &scope, &json!({"tool_id": "lookup"})),
            VirtualToolOutcome::Error { code: -32602, .. }
        ));
    }

    #[test]
    fn assistant_mcp_scope_rejects_disabled_and_ambiguous_selected_instances() {
        let mut disabled = catalog_with_server_bound_workspace();
        disabled.instances[0].status = McpInstanceStatus::Disabled;
        let scope = VirtualMcpScope::selected(&disabled, &["selected".to_string()]);
        assert!(matches!(
            get(&disabled, &scope, &json!({"tool_id": "lookup"})),
            VirtualToolOutcome::Error { code: -32602, .. }
        ));

        let mut ambiguous = catalog_with_server_bound_workspace();
        let mut second_instance = ambiguous.instances[0].clone();
        second_instance.id = uuid::Uuid::from_u128(21);
        second_instance.instance_id = "second".to_string();
        let mut second_group = ambiguous.groups[0].clone();
        second_group.id = uuid::Uuid::from_u128(22);
        second_group.instance_record_id = second_instance.id;
        let mut second_binding = ambiguous.bindings[0].clone();
        second_binding.id = uuid::Uuid::from_u128(23);
        second_binding.instance_record_id = second_instance.id;
        ambiguous.instances.push(second_instance);
        ambiguous.groups.push(second_group);
        ambiguous.bindings.push(second_binding);
        let scope =
            VirtualMcpScope::selected(&ambiguous, &["selected".to_string(), "second".to_string()]);
        assert!(matches!(
            get(&ambiguous, &scope, &json!({"tool_id": "lookup"})),
            VirtualToolOutcome::Error { code: -32602, .. }
        ));
    }
}
