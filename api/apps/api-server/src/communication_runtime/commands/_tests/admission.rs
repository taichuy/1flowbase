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

#[tokio::test]
async fn communication_command_blocked_chat_backlog_does_not_hide_another_chat() {
    let fixture = Fixture::new().await;
    let (alice, command) = fixture.receive("alice", "unresolved", "/help").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&alice, command)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());

    // Two bounded receive commits put more than one scan window behind the unresolved control.
    // All these receipts precede Bob's receipt in durable receive order.
    for range in [0..80, 80..105] {
        let revision = fixture
            .state
            .store
            .resolve_notification_authorization(
                fixture.key.0,
                fixture.key.1,
                &fixture.state.provider_secret_master_key,
            )
            .await
            .unwrap()
            .revision;
        fixture
            .state
            .notification_service
            .commit_receive(CommitCommunicationReceiveInput {
                authorization: SaveNotificationAuthorizationInput {
                    workspace_id: fixture.key.0,
                    instance_id: fixture.key.1,
                    expected_revision: revision,
                    authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
                    managed_secrets: Default::default(),
                    master_key: fixture.state.provider_secret_master_key.clone(),
                },
                candidate_cursor: None,
                messages: range
                    .map(|index| CommunicationReceiveMessage {
                        provider_message_id: format!("backlog-{index}"),
                        conversation_id: "external-chat-alice".into(),
                        sender_id: "alice".into(),
                        sender_display_name: None,
                        received_at_ms: 42,
                        text: Some(format!("later ordinary input {index}")),
                        attachments: vec![],
                    })
                    .collect(),
            })
            .await
            .unwrap()
            .unwrap();
    }
    let (bob, bob_input) = fixture
        .receive("bob", "bob-ready", "independent question")
        .await;
    let queued = fixture
        .state
        .notification_service
        .pending_admissions(&alice)
        .await
        .unwrap();
    assert_eq!(queued.len(), 106);
    assert_eq!(queued.first(), Some(&command));
    assert!(
        queued.iter().all(|id| *id < bob_input),
        "fixture must distinguish a global first-100 scan"
    );

    let scanned = fixture
        .state
        .notification_service
        .scan_pending_admissions(fixture.key.0, fixture.key.1, 100)
        .await
        .unwrap();
    assert_eq!(
        scanned
            .iter()
            .map(|input| input.input_id)
            .collect::<Vec<_>>(),
        vec![bob_input]
    );
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .pending_admissions(&bob)
        .await
        .unwrap()
        .is_empty());
    let bob_turn: Option<Uuid> =
        sqlx::query_scalar("SELECT turn_id FROM communication_inputs WHERE id=$1")
            .bind(bob_input)
            .fetch_one(fixture.state.store.pool())
            .await
            .unwrap();
    assert!(
        bob_turn.is_some(),
        "another chat must reach the ordinary turn lane"
    );

    // Subsequent scans and a direct admission attempt cannot skip Alice's command barrier.
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .admit_input(&alice, queued[1])
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        fixture
            .state
            .notification_service
            .pending_admissions(&alice)
            .await
            .unwrap(),
        queued
    );
    let untouched: i64 = sqlx::query_scalar("SELECT count(*) FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND conversation_id=$3 AND admission_status='pending' AND turn_id IS NULL AND run_id IS NULL")
        .bind(alice.workspace_id).bind(alice.instance_id).bind(alice.conversation_id).fetch_one(fixture.state.store.pool()).await.unwrap();
    assert_eq!(untouched, 106);
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&alice, command)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());
}

#[tokio::test]
async fn communication_command_legacy_dispatcher_cannot_claim_new_admission_receipts() {
    let fixture = Fixture::new().await;
    let (ordinary_scope, ordinary) = fixture
        .receive("alice", "new-ordinary", "new question")
        .await;
    let (command_scope, command) = fixture.receive("bob", "new-command", "/new").await;
    assert!(fixture
        .state
        .notification_service
        .claim_pending_legacy_input(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .is_none());
    for input_id in [ordinary, command] {
        let state: (String, String, Option<Uuid>, Option<Uuid>) = sqlx::query_as("SELECT admission_status,processing_status,claim_id,run_id FROM communication_inputs WHERE id=$1")
            .bind(input_id).fetch_one(fixture.state.store.pool()).await.unwrap();
        assert_eq!(state, ("pending".into(), "pending".into(), None, None));
    }

    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .pending_admissions(&ordinary_scope)
        .await
        .unwrap()
        .is_empty());
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&command_scope, command)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());
    assert!(
        fixture
            .state
            .notification_service
            .claim_pending_legacy_input(fixture.key.0, fixture.key.1, 60)
            .await
            .unwrap()
            .is_none(),
        "persisting a command request must not expose it to the legacy dispatcher"
    );

    // Reproduce an upgrade-era observation receipt: its run was already reserved before
    // session admission existed. Reclaim its expired observer without replacing that run.
    let (_, legacy) = fixture
        .receive("legacy", "old-linked-run", "pre-upgrade question")
        .await;
    let original_run = Uuid::now_v7();
    let expired_claim = Uuid::now_v7();
    sqlx::query("UPDATE communication_inputs SET admission_status='legacy',processing_status='processing',run_id=$2,claim_id=$3,claim_expires_at=clock_timestamp()-interval '1 second' WHERE id=$1")
        .bind(legacy).bind(original_run).bind(expired_claim).execute(fixture.state.store.pool()).await.unwrap();
    let reclaimed = fixture
        .state
        .notification_service
        .claim_pending_legacy_input(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .expect("legacy observation remains supported");
    assert_eq!(reclaimed.message.id, legacy);
    assert_eq!(reclaimed.message.run_id, Some(original_run));
    assert_ne!(reclaimed.claim_id, expired_claim);
    assert!(fixture
        .state
        .notification_service
        .claim_pending_legacy_input(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        fixture
            .state
            .notification_service
            .pending_admissions(&command_scope)
            .await
            .unwrap(),
        vec![command]
    );
    let command_state: (String, Option<Uuid>, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT admission_status,turn_id,run_id,claim_id FROM communication_inputs WHERE id=$1",
    )
    .bind(command)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(command_state, ("pending".into(), None, None, None));
}
