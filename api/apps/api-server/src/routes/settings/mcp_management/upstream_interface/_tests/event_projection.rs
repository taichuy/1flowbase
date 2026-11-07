use super::*;
fn operation(id: &str, path: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: path.into(),
    }
}
#[test]
fn upstream_credentials_and_url_components_are_omitted_from_event_views() {
    let op = operation(
        "mcp.upstream_credentials.update",
        "/api/console/mcp/upstream-connections/:connection_id/credentials",
    );
    for body in [
        SaveMcpUpstreamCredentialBody::Bearer {
            token: "BEARER-SENTINEL".into(),
        },
        SaveMcpUpstreamCredentialBody::CustomHeader {
            header_name: "x-api-key".into(),
            header_value: "HEADER-VALUE-SENTINEL".into(),
        },
    ] {
        let value = McpUpstreamInput::SaveCredentials {
            connection_id: "upstream-a".into(),
            body,
        }
        .project_document_input(&op)
        .unwrap();
        assert_eq!(value["path"]["connection_id"], "upstream-a");
        assert!(!value.to_string().contains("SENTINEL"));
        assert!(value["body"].get("token").is_none());
        assert!(value["body"].get("header_value").is_none());
    }
    let schema = McpUpstreamInput::document_input_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("\"token\""));
    assert!(!schema.contains("header_value"));
    assert_eq!(endpoint_value("https://USER-SENTINEL:PASS-SENTINEL@upstream.test/mcp?api_key=QUERY-SENTINEL#FRAGMENT-SENTINEL").unwrap(), "https://upstream.test/mcp");
}
#[test]
fn draft_and_saved_test_share_interface_but_have_different_real_schemas() {
    let draft = operation(
        "mcp.upstream_connections.test",
        "/api/console/mcp/upstream-connections/test",
    );
    let saved = operation(
        "mcp.upstream_connections.test",
        "/api/console/mcp/upstream-connections/:connection_id/test",
    );
    let body = TestMcpUpstreamConnectionDraftBody {
        connection_id: None,
        endpoint: "https://upstream.test/mcp?token=QUERY-SENTINEL".into(),
        transport: "streamable_http".into(),
        auth_type: "bearer".into(),
        custom_header_name: None,
        credential: Some(SaveMcpUpstreamCredentialBody::Bearer {
            token: "DRAFT-SENTINEL".into(),
        }),
    };
    let value = McpUpstreamInput::TestDraft(body)
        .project_document_input(&draft)
        .unwrap();
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(value.get("path").is_none());
    assert_eq!(value["body"]["endpoint"], "https://upstream.test/mcp");
    assert!(McpUpstreamInput::document_input_schema(&draft).is_some());
    assert!(McpUpstreamInput::document_input_schema(&saved).is_none());
    assert!(!McpUpstreamInput::document_input_schema(&draft)
        .unwrap()
        .to_string()
        .contains("\"token\""));
    assert!(!McpUpstreamOutput::document_output_schema(&draft)
        .unwrap()
        .to_string()
        .contains("connection_id"));
    assert!(McpUpstreamOutput::document_output_schema(&saved)
        .unwrap()
        .to_string()
        .contains("connection_id"));
}
#[test]
fn upstream_diagnostics_cannot_echo_credential_values_into_events() {
    let op = operation(
        "mcp.upstream_connections.test",
        "/api/console/mcp/upstream-connections/:connection_id/test",
    );
    let output = McpUpstreamOutput::Test(McpUpstreamTestResponse {
        connection_id: "upstream-a".into(),
        ok: false,
        server_name: None,
        server_version: None,
        protocol_version: None,
        tested_at: "2026-11-07".into(),
        error: Some("connection failed for https://upstream.test?token=ERROR-SENTINEL".into()),
    });
    let value = output.project_document_output(&op).unwrap();
    assert_eq!(value["connection_id"], "upstream-a");
    assert_eq!(value["ok"], false);
    assert!(value.get("error").is_none());
    assert!(!McpUpstreamOutput::document_output_schema(&op)
        .unwrap()
        .to_string()
        .contains("\"error\""));
}
#[test]
fn ordinary_proxy_result_preserves_complete_json_and_all_array_items() {
    let result = json!({"rows":(0..100).collect::<Vec<_>>(),"text":"文".repeat(600)});
    let op = operation(
        "mcp.upstream_tools.debug",
        "/api/console/mcp/tools/:tool_id/debug",
    );
    let output = McpUpstreamOutput::Debug(DebugMcpProxyToolResponse {
        local_arguments: json!({"query":"records"}),
        remote_arguments: json!({"query":"records"}),
        upstream_result: result.clone(),
        mapped_result: result.clone(),
    });
    let value = output.project_document_output(&op).unwrap();
    assert_eq!(value["upstream_result"], result);
    assert_eq!(
        value["mapped_result"]["rows"].as_array().unwrap().len(),
        100
    );
}
