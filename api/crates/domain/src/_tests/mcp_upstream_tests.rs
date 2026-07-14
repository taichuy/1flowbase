use serde_json::json;

use crate::mcp_management::{
    apply_mcp_field_mapping, identity_mcp_field_mapping, McpCallToolResult, McpFieldMapping,
    McpToolExecutionTarget,
};

#[test]
fn issue_1246_ac_002_execution_target_is_a_tagged_contract() {
    let interface = McpToolExecutionTarget::InterfaceWrapper {
        interface_id: "get_runtime_profile".into(),
    };
    let proxy = McpToolExecutionTarget::McpProxy {
        upstream_connection_id: uuid::Uuid::nil(),
        remote_tool_name: "weather.lookup".into(),
        source_schema_hash: "sha256:test".into(),
    };

    assert_eq!(
        serde_json::to_value(interface).unwrap(),
        json!({"kind": "interface_wrapper", "interface_id": "get_runtime_profile"})
    );
    assert_eq!(
        serde_json::to_value(proxy).unwrap(),
        json!({
            "kind": "mcp_proxy",
            "upstream_connection_id": uuid::Uuid::nil(),
            "remote_tool_name": "weather.lookup",
            "source_schema_hash": "sha256:test"
        })
    );
}

#[test]
fn issue_1246_ac_010_ac_011_mapping_supports_identity_nested_rename_filter_and_required() {
    let schema = json!({
        "type": "object",
        "properties": {
            "city": {"type": "string"},
            "options": {
                "type": "object",
                "properties": {"units": {"type": "string"}}
            }
        },
        "required": ["city"]
    });
    let identity = identity_mcp_field_mapping(&schema);
    assert_eq!(identity.len(), 2);
    assert_eq!(
        identity
            .iter()
            .find(|entry| entry.source_path == "options.units")
            .map(|entry| entry.required),
        Some(false)
    );

    let mapping = vec![
        McpFieldMapping {
            source_path: "request.city".into(),
            target_path: "location.name".into(),
            required: true,
        },
        McpFieldMapping {
            source_path: "request.options.units".into(),
            target_path: "preferences.units".into(),
            required: false,
        },
    ];
    let mapped = apply_mcp_field_mapping(
        &json!({"request": {"city": "Shanghai", "options": {"units": "metric"}, "ignored": true}}),
        &mapping,
    )
    .unwrap();
    assert_eq!(
        mapped,
        json!({"location": {"name": "Shanghai"}, "preferences": {"units": "metric"}})
    );

    let error = apply_mcp_field_mapping(&json!({"request": {}}), &mapping).unwrap_err();
    assert_eq!(error.path(), "request.city");

    let invalid_optional = [McpFieldMapping {
        source_path: "request.items[0]".into(),
        target_path: "items.first".into(),
        required: false,
    }];
    let error = apply_mcp_field_mapping(&json!({"request": {}}), &invalid_optional).unwrap_err();
    assert_eq!(error.path(), "request.items[0]");
}

#[test]
fn issue_1246_ac_010_nested_identity_mapping_preserves_nested_required() {
    let schema = json!({
        "type": "object",
        "properties": {
            "options": {
                "type": "object",
                "properties": {"units": {"type": "string"}},
                "required": ["units"]
            }
        }
    });
    let mapping = identity_mcp_field_mapping(&schema);
    assert_eq!(mapping.len(), 1);
    assert_eq!(mapping[0].source_path, "options.units");
    assert!(mapping[0].required);
}

#[test]
fn issue_1246_ac_013_call_tool_mapping_preserves_content_and_error_semantics() {
    let upstream = McpCallToolResult {
        content: json!([{"type": "text", "text": "upstream failed"}]),
        structured_content: Some(json!({"weather": {"temperature": 28}})),
        is_error: Some(true),
    };
    let mapped = upstream
        .map_structured_content(&[McpFieldMapping {
            source_path: "weather.temperature".into(),
            target_path: "temperature_celsius".into(),
            required: true,
        }])
        .unwrap();

    assert_eq!(mapped.content, upstream.content);
    assert_eq!(mapped.is_error, Some(true));
    assert_eq!(
        mapped.structured_content,
        Some(json!({"temperature_celsius": 28}))
    );

    let content_only = McpCallToolResult {
        content: json!([{"type": "text", "text": "plain"}]),
        structured_content: None,
        is_error: None,
    };
    assert_eq!(
        content_only
            .map_structured_content(&identity_mcp_field_mapping(&json!({})))
            .unwrap(),
        content_only
    );
}

#[test]
fn issue_1246_ac_019_bundle_reads_v1_and_only_writes_tagged_target() {
    let legacy = json!({
        "tool_id":"weather","name":"Weather","short_description":"Weather",
        "full_description":"Weather","interface_id":"get_weather",
        "permission_code_snapshot":null,"risk_level_snapshot":"low","status":"enabled"
    });
    let tool: crate::McpBundleTool = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        tool.execution_target,
        McpToolExecutionTarget::InterfaceWrapper {
            interface_id: "get_weather".into()
        }
    );
    let exported = serde_json::to_value(tool).unwrap();
    assert_eq!(
        exported.pointer("/execution_target/kind"),
        Some(&json!("interface_wrapper"))
    );
    assert!(exported.get("interface_id").is_none());
}
