use super::*;
#[test]
fn optional_query_omits_absent_properties_and_catalog_has_no_empty_locations() {
    let op = ApiOperationContext {
        interface_id: "mcp.interface_catalog.view".into(),
        method: "GET".into(),
        path: "/api/console/mcp/interface-capabilities".into(),
    };
    assert_eq!(
        McpCatalogInput::Interfaces(McpInterfaceCatalogQuery {
            bindable_only: None
        })
        .project_document_input(&op)
        .unwrap(),
        json!({"query":{}})
    );
    assert_eq!(
        McpCatalogInput::Interfaces(McpInterfaceCatalogQuery {
            bindable_only: Some(false)
        })
        .project_document_input(&op)
        .unwrap(),
        json!({"query":{"bindable_only":false}})
    );
    assert_eq!(
        McpCatalogInput::Catalog
            .project_document_input(&op)
            .unwrap(),
        json!({})
    );
}
#[test]
fn list_output_preserves_every_record_and_long_descriptions() {
    let op = ApiOperationContext {
        interface_id: "mcp.items.list".into(),
        method: "GET".into(),
        path: "/api/console/mcp/list".into(),
    };
    let items = (0..70)
        .map(|i| McpListItemSummaryResponse {
            id: Some(format!("item-{i}")),
            item_kind: None,
            path: None,
            name: Some("工具".repeat(200)),
            description_short: Some("完整内容".repeat(100)),
            children_count: None,
            risk_level: None,
        })
        .collect::<Vec<_>>();
    let expected = serde_json::to_value(&items).unwrap();
    assert_eq!(
        McpCatalogOutput::List(items)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}

#[test]
fn progressive_interface_events_distinguish_query_and_detail_path() {
    let mut op = ApiOperationContext {
        interface_id: "mcp.catalog.view".into(),
        method: "GET".into(),
        path: "/api/console/mcp/interface-capabilities/page".into(),
    };
    let input = McpCatalogInput::InterfacesPage(McpInterfaceCapabilitiesPageQuery {
        limit: Some(5),
        bindable_only: Some(false),
        ..Default::default()
    });
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"query":{"limit":5,"bindable_only":false}})
    );
    assert_eq!(input.project_for_managed_hook().unwrap()["0"]["limit"], 5);
    let output = McpCatalogOutput::InterfacesPage(McpInterfaceCapabilityPageResponse {
        items: vec![],
        total: 1,
        offset: 1,
        limit: 5,
        has_more: false,
        next_offset: None,
    });
    assert_eq!(
        output.project_document_output(&op).unwrap(),
        json!({"items":[],"total":1,"offset":1,"limit":5,"has_more":false,"next_offset":null})
    );
    op.path = "/api/console/mcp/interface-capabilities/:interface_id".into();
    let input = McpCatalogInput::Interface("capability-id".into());
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"interface_id":"capability-id"}})
    );
    assert_eq!(
        input.project_for_managed_hook().unwrap()["0"]["interface_id"],
        "capability-id"
    );
}
