use crate::{
    McpBundleGroup, McpBundleInstance, McpBundleInstanceDiscoveryPolicy, McpBundleManifest,
    McpBundlePackage, McpBundleTool, McpBundleToolBinding, McpBundleUpstreamConnection,
    McpInstanceStatus, McpRiskLevel, McpToolExecutionTarget, McpToolStatus, McpUpstreamAuthType,
    McpUpstreamConnectionStatus, McpUpstreamTransport, MCP_BUNDLE_SCHEMA_VERSION,
};
use uuid::Uuid;

#[test]
fn ac_002_project_instance_retains_only_its_referenced_graph() {
    let selected_connection_id = Uuid::now_v7();
    let other_connection_id = Uuid::now_v7();
    let package = McpBundlePackage {
        manifest: McpBundleManifest {
            schema_version: MCP_BUNDLE_SCHEMA_VERSION.to_string(),
            organization: "1flowbase".to_string(),
            bundle_id: "fixture".to_string(),
            bundle_version: "1.0.0".to_string(),
            locale: "zh_Hans".to_string(),
            minimum_host_version: "0.1.0".to_string(),
            exported_from_system_version: "0.1.0".to_string(),
            exported_at: "2026-08-17T00:00:00Z".to_string(),
            files: Vec::new(),
        },
        tools: vec![
            proxy_tool("selected_tool", selected_connection_id),
            proxy_tool("other_tool", other_connection_id),
        ],
        instances: vec![
            instance("selected", "selected_tool"),
            instance("other", "other_tool"),
        ],
        connections: vec![
            connection(selected_connection_id, "selected"),
            connection(other_connection_id, "other"),
        ],
    };

    let projected = package.project_instance("selected").unwrap();

    assert_eq!(projected.instances.len(), 1);
    assert_eq!(projected.instances[0].instance_id, "selected");
    assert_eq!(projected.tools.len(), 1);
    assert_eq!(projected.tools[0].tool_id, "selected_tool");
    assert_eq!(projected.connections.len(), 1);
    assert_eq!(
        projected.connections[0].connection_id,
        selected_connection_id
    );
    assert_eq!(projected.manifest.bundle_id, "fixture");
}

fn instance(instance_id: &str, tool_id: &str) -> McpBundleInstance {
    McpBundleInstance {
        instance_id: instance_id.to_string(),
        name: instance_id.to_string(),
        description_short: None,
        status: McpInstanceStatus::Enabled,
        default_entry_path: "/tools".to_string(),
        groups: vec![McpBundleGroup {
            path: "/tools".to_string(),
            display_name: "Tools".to_string(),
            description_short: None,
            enabled: true,
            sort_order: 0,
        }],
        bindings: vec![McpBundleToolBinding {
            group_path: "/tools".to_string(),
            tool_id: tool_id.to_string(),
            display_alias: None,
            visible: true,
            sort_order: 0,
        }],
        discovery_policy: McpBundleInstanceDiscoveryPolicy {
            list_default_limit: 20,
            list_max_depth: 3,
            list_regex_enabled: false,
            list_regex_max_length: 0,
            list_return_fields: serde_json::json!([]),
        },
    }
}

fn proxy_tool(tool_id: &str, upstream_connection_id: Uuid) -> McpBundleTool {
    McpBundleTool {
        tool_id: tool_id.to_string(),
        name: tool_id.to_string(),
        short_description: tool_id.to_string(),
        full_description: tool_id.to_string(),
        execution_target: McpToolExecutionTarget::McpProxy {
            upstream_connection_id,
            remote_tool_name: tool_id.to_string(),
            source_schema_hash: "sha256:fixture".to_string(),
        },
        parameter_schema_snapshot: serde_json::json!({}),
        result_schema_snapshot: serde_json::json!({}),
        input_mapping: serde_json::json!({}),
        output_mapping: serde_json::json!({}),
        permission_code_snapshot: None,
        risk_level_snapshot: McpRiskLevel::Low,
        status: McpToolStatus::Enabled,
    }
}

fn connection(connection_id: Uuid, name: &str) -> McpBundleUpstreamConnection {
    McpBundleUpstreamConnection {
        connection_id,
        name: name.to_string(),
        endpoint: "https://example.test/mcp".to_string(),
        transport: McpUpstreamTransport::StreamableHttp,
        auth_type: McpUpstreamAuthType::None,
        custom_header_name: None,
        status: McpUpstreamConnectionStatus::Enabled,
    }
}
