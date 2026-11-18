use super::*;

fn summary(conversation_id: Uuid) -> AssistantConversationSummaryResponse {
    AssistantConversationSummaryResponse {
        conversation_id: Some(conversation_id),
        legacy_flow_run_id: None,
        latest_flow_run_id: None,
        latest_flow_run_status: None,
        title: None,
        created_at: "2026-08-15T00:00:00Z".to_string(),
        updated_at: "2026-08-15T00:00:00Z".to_string(),
    }
}

#[tokio::test]
async fn conversation_events_are_isolated_by_scope() {
    let hub = AssistantConversationEventHub::default();
    let scope = AssistantConversationEventScope {
        workspace_id: Uuid::now_v7(),
        application_id: Uuid::now_v7(),
        actor_user_id: Uuid::now_v7(),
    };
    let mut matching = hub.subscribe(scope);
    let mut foreign = hub.subscribe(AssistantConversationEventScope {
        actor_user_id: Uuid::now_v7(),
        ..scope
    });
    let conversation_id = Uuid::now_v7();

    hub.publish(
        scope,
        AssistantConversationEventKind::Created,
        summary(conversation_id),
    );

    let event = matching.recv().await.expect("matching event");
    assert_eq!(event.event_type, "conversation.created");
    assert_eq!(event.item.conversation_id, Some(conversation_id));
    assert!(foreign.try_recv().is_err());
}
