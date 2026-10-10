use super::*;
use domain::{ApplicationRunConversationContextItem, ApplicationRunConversationMessageItem};

fn item(source: &str, answer: &str) -> ApplicationRunConversationMessageItem {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    ApplicationRunConversationMessageItem {
        id: Uuid::nil(),
        scope_id: Uuid::nil(),
        application_id: Uuid::nil(),
        flow_run_id: Uuid::nil(),
        display_sequence: 0,
        source_kind: "business_turn".into(),
        role: None,
        content: None,
        query: Some("Current question".into()),
        model: None,
        answer: Some(answer.into()),
        detail_run_id: Some(Uuid::nil()),
        can_open_detail: true,
        is_current: true,
        status: "waiting_callback".into(),
        started_at: now,
        finished_at: None,
        projection_version: 7,
        created_at: now,
        updated_at: now,
        output_source: Some(source.into()),
    }
}

#[test]
fn native_overview_preserves_real_completed_turn_and_unique_message_positions() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../application_run_logs/_tests/fixtures/completed_codex_turn.json"
    ))
    .unwrap();
    let final_output = fixture["final_output"].as_str().unwrap();
    let mut turn = item("provider_output_item", final_output);
    turn.query = Some(fixture["user_input"].as_str().unwrap().into());
    let contexts = (0..2)
        .map(|_| ApplicationRunConversationContextItem {
            id: Uuid::now_v7(),
            flow_run_id: Uuid::now_v7(),
            role: "system".into(),
            context_source: "instructions".into(),
            content: "System context".into(),
            display_sequence: 0,
        })
        .collect();
    let (messages, placeholder) = native_overview_messages(contexts, vec![turn]);
    assert!(placeholder.is_none());
    assert_eq!(
        messages.iter().map(|m| m.role.as_str()).collect::<Vec<_>>(),
        ["system", "system", "user", "assistant"]
    );
    assert_eq!(messages[2].content, fixture["user_input"].as_str().unwrap());
    assert_eq!(messages[3].content, final_output);
    assert_eq!(
        messages.iter().map(|m| m.sequence).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
}

#[test]
fn native_overview_distinguishes_timeout_placeholder_from_literal_assistant_reply() {
    for source in ["projection_timeout", "waiting_callback"] {
        let (messages, placeholder) =
            native_overview_messages(vec![], vec![item(source, "Timeout")]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(placeholder.as_deref(), Some("Timeout"));
    }
    for source in ["persisted_answer", "provider_output_item"] {
        let (messages, placeholder) =
            native_overview_messages(vec![], vec![item(source, "Timeout")]);
        assert_eq!(messages.last().unwrap().role, "assistant");
        assert_eq!(messages.last().unwrap().content, "Timeout");
        assert!(placeholder.is_none());
    }
}
