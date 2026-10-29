use super::*;
use crate::routes::assistant::{AssistantModelOption, AssistantRunCapabilities};
use control_plane::notifications::{BuiltinCommunicationIntent, CommunicationCommandIntent};
use std::sync::atomic::{AtomicBool, Ordering};

#[tokio::test]
async fn communication_command_new_joins_blocked_generation_before_durable_confirmation() {
    let fixture = Fixture::new().await;
    let (scope, ordinary) = fixture
        .receive("alice", "ordinary", "start a long answer")
        .await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let session = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    fixture
        .state
        .notification_service
        .select_model(&scope, Some("preserved-model"))
        .await
        .unwrap();
    let legacy_activity = fixture
        .state
        .communication_activity
        .session(scope.conversation_id, 0)
        .unwrap();
    let legacy_lease = legacy_activity.enter().unwrap();
    let (legacy_tx, legacy_rx) = tokio::sync::oneshot::channel();
    let legacy = tokio::spawn(async move {
        legacy_activity.cancelled().await;
        drop(legacy_lease);
        let _ = legacy_tx.send(());
    });
    let activity = fixture
        .state
        .communication_activity
        .session(session.id, session.epoch)
        .unwrap();
    let lease = activity.enter().unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let dropped = Arc::new(AtomicBool::new(false));
    struct Probe(Arc<AtomicBool>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let probe = dropped.clone();
    let task = tokio::spawn(async move {
        {
            let generation = async move {
                let _probe = Probe(probe);
                let _ = started_tx.send(());
                std::future::pending::<()>().await;
            };
            tokio::pin!(generation);
            tokio::select! { biased; _=activity.cancelled()=>{}, _=&mut generation=>panic!("blocked generation returned") }
        }
        let _ = cancelled_tx.send(());
        let _ = release_rx.await;
        drop(lease);
    });
    started_rx.await.unwrap();
    let (_, new) = fixture.receive("alice", "new", "/new").await;
    let (_, next) = fixture
        .receive("alice", "next", "new session question")
        .await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let state = fixture.state.clone();
    let key = fixture.key;
    let (_signal, stop) = watch::channel(false);
    let command =
        tokio::spawn(async move { super::super::effects::process_next(&state, key, stop).await });
    tokio::time::timeout(Duration::from_secs(15), cancelled_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(
        dropped.load(Ordering::SeqCst),
        "provider generation future must be dropped"
    );
    let result = fixture
        .state
        .notification_service
        .command_for_input(&scope, new)
        .await
        .unwrap()
        .unwrap();
    assert!(
        result.reply.is_none(),
        "cancellation signal alone cannot acknowledge /new"
    );
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .pending_admissions(&scope)
        .await
        .unwrap()
        .contains(&next));
    release_tx.send(()).unwrap();
    task.await.unwrap();
    tokio::time::timeout(Duration::from_secs(15), legacy_rx)
        .await
        .unwrap()
        .unwrap();
    legacy.await.unwrap();
    command.await.unwrap().unwrap();
    let result = fixture
        .state
        .notification_service
        .command_for_input(&scope, new)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.reply.unwrap().text,
        "The previous conversation has stopped generating. A new conversation has started."
    );
    let current = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(current.id, session.id);
    assert_eq!(current.selected_model.as_deref(), Some("preserved-model"));
    let turn_state:String=sqlx::query_scalar("SELECT t.state FROM communication_reply_turns t JOIN communication_inputs i ON i.turn_id=t.id WHERE i.id=$1").bind(ordinary).fetch_one(fixture.state.store.pool()).await.unwrap();
    assert_eq!(turn_state, "invalidated");
    assert_eq!(result.response_session_id, Some(current.id));
}

fn capabilities() -> AssistantRunCapabilities {
    AssistantRunCapabilities {
        model_selection_enabled: true,
        reasoning_effort_enabled: false,
        models: vec![AssistantModelOption {
            id: "user/opaque:model-b".into(),
            name: Some("Model B".into()),
            context_window: None,
            reasoning_efforts: vec![],
            default_reasoning_effort: None,
        }],
    }
}

#[tokio::test]
async fn communication_command_new_persists_join_failure_reply_and_releases_admission() {
    use crate::communication_runtime::activity::tests::{provider_request, OwnedProviderFixture};
    let fixture = Fixture::new().await;
    let (scope, _) = fixture.receive("alice", "ordinary", "old answer").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let session = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    let activity = fixture
        .state
        .communication_activity
        .session(session.id, session.epoch)
        .unwrap();
    let run_id = Uuid::now_v7();
    let _registration = fixture
        .state
        .communication_activity
        .register_provider_run(run_id, activity)
        .unwrap();
    let provider = Arc::new(OwnedProviderFixture::new(true));
    let backend =
        orchestration_runtime::runtime_backend::OrchestrationRuntimeBackend::new(provider.clone());
    let registry = fixture.state.communication_activity.clone();
    let execution = tokio::spawn(async move {
        registry
            .execute_provider(run_id, &backend, provider_request(), Default::default())
            .await
    });
    provider.wait_started().await;
    let (_, command_input) = fixture.receive("alice", "new", "/new").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let (_signal, stop) = watch::channel(false);
    super::super::effects::process_next(&fixture.state, fixture.key, stop)
        .await
        .unwrap();
    assert!(execution.await.unwrap().is_err());

    let result = fixture
        .state
        .notification_service
        .command_for_input(&scope, command_input)
        .await
        .unwrap()
        .unwrap();
    let current = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(current.id, session.id);
    assert_eq!(result.response_session_id, Some(current.id));
    assert_eq!(
        result.failure_code.as_deref(),
        Some("communication_provider_cancel_join_failed")
    );
    assert_eq!(result.reply.unwrap().text,
        "The previous conversation could not be confirmed stopped. Replies from it are blocked. A new conversation has been created.");
    assert!(!fixture
        .state
        .notification_service
        .pending_admissions(&scope)
        .await
        .unwrap()
        .contains(&command_input));
    assert!(
        fixture
            .state
            .notification_service
            .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
            .await
            .unwrap()
            .is_none(),
        "the physical join failure must be terminal evidence, not an endless command retry"
    );
}

#[tokio::test]
async fn communication_command_model_allowed_choice_is_atomic_and_current_turn_stays_frozen() {
    let fixture = Fixture::new().await;
    let (scope, ordinary) = fixture.receive("alice", "ordinary", "existing turn").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let (_, input) = fixture
        .receive("alice", "choose", "/model-user-user/opaque:model-b")
        .await;
    let request = fixture
        .state
        .notification_service
        .enqueue_command(
            &scope,
            input,
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Model {
                model: Some("user/opaque:model-b".into()),
            }),
        )
        .await
        .unwrap();
    let stale = fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE communication_command_events SET effect_claim_expires_at=clock_timestamp()-interval '1 second' WHERE event_id=$1").bind(request.event_id).execute(fixture.state.store.pool()).await.unwrap();
    let claim = fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .unwrap();
    assert!(super::super::effects::apply_model(
        &fixture.state,
        &stale,
        &capabilities(),
        Some("user/opaque:model-b"),
        None
    )
    .await
    .is_err());
    assert!(fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap()
        .selected_model
        .is_none());
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());
    sqlx::query("CREATE FUNCTION reject_model_command_reply() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.reply_json IS NOT NULL THEN RAISE EXCEPTION 'fixture model receipt abort'; END IF; RETURN NEW; END $$").execute(fixture.state.store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_model_command_reply BEFORE UPDATE ON communication_command_events FOR EACH ROW EXECUTE FUNCTION reject_model_command_reply()").execute(fixture.state.store.pool()).await.unwrap();
    assert!(super::super::effects::apply_model(
        &fixture.state,
        &claim,
        &capabilities(),
        Some("user/opaque:model-b"),
        None
    )
    .await
    .is_err());
    assert!(
        fixture
            .state
            .notification_service
            .active_session(&scope)
            .await
            .unwrap()
            .unwrap()
            .selected_model
            .is_none(),
        "model selection must roll back with its reply receipt"
    );
    assert!(fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap()
        .reply
        .is_none());
    sqlx::query("DROP TRIGGER reject_model_command_reply ON communication_command_events")
        .execute(fixture.state.store.pool())
        .await
        .unwrap();
    super::super::effects::apply_model(
        &fixture.state,
        &claim,
        &capabilities(),
        Some("user/opaque:model-b"),
        None,
    )
    .await
    .unwrap();
    let session = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        session.selected_model.as_deref(),
        Some("user/opaque:model-b")
    );
    let frozen:Option<String>=sqlx::query_scalar("SELECT t.selected_model FROM communication_reply_turns t JOIN communication_inputs i ON i.turn_id=t.id WHERE i.id=$1").bind(ordinary).fetch_one(fixture.state.store.pool()).await.unwrap();
    assert!(
        frozen.is_none(),
        "current turn retains its selected-model snapshot"
    );
    let result = fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap();
    assert!(result.reply.unwrap().text.contains("Model B"));
    assert!(fixture
        .state
        .notification_service
        .pending_admissions(&scope)
        .await
        .unwrap()
        .is_empty());
    let (bob, _) = fixture.receive("bob", "bob-first", "separate chat").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    assert!(fixture
        .state
        .notification_service
        .active_session(&bob)
        .await
        .unwrap()
        .unwrap()
        .selected_model
        .is_none());
    assert!(fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap()
        .config
        .model
        .is_none());
    let (_, listing) = fixture.receive("alice", "listing", "/model").await;
    fixture
        .state
        .notification_service
        .enqueue_command(
            &scope,
            listing,
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Model { model: None }),
        )
        .await
        .unwrap();
    let list_claim = fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .unwrap();
    super::super::effects::apply_model(
        &fixture.state,
        &list_claim,
        &capabilities(),
        None,
        Some("user/opaque:model-b"),
    )
    .await
    .unwrap();
    let listed = fixture
        .state
        .notification_service
        .command_for_input(&scope, listing)
        .await
        .unwrap()
        .unwrap()
        .reply
        .unwrap()
        .text;
    assert!(listed.starts_with("Current model: `user/opaque:model-b`\n\nAllowed models:\n\n- "));
    assert!(listed.contains("`/model-user-user/opaque:model-b`"));
    assert_eq!(
        listed.lines().filter(|line| line.starts_with("- ")).count(),
        capabilities().models.len()
    );
    let (_, invalid) = fixture
        .receive("alice", "invalid", "/model foreign/model")
        .await;
    fixture
        .state
        .notification_service
        .enqueue_command(
            &scope,
            invalid,
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Model {
                model: Some("foreign/model".into()),
            }),
        )
        .await
        .unwrap();
    let claim = fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .unwrap();
    super::super::effects::apply_model(
        &fixture.state,
        &claim,
        &capabilities(),
        Some("foreign/model"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        fixture
            .state
            .notification_service
            .command_for_input(&scope, invalid)
            .await
            .unwrap()
            .unwrap()
            .failure_code
            .as_deref(),
        Some("model_not_allowed")
    );
    assert_eq!(
        fixture
            .state
            .notification_service
            .active_session(&scope)
            .await
            .unwrap()
            .unwrap()
            .selected_model
            .as_deref(),
        Some("user/opaque:model-b")
    );
}

#[tokio::test]
async fn communication_command_help_uses_effective_availability_and_rechecks_disabled_builtin() {
    let fixture = Fixture::new().await;
    let (scope, help) = fixture.receive("alice", "help", "/help").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    fixture.effect().await;
    let text = fixture
        .state
        .notification_service
        .command_for_input(&scope, help)
        .await
        .unwrap()
        .unwrap()
        .reply
        .unwrap()
        .text;
    assert!(text.contains("/new"));
    assert!(text.contains("/help"));
    assert_eq!(
        text,
        "- `/new` — Start a new AI session\n- `/help` — Show enabled commands"
    );
    assert!(
        !text.contains("/model"),
        "unavailable application cannot advertise model selection"
    );
    let (_, new) = fixture.receive("alice", "disable-race", "/new").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let before = fixture
        .state
        .notification_service
        .active_session(&scope)
        .await
        .unwrap()
        .unwrap();
    let mut binding = fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap();
    binding.config.command_settings = vec![CommunicationCommandSetting {
        command_id: "host/new".into(),
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
    fixture.effect().await;
    assert_eq!(
        fixture
            .state
            .notification_service
            .active_session(&scope)
            .await
            .unwrap()
            .unwrap()
            .id,
        before.id
    );
    assert_eq!(
        fixture
            .state
            .notification_service
            .command_for_input(&scope, new)
            .await
            .unwrap()
            .unwrap()
            .failure_code
            .as_deref(),
        Some("command_disabled")
    );
}
