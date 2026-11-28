use super::*;
use control_plane::ports::{McpInstancePageQuery, McpToolPageQuery};
use domain::{McpInstanceStatus, McpToolExecutionTarget};
use serde_json::json;

#[tokio::test]
async fn progressive_queries_filter_and_count_the_actual_workspace_rows() {
    let (store, workspace, actor) = seed_store().await;
    let workspace_id = workspace.id;
    let mut input =
        managed_frontstage_seed_input(actor.id, workspace_id, managed_frontstage_package("1.0.2"));
    seed_builtin_bundle_installation(&store, actor.id, "1.0.2").await;
    input.tools[0].execution_target = McpToolExecutionTarget::InterfaceWrapper {
        interface_id: "get_runtime_profile".into(),
    };
    input.tools[0].input_mapping = json!({"mappings":[{"interface_param":"des_id","source":{"kind":"mcp_call"},"required":true}]});
    input.tools[1].input_mapping = json!({"mappings":[{"interface_param":"des_id","source":{"kind":"mcp_call"},"required":"true"}]});
    store
        .seed_mcp_bundle_graph_once_atomically(&input)
        .await
        .unwrap();
    store
        .create_mcp_instance(&CreateMcpInstanceInput {
            id: uuid::Uuid::now_v7(),
            actor_user_id: actor.id,
            workspace_id,
            instance_id: "empty".into(),
            name: "Empty".into(),
            description_short: Some("literal_%".into()),
            status: McpInstanceStatus::Draft,
            default_entry_path: "/".into(),
            webmcp_exposure: domain::WebMcpExposure::Disabled,
        })
        .await
        .unwrap();
    let summaries = store
        .list_mcp_instances_with_counts(workspace_id, None)
        .await
        .unwrap();
    assert_eq!(summaries[0].instance.instance_id, "empty");
    assert_eq!((summaries[0].group_count, summaries[0].tool_count), (0, 0));
    let enabled = store
        .list_mcp_instances_with_counts(workspace_id, Some(McpInstanceStatus::Enabled))
        .await
        .unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!((enabled[0].group_count, enabled[0].tool_count), (1, 2));
    assert_eq!(
        store
            .get_mcp_instance_directory_counts(enabled[0].instance.id)
            .await
            .unwrap(),
        (1, 2)
    );
    let mut instances = McpInstancePageQuery {
        page: 2,
        page_size: 1,
        status: None,
        keyword: None,
    };
    let page = store
        .list_mcp_instances_page(workspace_id, &instances)
        .await
        .unwrap();
    assert_eq!((page.total, page.page, page.page_size), (2, 2, 1));
    assert_eq!(page.items[0].instance.instance_id, "frontstage_browser");
    instances.page = 1;
    instances.status = Some(McpInstanceStatus::Enabled);
    instances.keyword = Some("mANagEd".into());
    assert_eq!(
        store
            .list_mcp_instances_page(workspace_id, &instances)
            .await
            .unwrap()
            .total,
        1
    );
    instances.status = None;
    instances.keyword = Some("_%".into());
    assert_eq!(
        store
            .list_mcp_instances_page(workspace_id, &instances)
            .await
            .unwrap()
            .items[0]
            .instance
            .instance_id,
        "empty"
    );

    let mut tools = McpToolPageQuery {
        page: 1,
        page_size: 1,
        keyword: None,
        interface_operations: Default::default(),
        execution_target_kind: None,
        interface_id: None,
        risk_level: None,
        status: None,
        des_id_required: None,
        tool_ids: None,
    };
    let full = store.list_mcp_tools(workspace_id).await.unwrap();
    for page_number in [1, 2, 3] {
        tools.page = page_number;
        let page = store
            .list_mcp_tools_page(workspace_id, &tools)
            .await
            .unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(
            page.items.first().map(|tool| &tool.tool_id),
            full.get(page_number - 1).map(|tool| &tool.tool_id)
        );
        if let Some(item) = page.items.first() {
            assert_eq!(
                item.des_id_required,
                domain::mcp_management::mcp_des_id_required(&full[page_number - 1].input_mapping)
            );
            assert!(item.managed_by.is_some());
        }
    }
    tools.page = 1;
    // The second seeded tool is an Assistant client. Match its displayed operation,
    // including a search crossing from the tool ID into the operation's prefix.
    let assistant = &input.tools[1];
    let McpToolExecutionTarget::AssistantClient { capability_code } = &assistant.execution_target
    else {
        panic!("expected an Assistant client in the bundle")
    };
    for keyword in [
        format!("Assistant browser capability {capability_code}"),
        format!("{} ASSISTANT browser", assistant.tool_id),
    ] {
        tools.keyword = Some(keyword);
        let result = store
            .list_mcp_tools_page(workspace_id, &tools)
            .await
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].tool_id, assistant.tool_id);
    }
    tools.keyword = Some("GET /api/console/system/runtime-profile".into());
    tools.interface_operations.insert(
        "get_runtime_profile".into(),
        "GET /api/console/system/runtime-profile".into(),
    );
    tools.execution_target_kind = Some("interface_wrapper".into());
    tools.interface_id = Some("get_runtime_profile".into());
    tools.risk_level = Some("low".into());
    tools.status = Some("enabled".into());
    tools.des_id_required = Some(true);
    tools.tool_ids = Some(vec!["frontstage_list_page_blocks".into()]);
    let result = store
        .list_mcp_tools_page(workspace_id, &tools)
        .await
        .unwrap();
    assert_eq!(result.total, 1);
    assert!(result.items[0].des_id_required);
    tools.des_id_required = Some(false);
    assert_eq!(
        store
            .list_mcp_tools_page(workspace_id, &tools)
            .await
            .unwrap()
            .total,
        0
    );
    tools.des_id_required = Some(true);
    tools.interface_operations.clear();
    assert_eq!(
        store
            .list_mcp_tools_page(workspace_id, &tools)
            .await
            .unwrap()
            .total,
        0
    );
}
