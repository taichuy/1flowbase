use super::*;
use utoipa::OpenApi;

#[test]
fn preference_is_scoped_to_current_workspace() {
    let current = Uuid::from_u128(1);
    let other = Uuid::from_u128(2);
    let application = Uuid::from_u128(3);
    let meta = json!({ ASSISTANT_META_KEY: { "workspaces": {
        current.to_string(): { "application_id": application, "mcp_instance_ids": ["alpha"] },
        other.to_string(): { "application_id": Uuid::from_u128(4), "mcp_instance_ids": ["beta"] }
    }}});
    assert_eq!(
        read_preference(&meta, current).application_id,
        Some(application)
    );
    assert_eq!(
        read_preference(&meta, current).mcp_instance_ids,
        vec!["alpha"]
    );
    assert_eq!(
        read_preference(&meta, current).enabled_client_tools,
        vec![
            AssistantClientToolId::GetClientContext,
            AssistantClientToolId::RefreshClientView,
        ]
    );
}

#[test]
fn ac_001_assistant_client_tools_default_enabled_and_preserve_explicit_disable() {
    let workspace_id = Uuid::from_u128(20);
    assert_eq!(
        read_preference(&json!({}), workspace_id).enabled_client_tools,
        vec![
            AssistantClientToolId::GetClientContext,
            AssistantClientToolId::RefreshClientView,
        ]
    );

    let meta = json!({ ASSISTANT_META_KEY: { "workspaces": {
        workspace_id.to_string(): {
            "application_id": null,
            "mcp_instance_ids": [],
            "enabled_client_tools": ["get_client_context"]
        }
    }}});
    assert_eq!(
        read_preference(&meta, workspace_id).enabled_client_tools,
        vec![AssistantClientToolId::GetClientContext]
    );
}

#[test]
fn assistant_routes_are_authenticated_console_routes() {
    let assembly = route_assembly();
    let bindings = assembly.bindings();
    assert!(bindings.iter().any(|binding| {
        binding.route.method == "GET"
            && binding.route.path == "/api/console/assistant/settings"
            && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
    }));
    assert!(bindings.iter().any(|binding| {
        binding.route.method == "POST"
            && binding.route.path == "/api/console/assistant/runs"
            && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
    }));
    assert!(bindings.iter().any(|binding| {
        binding.route.method == "POST"
            && binding.route.path == "/api/console/assistant/runs/stream"
            && binding.ownership == access_control::ConsoleRouteOwnership::Authenticated
    }));
}

#[test]
fn assistant_console_routes_have_static_openapi_identities() {
    let document = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();

    for (method, path, operation_id) in [
        (
            "get",
            "/api/console/assistant/settings",
            "assistant_get_settings",
        ),
        (
            "patch",
            "/api/console/assistant/settings",
            "assistant_update_settings",
        ),
        ("post", "/api/console/assistant/runs", "assistant_start_run"),
        (
            "post",
            "/api/console/assistant/runs/stream",
            "assistant_start_run_stream",
        ),
    ] {
        assert_eq!(document["paths"][path][method]["operationId"], operation_id);
        assert!(
            document["paths"][path][method]["responses"]
                .get("403")
                .is_some(),
            "{method} {path} documents the API-key principal rejection"
        );
    }
}

#[test]
fn assistant_mcp_provider_exposes_only_virtual_ui_meta_tools() {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    let workspace_id = Uuid::from_u128(10);
    let instance_id = Uuid::from_u128(11);
    let tool_id = Uuid::from_u128(12);
    let instance = domain::McpInstanceRecord {
        id: instance_id,
        workspace_id,
        instance_id: "catalog".to_string(),
        name: "Catalog".to_string(),
        description_short: None,
        status: domain::McpInstanceStatus::Enabled,
        default_entry_path: "/".to_string(),
        webmcp_exposure: domain::WebMcpExposure::Disabled,
        managed_by: None,
        created_by: workspace_id,
        updated_by: workspace_id,
        created_at: now,
        updated_at: now,
    };
    let tool = domain::McpToolRecord {
        max_inline_chars: None,
        response_fields: None,
        id: tool_id,
        workspace_id,
        tool_id: "lookup".to_string(),
        name: "Lookup".to_string(),
        short_description: "Lookup".to_string(),
        full_description: "Lookup a catalog item".to_string(),
        execution_target: domain::McpToolExecutionTarget::InterfaceWrapper {
            interface_id: "lookup".to_string(),
        },
        parameter_schema: json!({"type":"object"}),
        result_schema: json!({}),
        input_mapping: json!({"mappings": []}),
        output_mapping: json!({"mappings": []}),
        permission_code: None,
        risk_level: domain::McpRiskLevel::Low,
        des_id: "revision".to_string(),
        des_id_required: false,
        status: domain::McpToolStatus::Enabled,
        revision: 1,
        managed_by: None,
        created_by: workspace_id,
        updated_by: workspace_id,
        created_at: now,
        updated_at: now,
    };
    let group = |id, path: &str| domain::McpGroupRecord {
        id,
        instance_record_id: instance_id,
        path: path.to_string(),
        display_name: path.to_string(),
        description_short: None,
        enabled: true,
        sort_order: 0,
        created_by: workspace_id,
        updated_by: workspace_id,
        created_at: now,
        updated_at: now,
    };
    let binding = |id, path: &str| domain::McpToolBindingRecord {
        id,
        instance_record_id: instance_id,
        tool_record_id: tool_id,
        group_path: path.to_string(),
        tool_id: "lookup".to_string(),
        display_alias: None,
        visible: true,
        sort_order: 0,
        created_by: workspace_id,
        updated_by: workspace_id,
        created_at: now,
        updated_at: now,
    };
    let catalog = domain::McpCatalogSnapshot {
        instances: vec![instance],
        groups: vec![
            group(Uuid::from_u128(13), "/one"),
            group(Uuid::from_u128(14), "/two"),
        ],
        tools: vec![tool],
        bindings: vec![
            binding(Uuid::from_u128(15), "/one"),
            binding(Uuid::from_u128(16), "/two"),
        ],
        discovery_policies: Vec::new(),
    };

    let scope = VirtualMcpScope::selected(&catalog, &["catalog".to_string()]);
    let provider_tools = virtual_ui::provider_tools(&catalog, &scope);

    assert_eq!(provider_tools.len(), 4);
    assert_eq!(
        provider_tools
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "catalog_mcp_list",
            "catalog_mcp_get",
            "catalog_mcp_result",
            "catalog_mcp_call"
        ]
    );
    assert!(provider_tools
        .iter()
        .all(
            |tool| tool["function"]["name"].as_str().is_some_and(|name| name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
        ));
    assert!(provider_tools.iter().all(|tool| {
        !tool["function"]["parameters"]
            .to_string()
            .contains("workspace_id")
    }));
    assert!(provider_tools[0]["function"]["parameters"]["properties"]
        .get("path_regex")
        .is_none());
}

#[tokio::test]
async fn expired_state_factory_returns_error_instead_of_reconstructing_or_selecting_latest() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let factory = StateAssistantRuntimeToolInvokerFactory {
        state: Arc::downgrade(&state),
    };
    let actor = domain::ActorContext::root(Uuid::nil(), state.bootstrap_workspace_id, "root");
    drop(state);
    assert!(factory.for_actor(&actor).await.is_err());
}
