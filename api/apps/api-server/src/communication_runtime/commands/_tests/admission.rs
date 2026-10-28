use super::*;

#[tokio::test]
async fn communication_command_unknown_and_disabled_controls_block_then_exclude_without_runs() {
    let fixture = Fixture::new().await;
    let (scope, command) = fixture
        .receive("alice", "unknown", "/missing original text")
        .await;
    let (_, ordinary) = fixture.receive("alice", "next", "ordinary question").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let request = fixture
        .state
        .notification_service
        .command_for_input(&scope, command)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request.request.command_id, "host/rejected");
    assert_eq!(
        fixture
            .state
            .notification_service
            .pending_admissions(&scope)
            .await
            .unwrap(),
        vec![command, ordinary]
    );
    let turns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_reply_turns WHERE conversation_id=$1",
    )
    .bind(scope.conversation_id)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        turns, 0,
        "later ordinary input cannot cross unresolved control"
    );
    fixture.effect().await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let control: (String, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT admission_status,turn_id,run_id FROM communication_inputs WHERE id=$1",
    )
    .bind(command)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(control, ("excluded".into(), None, None));
    let admitted: Option<Uuid> =
        sqlx::query_scalar("SELECT turn_id FROM communication_inputs WHERE id=$1")
            .bind(ordinary)
            .fetch_one(fixture.state.store.pool())
            .await
            .unwrap();
    assert!(admitted.is_some());
    let history: String = sqlx::query_scalar(
        "SELECT content FROM communication_messages WHERE input_id=$1 AND direction='inbound'",
    )
    .bind(command)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(history, "/missing original text");

    let mut binding = fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap();
    binding.config.command_settings = vec![CommunicationCommandSetting {
        command_id: "host/help".into(),
        enabled: false,
    }];
    fixture
        .state
        .notification_service
        .save_binding(
            &fixture.actor,
            fixture.key.1,
            binding.config,
            "fixture-tools".into(),
        )
        .await
        .unwrap();
    let (bob, disabled) = fixture.receive("bob", "disabled", "/help").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    fixture.effect().await;
    let result = fixture
        .state
        .notification_service
        .command_for_input(&bob, disabled)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.failure_code.as_deref(), Some("command_disabled"));
    assert_eq!(
        result.reply.unwrap().text,
        "This command is disabled for this channel."
    );
}

#[tokio::test]
async fn communication_command_admission_does_not_enqueue_controls_after_unfinished_new() {
    let fixture = Fixture::new().await;
    let (scope, new) = fixture.receive("alice", "new", "/new").await;
    let (_, later) = fixture.receive("alice", "later", "/model").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&scope, new)
        .await
        .unwrap()
        .is_some());
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&scope, later)
        .await
        .unwrap()
        .is_none());
    fixture.effect().await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let later = fixture
        .state
        .notification_service
        .command_for_input(&scope, later)
        .await
        .unwrap()
        .unwrap();
    let active = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(later.request.session_id, active.id.to_string());
}
