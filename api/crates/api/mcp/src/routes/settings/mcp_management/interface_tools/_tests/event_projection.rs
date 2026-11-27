use super::*;
use crate::openapi_interface::contract::ApiOutput;
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

#[test]
fn tool_page_projects_only_its_named_query_and_summary() {
    let op = ApiOperationContext {
        interface_id: "mcp.tools.view".into(),
        method: "GET".into(),
        path: "/api/console/mcp/pages/tools".into(),
    };
    let input = McpToolsInput::ToolsPage(McpToolPageHttpQuery {
        keyword: Some("GET /api/example".into()),
        des_id_required: Some(false),
        ..Default::default()
    });
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"query":{"keyword":"GET /api/example","des_id_required":false}})
    );
    assert_eq!(
        input.project_for_managed_hook().unwrap()["0"]["search_byte_count"],
        16
    );
    let output = McpToolsOutput::ToolsPage(McpToolSummaryPageResponse {
        items: vec![],
        total: 21,
        page: 2,
        page_size: 20,
    });
    assert_eq!(
        output.project_document_output(&op).unwrap(),
        json!({"items":[],"total":21,"page":2,"page_size":20})
    );
    assert_eq!(output.project_for_managed_hook().unwrap()["0"]["total"], 21);
}
