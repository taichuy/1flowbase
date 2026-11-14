use super::*;
#[test]
fn tool_event_keeps_schemas_mappings_enum_wire_shape_and_long_text() {
    let op = ApiOperationContext {
        interface_id: "mcp.tools.create".into(),
        method: "POST".into(),
        path: "/api/console/mcp/tools".into(),
    };
    let text = "完整工具说明".repeat(400);
    let schema = json!({"type":"object","properties":{"rows":{"type":"array","items":{"type":"integer"}}},"additionalProperties":false});
    let mapping = json!({"rows":(0..100).collect::<Vec<_>>()});
    let body = CreateMcpToolBody {
        tool_id: "lookup".into(),
        des_id: Some("description-a".into()),
        name: "Lookup".into(),
        short_description: "Find records".into(),
        full_description: Some(text.clone()),
        execution_target: McpToolExecutionTargetDto::InterfaceWrapper {
            interface_id: "records.list".into(),
        },
        parameter_schema: schema.clone(),
        result_schema: json!({"type":"array"}),
        input_mapping: mapping.clone(),
        output_mapping: json!({"result":"$.rows"}),
        max_inline_chars: Some(10000),
        response_fields: Some(vec!["rows".into()]),
        permission_code: None,
        risk_level: "read".into(),
        status: "enabled".into(),
    };
    let value = McpToolsInput::Create(body)
        .project_document_input(&op)
        .unwrap();
    assert_eq!(value["body"]["parameter_schema"], schema);
    assert_eq!(value["body"]["input_mapping"], mapping);
    assert_eq!(value["body"]["full_description"], text);
    assert_eq!(
        value["body"]["execution_target"],
        json!({"kind":"interface_wrapper","interface_id":"records.list"})
    );
    assert!(value.get("path").is_none());
    assert!(value.get("query").is_none());
    let op = ApiOperationContext {
        interface_id: "mcp.tools.get".into(),
        method: "GET".into(),
        path: "/api/console/mcp/tools/:tool_id".into(),
    };
    assert_eq!(
        McpToolsInput::Get("lookup".into())
            .project_document_input(&op)
            .unwrap(),
        json!({"path":{"tool_id":"lookup"}})
    );
}
