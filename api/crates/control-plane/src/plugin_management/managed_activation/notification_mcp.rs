//! Plugin-owned notification MCP graph. Operations are a closed Host contract.
use crate::ports::{CreateMcpToolInput, McpBundleSeedKind, SeedMcpBundleGraphInput};
use anyhow::Result;
use domain::*;
use extension_contracts::NotificationMcpOperation as Op;
use serde_json::{json, Value};
use uuid::Uuid;

pub(super) fn prepare(
    actor: &ActorContext,
    installation_id: Uuid,
    manifest: &plugin_framework::PluginManifestV1,
) -> Result<Option<SeedMcpBundleGraphInput>> {
    let declarations: Vec<_> = manifest
        .notification_types
        .iter()
        .filter_map(|t| t.receive.as_ref())
        .filter(|r| r.mcp_instance_id.is_some())
        .collect();
    if declarations.is_empty() {
        return Ok(None);
    }
    // One plugin owns one complete graph; two reset templates cannot prune each other.
    if declarations.len() != 1
        || manifest
            .managed_service
            .as_ref()
            .is_some_and(|s| s.mcp_instance.is_some())
    {
        return Err(crate::errors::ControlPlaneError::InvalidInput(
            "notification_mcp_single_owner",
        )
        .into());
    }
    let receive = declarations[0];
    receive.validate()?;
    let id = receive.mcp_instance_id.as_ref().expect("filtered");
    let code = manifest.plugin_code()?;
    let tools: Vec<_> = receive.mcp_tools.iter().map(|tool| {
        // MCP catalog identifiers are the public OpenAPI operation_id values.
        // Canonical invocation still resolves the route to its frozen Kernel binding.
        let operation = match tool.operation {
            Op::HistoryMessages | Op::SearchMessages => "communications_messages",
            Op::HistoryFiles => "communications_files",
            Op::ReadFile => "communications_file",
            Op::FileInfo => "communications_file_info",
            Op::ViewImage => "communications_file_image",
        };
        let mut schema = tool.input_schema.clone();
        schema["properties"]["instance_id"] = json!({"type":"string","format":"uuid","description":"channel_id from Host session information; never an MCP instance ID"});
        let mut required_fields = schema.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
        required_fields.push(json!("instance_id"));
        schema["required"] = json!(required_fields);
        let properties = schema["properties"].as_object().expect("object schema");
        let required = schema["required"].as_array().expect("required");
        let parameters: Vec<_> = properties.iter().map(|(name, property)| json!({
            "name":name,"field_type":property["type"],"parameter_type":"url","schema":property,
            "description":property["description"],"required":required.contains(&json!(name))
        })).collect();
        let mappings: Vec<_> = properties.iter().map(|(name, property)| { let mut mapping = json!({
            "interface_param":name,"mcp_param":name,"source":{"kind":"mcp_argument","path":name},
            "description":property.get("description").and_then(Value::as_str).unwrap_or(""),"required":required.contains(&json!(name))
        }); if let Some(default) = property.get("default") { mapping["default_value"] = default.clone(); } mapping }).collect();
        CreateMcpToolInput {
            id: Uuid::now_v7(), actor_user_id: actor.user_id, workspace_id: actor.current_workspace_id,
            tool_id: format!("plugin.{code}.{}",tool.name), name: tool.name.clone(),
            short_description: tool.description.clone(), full_description: tool.description.clone(),
            execution_target: McpToolExecutionTarget::InterfaceWrapper { interface_id: operation.into() },
            parameter_schema: schema, result_schema: json!({}), input_mapping: json!({"interface_parameters":parameters,"mappings":mappings}),
            output_mapping: if tool.operation == Op::ViewImage { json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"}) } else { json!({}) },
            max_inline_chars: None, response_fields: None, permission_code: None, risk_level: McpRiskLevel::Low,
            des_id: Uuid::now_v7().simple().to_string()[..8].into(), des_id_required: false, status: McpToolStatus::Enabled,
        }
    }).collect();
    Ok(Some(SeedMcpBundleGraphInput {
        kind: McpBundleSeedKind::PluginTemplateReset { installation_id },
        actor_user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        source: McpManagedBundleSource {
            organization: manifest.publisher_namespace.clone(),
            bundle_id: code.into(),
            bundle_version: manifest.version.clone(),
        },
        instances: vec![McpBundleInstance {
            instance_id: id.clone(),
            name: id.clone(),
            description_short: Some(
                "Conversation history, search, file metadata and on-demand images".into(),
            ),
            status: McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
            groups: vec![McpBundleGroup {
                path: "/".into(),
                display_name: id.clone(),
                description_short: None,
                enabled: true,
                sort_order: 0,
            }],
            bindings: tools
                .iter()
                .enumerate()
                .map(|(i, t)| McpBundleToolBinding {
                    tool_id: t.tool_id.clone(),
                    group_path: "/".into(),
                    display_alias: None,
                    visible: true,
                    sort_order: i as i32,
                })
                .collect(),
            discovery_policy: McpBundleInstanceDiscoveryPolicy {
                list_default_limit: 20,
                list_max_depth: 3,
                list_regex_enabled: false,
                list_regex_max_length: 64,
                list_return_fields: json!(["id", "name", "short_description"]),
            },
        }],
        tools,
    }))
}

#[cfg(test)]
#[path = "_tests/notification_mcp.rs"]
mod tests;
