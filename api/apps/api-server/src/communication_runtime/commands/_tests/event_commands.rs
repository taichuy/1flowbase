use super::*;
use control_plane::notifications::CommunicationCommandIntent;
use domain::event_center::EventDefinitionRecord;

async fn configure(fixture: &mut Fixture) -> EventDefinitionRecord {
    // Explicit test deployment budget; production capacity remains deployment-owned.
    Arc::get_mut(&mut fixture.state)
        .expect("fixture state is uniquely owned")
        .event_center = control_plane::event_center::EventCenterRuntime::new(
        Arc::new(storage_ephemeral::MemoryEventBus::new()),
        Some(control_plane::event_center::EventCenterRuntimePolicy {
            bus_limits: EventBusLimits {
                max_items: 8,
                max_json_bytes: 64_000,
                max_waiters: 8,
                max_topics: 8,
                max_consumers: 8,
                max_topic_name_bytes: 8_000,
            },
            observation_max_bytes: 16_000,
            observation_ttl_ms: 60_000,
        }),
    )
    .unwrap();
    // Historical communication commands still publish their stored custom contract.
    // Seed that record directly: new interface rules cannot define a free-form schema.
    let now = time::OffsetDateTime::now_utc();
    let definition = EventDefinitionRecord {
        event_kind: None,
        interface_event: None,
        id: Uuid::now_v7(),
        workspace_id: fixture.actor.current_workspace_id,
        contract_id: "custom.command-test".into(),
        name: "Command test".into(),
        description: String::new(),
        enabled: true,
        payload_schema: json!({"type":"object","required":["message"],"additionalProperties":false,"properties":{"message":{"type":"string","minLength":1}}}),
        created_by: fixture.actor.user_id,
        created_at: now,
        updated_at: now,
    };
    fixture
        .state
        .store
        .create_event_definition(&definition)
        .await
        .unwrap();
    let mut binding = fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap();
    binding
        .config
        .commands
        .push(domain::CommunicationCommandConfiguration::Event(
            CommunicationEventCommandBinding {
                command_id: "user/notify".into(),
                name: "notify".into(),
                description: "Publish event".into(),
                enabled: true,
                contract_id: definition.contract_id.clone(),
                input_mapping: std::collections::BTreeMap::from([(
                    "message".into(),
                    "/arguments".into(),
                )]),
            },
        ));
    fixture
        .state
        .notification_service
        .save_binding(
            &fixture.actor,
            fixture.key.1,
            binding.config,
            binding.plugin_mcp_instance_id,
        )
        .await
        .unwrap();
    definition
}

#[tokio::test]
async fn event_command_catalog_help_classification_and_invalid_payload_never_fall_back_to_chat() {
    let mut fixture = Fixture::new().await;
    configure(&mut fixture).await;
    let binding = fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap();
    let catalog = fixture
        .state
        .notification_service
        .event_command_catalog(&fixture.actor, &[], &binding.config.event_commands())
        .await
        .unwrap();
    assert!(catalog.iter().any(|c| c.command_id == "user/notify"
        && c.source_kind == CommunicationCommandSourceKind::User
        && c.available));
    assert!(
        control_plane::notifications::communication_command_help(&catalog)
            .unwrap()
            .text
            .contains("/notify")
    );
    let intent = fixture
        .state
        .notification_service
        .resolve_event_command(
            &fixture.actor,
            &[],
            &binding.config.event_commands(),
            "/notify hi",
        )
        .await
        .unwrap();
    assert!(
        matches!(intent, CommunicationCommandIntent::Event { command_id, arguments } if command_id=="user/notify" && arguments=="hi")
    );
    let mut disabled = binding.config.event_commands();
    disabled[0].enabled = false;
    assert!(matches!(
        fixture
            .state
            .notification_service
            .resolve_event_command(&fixture.actor, &[], &disabled, "/notify hi")
            .await
            .unwrap(),
        CommunicationCommandIntent::Rejected { .. }
    ));
    let (scope, input) = fixture.receive("alice", "empty-event", "/notify").await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    fixture.effect().await;
    let record = fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        record.failure_code.as_deref(),
        Some("event_command_publish_rejected")
    );
    assert!(record.reply.is_some());
    assert!(!fixture
        .state
        .notification_service
        .pending_admissions(&scope)
        .await
        .unwrap()
        .contains(&input));
}

#[tokio::test]
async fn event_command_publish_records_zero_targets_and_does_not_reexecute() {
    let mut fixture = Fixture::new().await;
    configure(&mut fixture).await;
    let (scope, input) = fixture
        .receive("alice", "valid-event", "/notify hello")
        .await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    fixture.effect().await;
    let record = fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap();
    let text = record.reply.as_ref().unwrap().text.as_str();
    assert_eq!(
        record.failure_code.as_deref(),
        Some("event_publication_rejected")
    );
    assert!(text.contains("No workflow is available"));
    let activity_count = fixture
        .state
        .event_center
        .snapshot(fixture.key.0)
        .activity
        .len();
    fixture.effect().await;
    let again = fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.reply.unwrap().text, text);
    assert_eq!(
        fixture
            .state
            .event_center
            .snapshot(fixture.key.0)
            .activity
            .len(),
        activity_count
    );
}

#[tokio::test]
async fn reclaimed_event_claim_is_silent_and_terminal_without_republishing() {
    let mut fixture = Fixture::new().await;
    let target = configure(&mut fixture).await;
    let (scope, input) = fixture
        .receive("alice", "uncertain-event", "/notify hello")
        .await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    let first = fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.record.failure_code, None);
    sqlx::query("UPDATE communication_command_events SET effect_claim_expires_at=clock_timestamp()-interval '1 second' WHERE event_id=$1")
        .bind(input).execute(fixture.state.store.pool()).await.unwrap();
    fixture.effect().await;
    let result = fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.failure_code.as_deref(),
        Some("command_outcome_unknown")
    );
    assert!(result.reply.is_none());
    assert_eq!(serde_json::to_value(result.send_status).unwrap(), "none");
    assert!(fixture
        .state
        .notification_service
        .pending_command_responses(fixture.key.0, fixture.key.1, 10)
        .await
        .unwrap()
        .is_empty());
    let outbound: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_messages WHERE instance_id=$1 AND direction='outbound'",
    )
    .bind(fixture.key.1)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        outbound, 0,
        "internal recovery must not create a chat message"
    );
    fixture.effect().await;
    assert!(fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .is_none());
    let status: (String, String, Option<String>) = sqlx::query_as(
        "SELECT processing_status,send_status,failure_code FROM communication_inputs WHERE id=$1",
    )
    .bind(input)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(
        status,
        (
            "failed".into(),
            "none".into(),
            Some("command_outcome_unknown".into())
        )
    );
    assert!(fixture
        .state
        .event_center
        .snapshot(fixture.key.0)
        .activity
        .is_empty());

    // A failed command must release this chat. The next input owns a new event;
    // it must not be merged with the failure or inherit its outcome.
    let consumer = fixture
        .state
        .event_center
        .subscribe(
            control_plane::event_center::EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: fixture.key.0,
                contract_id: target.contract_id,
                trigger_phase: domain::event_center::EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            false,
        )
        .unwrap();
    let (_, next_input) = fixture
        .receive("alice", "after-uncertain-event", "/notify next command")
        .await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    fixture.effect().await;
    let next = fixture
        .state
        .notification_service
        .command_for_input(&scope, next_input)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(next.event_id, input);
    assert_eq!(next.failure_code, None);
    assert!(next.response_session_id.is_some());
    assert!(next.reply.is_none());
    assert!(fixture
        .state
        .notification_service
        .pending_admissions(&scope)
        .await
        .unwrap()
        .is_empty());
    let delivery = tokio::time::timeout(Duration::from_secs(1), consumer.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        delivery.signal().body,
        control_plane::event_center::EventCenterSignalBody::Published {
            payload: json!({"message":"next command"})
        }
    );
    delivery.complete();
}

#[tokio::test]
async fn admitted_event_command_delivers_the_mapped_payload_to_the_explicit_event_subscription() {
    let mut fixture = Fixture::new().await;
    let target = configure(&mut fixture).await;
    let consumer = fixture
        .state
        .event_center
        .subscribe(
            control_plane::event_center::EventRuntimeSubscription {
                subscription_id: Uuid::now_v7(),
                workspace_id: fixture.key.0,
                contract_id: target.contract_id.clone(),
                trigger_phase: domain::event_center::EventTriggerPhase::Published,
                consumer_id: Uuid::now_v7(),
            },
            false,
        )
        .unwrap();
    let (scope, input) = fixture
        .receive("alice", "delivery-event", "/notify actual event")
        .await;
    super::super::admission::admit_available(&fixture.state, fixture.key)
        .await
        .unwrap();
    fixture.effect().await;
    let delivery = tokio::time::timeout(Duration::from_secs(1), consumer.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(delivery.signal().workspace_id, fixture.key.0);
    assert_eq!(
        delivery.signal().body,
        control_plane::event_center::EventCenterSignalBody::Published {
            payload: json!({"message":"actual event"})
        }
    );
    let result = fixture
        .state
        .notification_service
        .command_for_input(&scope, input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.failure_code, None);
    assert!(result.reply.is_none());
    assert!(fixture
        .state
        .notification_service
        .pending_command_responses(fixture.key.0, fixture.key.1, 10)
        .await
        .unwrap()
        .is_empty());
    let outbound: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_messages WHERE instance_id=$1 AND direction='outbound'",
    )
    .bind(fixture.key.1)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(outbound, 0);
    assert!(fixture
        .state
        .notification_service
        .claim_builtin_command(fixture.key.0, fixture.key.1, 60)
        .await
        .unwrap()
        .is_none());
    let status: (String, String) = sqlx::query_as(
        "SELECT processing_status,send_status FROM communication_inputs WHERE id=$1",
    )
    .bind(input)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    assert_eq!(status, ("succeeded".into(), "none".into()));
    delivery.complete();
}

#[tokio::test]
async fn saving_event_commands_rejects_bad_mapping_namespace_names_and_unavailable_targets() {
    let mut fixture = Fixture::new().await;
    configure(&mut fixture).await;
    let binding = fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap();
    for change in 0..6 {
        let mut input = binding.config.clone();
        let event = input
            .commands
            .iter_mut()
            .find_map(|command| match command {
                domain::CommunicationCommandConfiguration::Event(event) => Some(event),
                _ => None,
            })
            .unwrap();
        match change {
            0 => {
                event.input_mapping.clear();
            }
            1 => {
                event
                    .input_mapping
                    .insert("message".into(), "/workspace_id".into());
            }
            2 => {
                event.command_id = "host/notify".into();
            }
            3 => {
                event.name = "help".into();
            }
            4 => {
                event.contract_id = "custom.missing".into();
            }
            _ => {
                input.commands.push(input.commands[0].clone());
            }
        }
        assert!(
            fixture
                .state
                .notification_service
                .save_binding(
                    &fixture.actor,
                    fixture.key.1,
                    input,
                    binding.plugin_mcp_instance_id.clone()
                )
                .await
                .is_err(),
            "invalid binding case {change}"
        );
    }
    let saved = fixture
        .state
        .notification_service
        .get_binding(&fixture.actor, fixture.key.1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        saved.revision, binding.revision,
        "invalid saves cannot mutate the binding"
    );
    assert_eq!(saved.config.commands, binding.config.commands);
}

#[cfg(feature = "test-support")]
#[path = "event_commands_workflow.rs"]
mod workflow;
