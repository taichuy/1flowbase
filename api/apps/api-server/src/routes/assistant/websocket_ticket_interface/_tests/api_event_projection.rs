use super::*;
#[test]
fn websocket_ticket_is_not_an_event_field_and_http_serialization_is_unchanged() {
    let op = ApiEventOperationContext {
        interface_id: "assistant.runs.websocket-ticket.create".into(),
        method: "POST".into(),
        path: "/api/console/assistant/runs/websocket-ticket".into(),
    };
    let response = AssistantWebSocketTicketResponse {
        ticket: "TICKET-SECRET-SENTINEL".into(),
        protocol: "1flowbase.assistant.v1",
        expires_in_seconds: 60,
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap()["ticket"],
        "TICKET-SECRET-SENTINEL"
    );
    assert_eq!(
        AssistantWebSocketTicketOutput::Ticket(response)
            .project_document_output(&op)
            .unwrap(),
        json!({"protocol":"1flowbase.assistant.v1","expires_in_seconds":60})
    );
    assert!(!AssistantWebSocketTicketOutput::document_output_schema(&op)
        .unwrap()
        .to_string()
        .contains("\"ticket\""));
    assert_eq!(
        AssistantWebSocketTicketInput::Create {
            body: CreateAssistantWebSocketTicketBody {
                application_id: Uuid::nil()
            },
            origin: Some("ORIGIN-HEADER-SENTINEL".into())
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"body":{"application_id":Uuid::nil()}})
    );
}
