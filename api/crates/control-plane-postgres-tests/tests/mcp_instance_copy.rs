mod support;

use control_plane::mcp_management::{
    CopyMcpInstanceCommand, CreateMcpInstanceCommand, CreateMcpToolBindingCommand,
    CreateMcpToolCommand, McpManagementService, SaveMcpClientCredentialCommand,
    UpdateMcpInstanceDiscoveryPolicyCommand, UpsertMcpGroupCommand,
};

#[tokio::test]
async fn mcp_instance_copy_reuses_tools_and_excludes_client_credentials() {
    let (store, _workspace, actor) = support::seed_store().await;
    let service = McpManagementService::new(store);
    let source = service
        .create_instance(CreateMcpInstanceCommand {
            actor_user_id: actor.id,
            instance_id: "source_ops".into(),
            name: "Source Operations".into(),
            description_short: Some("Copy this configuration".into()),
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/ops".into(),
        })
        .await
        .unwrap();
    let tool = service
        .create_tool(CreateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: "runtime_profile_copy_source".into(),
            name: "Runtime Profile".into(),
            short_description: "Read runtime profile".into(),
            full_description: "Read the current runtime profile.".into(),
            interface_entry: support::runtime_profile_interface(),
            input_mapping: serde_json::json!({}),
            output_mapping: serde_json::json!({}),
            des_id: None,
            status: domain::McpToolStatus::Enabled,
        })
        .await
        .unwrap();
    service
        .upsert_group(UpsertMcpGroupCommand {
            actor_user_id: actor.id,
            instance_id: source.instance_id.clone(),
            path: "/ops".into(),
            display_name: "Operations".into(),
            description_short: Some("Operational tools".into()),
            enabled: true,
            sort_order: 7,
        })
        .await
        .unwrap();
    service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: source.instance_id.clone(),
            group_path: "/ops".into(),
            tool_id: tool.tool_id.clone(),
            display_alias: Some("Runtime copy".into()),
            visible: false,
            sort_order: 11,
        })
        .await
        .unwrap();
    service
        .update_instance_discovery_policy(UpdateMcpInstanceDiscoveryPolicyCommand {
            actor_user_id: actor.id,
            instance_id: source.instance_id.clone(),
            list_default_limit: 17,
            list_max_depth: 5,
            list_regex_enabled: true,
            list_regex_max_length: 64,
            list_return_fields: serde_json::json!(["id", "name", "path"]),
        })
        .await
        .unwrap();
    service
        .save_client_credential(SaveMcpClientCredentialCommand {
            actor_user_id: actor.id,
            instance_id: source.instance_id.clone(),
            api_key: "source-secret".into(),
            master_key: "test-master-key".into(),
        })
        .await
        .unwrap();

    let copied = service
        .copy_instance(CopyMcpInstanceCommand {
            actor_user_id: actor.id,
            source_instance_id: source.instance_id.clone(),
            instance_id: "copied_ops".into(),
            name: "Copied Operations".into(),
        })
        .await
        .unwrap();
    assert_eq!(copied.status, domain::McpInstanceStatus::Draft);
    assert_eq!(
        copied.description_short.as_deref(),
        Some("Copy this configuration")
    );
    assert_eq!(copied.default_entry_path, "/ops");

    let catalog = service.read_workspace_catalog(actor.id).await.unwrap();
    let copied_groups = catalog
        .groups
        .iter()
        .filter(|group| group.instance_record_id == copied.id)
        .collect::<Vec<_>>();
    assert_eq!(copied_groups.len(), 1);
    assert_eq!(copied_groups[0].path, "/ops");
    assert_eq!(copied_groups[0].sort_order, 7);
    let copied_bindings = catalog
        .bindings
        .iter()
        .filter(|binding| binding.instance_record_id == copied.id)
        .collect::<Vec<_>>();
    assert_eq!(copied_bindings.len(), 1);
    assert_eq!(copied_bindings[0].tool_record_id, tool.id);
    assert_eq!(
        copied_bindings[0].display_alias.as_deref(),
        Some("Runtime copy")
    );
    assert!(!copied_bindings[0].visible);
    assert_eq!(copied_bindings[0].sort_order, 11);
    let copied_policy = catalog
        .discovery_policies
        .iter()
        .find(|policy| policy.instance_record_id == copied.id)
        .unwrap();
    assert_eq!(copied_policy.list_default_limit, 17);
    assert_eq!(copied_policy.list_max_depth, 5);
    assert!(copied_policy.list_regex_enabled);
    assert_eq!(copied_policy.list_regex_max_length, 64);
    assert_eq!(
        service
            .get_client_credential(actor.id, &source.instance_id, "test-master-key")
            .await
            .unwrap()
            .as_deref(),
        Some("source-secret")
    );
    assert!(service
        .get_client_credential(actor.id, &copied.instance_id, "test-master-key")
        .await
        .unwrap()
        .is_none());

    let duplicate = service
        .copy_instance(CopyMcpInstanceCommand {
            actor_user_id: actor.id,
            source_instance_id: source.instance_id,
            instance_id: copied.instance_id.clone(),
            name: "Duplicate Copy".into(),
        })
        .await;
    assert!(duplicate.is_err());
    let catalog_after_conflict = service.read_workspace_catalog(actor.id).await.unwrap();
    assert_eq!(
        catalog_after_conflict
            .instances
            .iter()
            .filter(|instance| instance.instance_id == copied.instance_id)
            .count(),
        1
    );
}
