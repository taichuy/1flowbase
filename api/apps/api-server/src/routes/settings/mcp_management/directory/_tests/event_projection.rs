use super::*;
#[test]
fn directory_event_matches_real_http_snapshot_without_summary_limits() {
    let op = ApiEventOperationContext {
        interface_id: "mcp.directory.read".into(),
        method: "GET".into(),
        path: "/api/console/mcp/instances/:instance_id/directory".into(),
    };
    let groups = (0..60)
        .map(|i| McpGroupResponse {
            id: format!("group-{i}"),
            instance_record_id: "instance-a".into(),
            path: format!("group-{i}"),
            display_name: "工具组".repeat(100),
            description_short: None,
            enabled: true,
            sort_order: i,
        })
        .collect();
    let snapshot = McpInstanceDirectoryResponse {
        instance_record_id: "instance-a".into(),
        groups,
        bindings: vec![],
    };
    let expected = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        McpDirectoryOutput(snapshot)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    assert_eq!(
        McpDirectoryInput::Read("instance-a".into())
            .project_document_input(&op)
            .unwrap(),
        json!({"path":{"instance_id":"instance-a"}})
    );
}
