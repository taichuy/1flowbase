use super::*;
use crate::routes::webmcp::{WebMcpToolAnnotationsResponse, WebMcpToolResponse};
#[test]
fn webmcp_outer_metadata_excludes_opaque_downstream_credentials_without_changing_typed_carriers() {
    let op = ApiEventOperationContext {
        interface_id: "webmcp.tools.invoke".into(),
        method: "POST".into(),
        path: "/api/webmcp/:instance_id/tools/:operation".into(),
    };
    let input = WebMcpInput::Tool {
        instance_id: "instance-a".into(),
        operation: "invoke".into(),
        arguments: json!({"password":"ARGUMENT-PASSWORD-SENTINEL"}),
        forward_headers: vec![WebMcpForwardHeader {
            name: "authorization".into(),
            value: b"Bearer HEADER-SENTINEL".to_vec(),
        }],
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"instance_id":"instance-a","operation":"invoke"}})
    );
    let schema = WebMcpInput::document_input_schema(&op).unwrap().to_string();
    assert!(!schema.contains("arguments"));
    assert!(!schema.contains("headers"));
    let output = WebMcpOutput::Tool(InvokeWebMcpToolResponse {
        content: json!({"api_key":"DOWNSTREAM-KEY-SENTINEL"}),
        is_error: false,
    });
    assert_eq!(
        output.project_document_output(&op).unwrap(),
        json!({"is_error":false})
    );
    assert!(!WebMcpOutput::document_output_schema(&op)
        .unwrap()
        .to_string()
        .contains("content"));
    let WebMcpOutput::Tool(response) = output else {
        unreachable!()
    };
    assert_eq!(response.content["api_key"], "DOWNSTREAM-KEY-SENTINEL");
}
#[test]
fn public_registration_catalog_is_complete_with_long_descriptions_and_many_tools() {
    let op = ApiEventOperationContext {
        interface_id: "webmcp.registrations.list".into(),
        method: "GET".into(),
        path: "/api/webmcp/registrations".into(),
    };
    let registrations = vec![WebMcpRegistrationResponse { instance_id:"instance-a".into(),tools:(0..80).map(|i|WebMcpToolResponse { operation:format!("tool-{i}"),name:format!("Tool {i}"),title:"Tool".repeat(100),description:"Complete description".repeat(200),input_schema:json!({"type":"object","business_metadata":{"secret_label":"Public title"}}),annotations:WebMcpToolAnnotationsResponse { read_only_hint:true,untrusted_content_hint:true } }).collect() }];
    let expected = serde_json::to_value(&registrations).unwrap();
    assert_eq!(
        WebMcpOutput::Registrations(registrations)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    assert_eq!(
        WebMcpInput::Registrations
            .project_document_input(&op)
            .unwrap(),
        json!({})
    );
}
