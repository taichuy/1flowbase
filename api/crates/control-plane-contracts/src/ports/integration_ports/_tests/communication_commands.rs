use super::*;

#[test]
fn communication_command_delivery_uses_host_scope_and_stable_input_causation() {
    let scope = CommunicationConversationScope {
        workspace_id: Uuid::now_v7(),
        instance_id: Uuid::now_v7(),
        external_identity: "alice".into(),
        conversation_id: Uuid::now_v7(),
    };
    let input = Uuid::now_v7();
    let mut request = CommunicationCommandRequest {
        command_id: "acme/bot/status".into(),
        arguments: "today".into(),
        channel_id: scope.instance_id.to_string(),
        conversation_id: scope.conversation_id.to_string(),
        session_id: Uuid::now_v7().to_string(),
        input_id: input.to_string(),
    };
    let delivery = communication_command_delivery(input, &scope, &request).unwrap();
    assert_eq!(delivery.workspace_id, scope.workspace_id.to_string());
    assert_eq!(delivery.causation_id, input.to_string());
    assert_eq!(delivery.correlation_id, input.to_string());
    assert_eq!(
        delivery.schema.payload_schema,
        CommunicationCommandRequest::event_schema()
    );
    request.channel_id = Uuid::now_v7().to_string();
    assert!(communication_command_delivery(input, &scope, &request).is_err());
}
