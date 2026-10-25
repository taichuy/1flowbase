//! Compile a verified plugin's initial, subsequently user-owned MCP configuration.
use super::mcp_projection::{input_mapping, media_result_schema, parameter_schema};
use crate::ports::{CreateMcpToolInput, McpBundleSeedKind, SeedMcpBundleGraphInput};
use anyhow::Result;
use serde_json::json;
use uuid::Uuid;

/// Called with the verified installed manifest before activation; no live HTTP catalog is needed.
pub fn prepare_managed_mcp_template(
    actor: &domain::ActorContext,
    installation_id: Uuid,
    manifest: &plugin_framework::PluginManifestV1,
) -> Result<Option<SeedMcpBundleGraphInput>> {
    let Some(service) = &manifest.managed_service else {
        return Ok(None);
    };
    let Some(instance) = &service.mcp_instance else {
        return Ok(None);
    };
    let plugin_code = manifest.plugin_code()?;
    let tools: Vec<_> = service
        .operations
        .iter()
        .filter_map(|operation| {
            let mcp = operation.mcp.as_ref()?;
            Some(CreateMcpToolInput {
                id: Uuid::now_v7(),
                actor_user_id: actor.user_id,
                workspace_id: actor.current_workspace_id,
                tool_id: format!("plugin.{plugin_code}.{}", mcp.name),
                name: mcp.name.clone(),
                short_description: mcp.description.clone(),
                full_description: mcp.description.clone(),
                execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
                    interface_id: operation.interface_id.clone(),
                },
                parameter_schema: parameter_schema(&operation.input_schema),
                result_schema: media_result_schema(
                    &operation.output_schema,
                    mcp.output_mapping.as_ref(),
                ),
                input_mapping: input_mapping(&operation.input_schema),
                output_mapping: mcp
                    .output_mapping
                    .clone()
                    .unwrap_or_else(|| operation.output_schema.clone()),
                max_inline_chars: None,
                response_fields: None,
                permission_code: None,
                risk_level: if operation.method == "GET" {
                    domain::McpRiskLevel::Low
                } else {
                    domain::McpRiskLevel::High
                },
                des_id: Uuid::now_v7().simple().to_string()[..8].to_string(),
                des_id_required: false,
                status: domain::McpToolStatus::Enabled,
            })
        })
        .collect();
    Ok(Some(SeedMcpBundleGraphInput {
        kind: McpBundleSeedKind::PluginTemplate { installation_id },
        actor_user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        source: domain::McpManagedBundleSource {
            organization: manifest.publisher_namespace.clone(),
            bundle_id: plugin_code.to_string(),
            bundle_version: manifest.version.clone(),
        },
        instances: vec![domain::McpBundleInstance {
            instance_id: instance.instance_id.clone(),
            name: instance.name.clone(),
            description_short: Some(instance.description.clone()),
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/".into(),
            groups: vec![domain::McpBundleGroup {
                path: "/".into(),
                display_name: instance.name.clone(),
                description_short: Some(instance.description.clone()),
                enabled: true,
                sort_order: 0,
            }],
            bindings: tools
                .iter()
                .enumerate()
                .map(|(index, tool)| domain::McpBundleToolBinding {
                    tool_id: tool.tool_id.clone(),
                    group_path: "/".into(),
                    display_alias: None,
                    visible: true,
                    sort_order: index as i32,
                })
                .collect(),
            discovery_policy: domain::McpBundleInstanceDiscoveryPolicy {
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
