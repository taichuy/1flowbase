use super::*;
fn manifest() -> plugin_framework::PluginManifestV1 {
    let mut manifest = json!({
        "manifest_version":2,"plugin_id":"chat_fixture","version":"1.0.0","publisher_namespace":"acme",
        "vendor":"acme","display_name":"Chat fixture","description":"Scoped chat fixture",
        "source_kind":"uploaded","trust_level":"unverified","consumption_kind":"runtime_extension",
        "execution_mode":"process_per_call","slot_codes":[],"selection_mode":"assignment_then_select",
        "minimum_host_version":"0.1.0","contract_version":"1flowbase.extension-bus/v1",
        "schema_version":"1flowbase.plugin.manifest/v2","binding_targets":["workspace"],
        "permissions":{"network":"none","secrets":"none","storage":"none","mcp":"none","subprocess":"deny"},
        "runtime":{"entry":"bin/fixture","protocol":"stdio_json"},
        "managed":{
            "module":{"bus_version":"v1","module_id":"chat_fixture","module_version":"1.0.0","module_kind":"runtime",
                "contributions":[{"contribution_id":"chat_fixture.send","contributor_module_id":"chat_fixture",
                    "point_id":"1flowbase.notification.send","contract_version":"1flowbase.notification/v1",
                    "required_permissions":["notification.send"],"mode":"append"},
                    {"contribution_id":"chat_fixture.receive","contributor_module_id":"chat_fixture",
                    "point_id":"1flowbase.notification.receive","contract_version":"1flowbase.notification/v1",
                    "required_permissions":["notification.receive"],"mode":"append"}]},
            "execution_bindings":[{"contribution_id":"chat_fixture.send","execution_mode":"process_per_call",
                "runtime":{"protocol":"stdio_json","entry":"bin/fixture"},"handler":"send",
                "payload":{"kind":"notification_type","type_code":"acme/chat_fixture/private"}},
                {"contribution_id":"chat_fixture.receive","execution_mode":"process_per_call",
                "runtime":{"protocol":"stdio_json","entry":"bin/fixture"},"handler":"receive",
                "payload":{"kind":"notification_receive","type_code":"acme/chat_fixture/private"}}]
        }
    });
    let tools:Vec<_> = ["history_messages","history_files","read_file","search_messages","file_info","view_image"].into_iter().map(|name|json!({
        "name":name,"description":name,"operation":name,
        "input_schema":{"type":"object","properties":{"message_id":{"type":"string"}},"required":["message_id"]}
    })).collect();
    manifest["notification_types"] = json!([{
        "type_code":"acme/chat_fixture/private","display_name":"Chat","description":"Chat",
        "config_schema":{"type":"object","properties":{}},"config_form":{"schema_version":"1.0.0","fields":[]},
        "input_schema":{"type":"object","properties":{}},"input_form":{"schema_version":"1.0.0","fields":[]},
        "output_schema":{"type":"object"},"contribution_id":"chat_fixture.send",
        "receive":{"contribution_id":"chat_fixture.receive","default_system_prompt":"Use tools on demand",
            "mcp_instance_id":"wechat-chat-channels","mcp_tools":tools}
    }]);
    plugin_framework::parse_plugin_manifest(&manifest.to_string()).unwrap()
}
#[test]
fn shared_notification_graph_resets_complete_owned_definition_and_exposes_native_image_only_on_demand(
) {
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");
    let installation = Uuid::now_v7();
    let mut manifest = manifest();
    let graph = prepare(&actor, installation, &manifest).unwrap().unwrap();
    assert!(
        matches!(graph.kind,McpBundleSeedKind::PluginTemplateReset{installation_id} if installation_id==installation)
    );
    assert_eq!(graph.instances[0].instance_id, "wechat-chat-channels");
    assert_eq!(graph.instances[0].name, "wechat-chat-channels");
    assert_eq!(graph.source.bundle_id, "chat_fixture");
    assert_eq!(graph.tools.len(), 6);
    for tool in &graph.tools {
        assert!(tool.tool_id.starts_with("plugin.chat_fixture."));
        assert!(tool.parameter_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("instance_id")));
        domain::mcp_management::validate_mcp_call_parameters(&tool.input_mapping).unwrap();
        if tool.name == "view_image" {
            assert_eq!(tool.output_mapping["mcp_content"][0]["type"], "image");
            assert_eq!(
                tool.execution_target.interface_id(),
                Some("communications_file_image")
            );
        } else {
            assert_eq!(tool.output_mapping, json!({}));
        }
    }
    manifest.notification_types[0]
        .receive
        .as_mut()
        .unwrap()
        .mcp_tools
        .pop();
    assert_eq!(
        prepare(&actor, installation, &manifest)
            .unwrap()
            .unwrap()
            .tools
            .len(),
        5
    );
    // Multiple declarations cannot compete to reset one plugin's namespace.
    manifest
        .notification_types
        .push(manifest.notification_types[0].clone());
    assert!(prepare(&actor, installation, &manifest).is_err());
}
