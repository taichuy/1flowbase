use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "PUT".into(),
        path: "/api/console/mcp/instances/:instance_id/client-credential".into(),
    }
}
#[test]
fn client_key_is_absent_from_event_schema_and_value_but_http_still_reveals_it() {
    let op = operation("mcp.client_credential.save");
    let input = McpCoreInput::SaveCredential(
        "instance-a".into(),
        SaveMcpClientCredentialBody {
            api_key: "SAVED-CLIENT-KEY-SENTINEL".into(),
        },
    );
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"instance_id":"instance-a"},"body":{}})
    );
    assert!(!McpCoreInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("api_key"));
    let response = McpClientCredentialResponse {
        saved: true,
        api_key: Some("REVEALED-CLIENT-KEY-SENTINEL".into()),
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap()["api_key"],
        "REVEALED-CLIENT-KEY-SENTINEL"
    );
    let op = operation("mcp.client_credential.reveal");
    assert_eq!(
        McpCoreOutput::Credential(response)
            .project_document_output(&op)
            .unwrap(),
        json!({"saved":true})
    );
    assert!(!McpCoreOutput::document_output_schema(&op)
        .unwrap()
        .to_string()
        .contains("api_key"));
}
#[test]
fn no_body_operation_has_only_its_documented_location() {
    let op = operation("mcp.instances.delete");
    assert_eq!(
        McpCoreInput::DeleteInstance("instance-a".into())
            .project_document_input(&op)
            .unwrap(),
        json!({"path":{"instance_id":"instance-a"}})
    );
    assert_eq!(
        McpCoreInput::ListInstances
            .project_document_input(&op)
            .unwrap(),
        json!({})
    );
    assert_eq!(
        McpCoreOutput::NoContent
            .project_document_output(&op)
            .unwrap(),
        Value::Null
    );
}
