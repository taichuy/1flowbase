use super::*;
#[test]
fn forwarded_headers_and_downstream_credentials_stay_host_owned() {
    let op = ApiOperationContext {
        interface_id: "mcp.debug.execute".into(),
        method: "POST".into(),
        path: "/api/console/mcp/debug/execute".into(),
    };
    let input = McpDebugInput {
        body: McpDebugExecuteBody {
            interface_id: "credential.reveal".into(),
            debug_response_mode: debug_execute::McpDebugResponseMode::DebugDetails,
            mcp_arguments: json!({"password":"ARGUMENT-SENTINEL"}),
            input_mapping: json!({"secret":"MAPPING-SENTINEL"}),
            output_mapping: json!({}),
        },
        forwarding: CallableDispatchForwarding {
            cookie: Some(b"COOKIE-SENTINEL".to_vec()),
            authorization: Some(b"BEARER-SENTINEL".to_vec()),
            csrf_token: Some(b"CSRF-SENTINEL".to_vec()),
            ..Default::default()
        },
    };
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(
        value,
        json!({"body":{"interface_id":"credential.reveal","debug_response_mode":"debug_details"}})
    );
    assert!(!value.to_string().contains("SENTINEL"));
    let schema = McpDebugInput::document_input_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("forwarding"));
    assert!(!schema.contains("mcp_arguments"));
    assert_eq!(
        McpDebugOutput::document_output_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::CredentialBoundary)
    );
    assert!(McpDebugOutput::Json(json!({"api_key":"OUTPUT-SENTINEL"}))
        .project_document_output(&op)
        .is_err());
}
