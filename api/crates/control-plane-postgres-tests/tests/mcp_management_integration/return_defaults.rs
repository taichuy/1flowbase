use super::*;

#[tokio::test]
async fn issue_2176_mcp_return_defaults_persist_through_service_and_repository() {
    use control_plane::mcp_management::UpdateMcpToolCommand;
    let (store, workspace, actor) = seed_store().await;
    let service = McpManagementService::new(store.clone());
    let tool = service
        .create_tool(CreateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: "return_defaults".into(),
            des_id: None,
            name: "Return defaults".into(),
            short_description: "Read configured fields".into(),
            full_description: String::new(),
            interface_entry: runtime_profile_interface(),
            input_mapping: serde_json::json!({}),
            output_mapping: serde_json::json!({}),
            max_inline_chars: None,
            response_fields: None,
            status: domain::McpToolStatus::Enabled,
        })
        .await
        .unwrap();
    assert_eq!(tool.max_inline_chars, None);
    assert_eq!(tool.response_fields, None);
    let tool = service
        .update_tool(UpdateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: tool.tool_id.clone(),
            des_id: Some(tool.des_id.clone()),
            name: tool.name.clone(),
            short_description: tool.short_description.clone(),
            full_description: tool.full_description.clone(),
            interface_entry: runtime_profile_interface(),
            input_mapping: tool.input_mapping.clone(),
            output_mapping: tool.output_mapping.clone(),
            max_inline_chars: Some(80_000),
            response_fields: Some(vec!["/items/0/body".into()]),
            status: domain::McpToolStatus::Enabled,
        })
        .await
        .unwrap();
    assert_eq!(tool.max_inline_chars, Some(80_000));
    let stored = store
        .get_mcp_tool(workspace.id, &tool.tool_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.response_fields, Some(vec!["/items/0/body".into()]));
    let invalid = service
        .update_tool(UpdateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: tool.tool_id.clone(),
            des_id: Some(tool.des_id.clone()),
            name: tool.name.clone(),
            short_description: tool.short_description.clone(),
            full_description: tool.full_description.clone(),
            interface_entry: runtime_profile_interface(),
            input_mapping: tool.input_mapping.clone(),
            output_mapping: tool.output_mapping.clone(),
            max_inline_chars: Some(0),
            response_fields: Some(vec![]),
            status: domain::McpToolStatus::Enabled,
        })
        .await;
    assert!(invalid.is_err());
    let preserved = store
        .get_mcp_tool(workspace.id, &tool.tool_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preserved.max_inline_chars, Some(80_000));
    assert_eq!(preserved.response_fields, stored.response_fields);
}
