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
