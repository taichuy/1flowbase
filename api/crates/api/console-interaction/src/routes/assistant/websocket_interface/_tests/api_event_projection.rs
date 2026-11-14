use super::*;
#[test]
fn post_upgrade_commands_are_not_projected_as_http_handshake_payloads() {
    let op = ApiOperationContext {
        interface_id: "assistant.runs.websocket.command".into(),
        method: "GET".into(),
        path: "/api/console/assistant/runs/websocket".into(),
    };
    assert_eq!(
        AssistantWebSocketCommandInput::document_input_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::ProtocolControl)
    );
    assert_eq!(
        AssistantWebSocketCommandOutput::document_output_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::ProtocolControl)
    );
    assert!(AssistantWebSocketCommandInput::Cancel {
        application_id: Uuid::nil(),
        run_id: Uuid::nil()
    }
    .project_document_input(&op)
    .is_err());
    assert!(AssistantWebSocketCommandOutput::Cancelled
        .project_document_output(&op)
        .is_err());
}
