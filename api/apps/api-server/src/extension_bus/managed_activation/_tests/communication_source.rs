//! The communication command source is the original committed receipt, not custom /event.
use super::*;
use control_plane::notifications::{CommunicationCommandIntent, NotificationService};

fn commands_manifest() -> Value {
    let mut raw = manifest("commands", "communication_command");
    raw["communication_commands"] = json!([
        {"command_id":"acme/acme.composition-commands/status","name":"status","description":"Read status","usage":"/status","contribution_id":"acme.composition-commands.status","default_enabled":true}
    ]);
    raw["managed"]["module"]["contributions"] = json!([{
        "contribution_id":"acme.composition-commands.status","contributor_module_id":"acme.composition-commands",
        "point_id":COMMUNICATION_COMMAND_POINT,"contract_version":"1","required_permissions":["event.subscribe"],"mode":"append"
    }]);
    raw["managed"]["execution_bindings"] = json!([{
        "contribution_id":"acme.composition-commands.status","execution_mode":"process_per_call",
        "runtime":{"protocol":"stdio_json","entry":"bin/worker.py"},"handler":"communication_command"
    }]);
    raw
}

async fn receive(
    fixture: &Fixture,
    service: &NotificationService<MainDurableStore>,
    channel: Uuid,
) -> (domain::CommunicationConversationScope, Uuid) {
    let now = time::OffsetDateTime::now_utc();
    fixture
        .runtime
        .store
        .save_notification_instance(SaveNotificationInstanceInput {
            record: domain::NotificationInstanceRecord {
                id: channel,
                workspace_id: fixture.actor.current_workspace_id,
                type_code: "communication.source-fixture".into(),
                installation_id: None,
                display_name: "Source command receipt".into(),
                status: domain::NotificationInstanceStatus::Enabled,
                config_json: json!({}),
                configured_secret_fields: vec![],
                created_at: now,
                updated_at: now,
            },
            secret_config_json: json!({}),
            master_key: "source-fixture-key".into(),
            create: true,
            initial_authorization: None,
        })
        .await
        .unwrap();
    service
        .save_binding(
            &fixture.actor,
            channel,
            domain::CommunicationBindingInput {
                event_commands: vec![],
                application_id: Uuid::now_v7(),
                model: None,
                reasoning_effort: None,
                file_table_id: None,
                mcp_instance_ids: vec![],
                system_prompt: String::new(),
                command_settings: vec![],
            },
            "fixture-tools".into(),
        )
        .await
        .unwrap();
    let revision = fixture
        .runtime
        .store
        .resolve_notification_authorization(
            fixture.actor.current_workspace_id,
            channel,
            "source-fixture-key",
        )
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: SaveNotificationAuthorizationInput {
                workspace_id: fixture.actor.current_workspace_id,
                instance_id: channel,
                expected_revision: revision,
                authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
                managed_secrets: Default::default(),
                master_key: "source-fixture-key".into(),
            },
            candidate_cursor: None,
            messages: vec![CommunicationReceiveMessage {
                provider_message_id: "source-command-message".into(),
                conversation_id: "source-command-chat".into(),
                sender_id: "alice".into(),
                sender_display_name: None,
                received_at_ms: 42,
                text: Some("/status fixture-fail".into()),
                attachments: vec![],
            }],
        })
        .await
        .unwrap()
        .unwrap();
    let (input, conversation): (Uuid, Uuid) = sqlx::query_as("SELECT id,conversation_id FROM communication_inputs WHERE scope_id=$1 AND instance_id=$2 AND external_message_key='source-command-message'")
        .bind(fixture.actor.current_workspace_id).bind(channel).fetch_one(fixture.runtime.store.pool()).await.unwrap();
    (
        domain::CommunicationConversationScope {
            workspace_id: fixture.actor.current_workspace_id,
            instance_id: channel,
            external_identity: "alice".into(),
            conversation_id: conversation,
        },
        input,
    )
}

#[tokio::test]
async fn system_event_center_communication_source_real_receipt_commit_handoff_and_failed_sdk_ack() {
    let fixture = Fixture::new().await;
    owned(&fixture, async {
        fixture.install(commands_manifest()).await;
        let source = fixture.source(COMMUNICATION_COMMAND_POINT);
        assert_eq!(source.descriptor.origin, EventDefinitionOrigin::Plugin);
        assert_eq!(source.descriptor.scope, fixture.scope());
        assert_eq!(source.descriptor.point_id, COMMUNICATION_COMMAND_POINT);
        let published = fixture.target(COMMUNICATION_COMMAND_POINT, EventTriggerPhase::Published);
        let consumed = fixture.target(COMMUNICATION_COMMAND_POINT, EventTriggerPhase::Consumed);
        let service = NotificationService::new(fixture.runtime.store.clone(), "source-fixture-key".into())
            .with_command_provider(Arc::new(ManagedCommunicationCommandProvider::new(fixture.runtime.composition.clone())));
        let (scope, input) = receive(&fixture, &service, Uuid::now_v7()).await;
        assert!(published.try_recv().unwrap().is_none(), "ordinary receive is not a command publication");
        let intent = service.resolve_command(&fixture.actor, &[], "/status fixture-fail").await.unwrap();
        assert!(matches!(intent, CommunicationCommandIntent::Plugin { .. }));
        // Force a real transaction rollback at its durable receipt INSERT. No observation can
        // escape the earlier outbox INSERT in the same owner transaction.
        sqlx::query("CREATE FUNCTION reject_source_command_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'source command rollback fixture'; END $$")
            .execute(fixture.runtime.store.pool()).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_source_command_receipt BEFORE INSERT ON communication_command_events FOR EACH ROW EXECUTE FUNCTION reject_source_command_receipt()")
            .execute(fixture.runtime.store.pool()).await.unwrap();
        assert!(service.enqueue_command(&scope, input, intent).await.is_err());
        assert!(published.try_recv().unwrap().is_none());
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM lifecycle_outbox WHERE event_id=$1").bind(input).fetch_one(fixture.runtime.store.pool()).await.unwrap(), 0);
        sqlx::query("DROP TRIGGER reject_source_command_receipt ON communication_command_events").execute(fixture.runtime.store.pool()).await.unwrap();
        let intent = service.resolve_command(&fixture.actor, &[], "/status fixture-fail").await.unwrap();
        let queued = service.enqueue_command(&scope, input, intent).await.unwrap();
        assert_eq!(queued.event_id, input);
        let delivery = published.try_recv().unwrap().expect("command commit must publish the source fact");
        let signal = delivery.signal();
        assert_eq!(signal.contract_id, COMMUNICATION_COMMAND_POINT);
        let context = signal.source_context.as_ref().unwrap();
        assert_eq!(context.scope, fixture.scope());
        assert_eq!(context.source_id, source.descriptor.source_id);
        assert_eq!(context.original_event_id.as_deref(), Some(input.to_string().as_str()));
        let EventCenterSignalBody::Published { payload } = &signal.body else { panic!("committed original command envelope required") };
        let command: ManagedEventDelivery = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(command.payload, serde_json::to_value(&queued.request).unwrap());
        assert_eq!(command.workspace_id, scope.workspace_id.to_string());
        assert!(jsonschema::validator_for(&source.descriptor.payload_schema).unwrap().is_valid(payload));
        let durable: Vec<u8> = sqlx::query_scalar("SELECT canonical_payload FROM lifecycle_outbox WHERE event_id=$1").bind(input).fetch_one(fixture.runtime.store.pool()).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&durable).unwrap(), *payload);
        drop(delivery);
        assert!(original_handoffs(&fixture, COMMUNICATION_COMMAND_POINT).is_empty());
        assert!(consumed.try_recv().unwrap().is_none());
        let intent = service.resolve_command(&fixture.actor, &[], "/status fixture-fail").await.unwrap();
        assert_eq!(service.enqueue_command(&scope, input, intent).await.unwrap().event_id, input);
        assert!(published.try_recv().unwrap().is_none(), "receipt dedup must not republish");
        assert_eq!(fixture.dispatcher().run_once().await.unwrap(), 1);
        let result = service.command_for_input(&scope, input).await.unwrap().unwrap();
        assert_eq!(result.failure_code.as_deref(), Some("fixture_command_failed"));
        assert!(result.reply.is_some(), "actual SDK failure must be committed before transport ACK");
        let (status, attempts): (String,i32) = sqlx::query_as("SELECT status,attempt_count FROM lifecycle_outbox_deliveries WHERE event_id=$1")
            .bind(input).fetch_one(fixture.runtime.store.pool()).await.unwrap();
        assert_eq!((status.as_str(), attempts), ("delivered",1));
        assert_eq!(fixture.dispatcher().run_once().await.unwrap(), 0, "ACK prevents duplicate original handoff");
        let handoffs = original_handoffs(&fixture, COMMUNICATION_COMMAND_POINT);
        assert_eq!(handoffs.len(), 1, "failed SDK result cannot erase the earlier original handoff");
        assert_eq!(handoffs[0].original_subscriber_id.as_ref(), queued.subscriber_id.as_ref());
        assert!(handoffs[0].original_consumer_id.as_ref().is_some_and(|id| !id.is_empty()));
        let observed = consumed.try_recv().unwrap().expect("original dispatcher must emit Consumed");
        assert_eq!(observed.signal().source_context.as_ref().unwrap().original_event_id.as_deref(), Some(input.to_string().as_str()));
        fixture.observer.refresh_queue_snapshots(fixture.runtime.store.lifecycle_source_queue_snapshots().await.unwrap());
        let queues = fixture.source(COMMUNICATION_COMMAND_POINT).queues;
        assert_eq!(queues.len(),1);
        assert_eq!(queues[0].buffered,Some(0));
        assert_eq!(queues[0].in_flight,Some(0));
        assert_eq!(Some(&queues[0].subscriber_id), queued.subscriber_id.as_ref());
        assert_eq!(original_handoffs(&fixture, COMMUNICATION_COMMAND_POINT), handoffs);
        assert!(!serde_json::to_string(&handoffs).unwrap().contains("fixture-fail"));
        assert!(fixture.center.snapshot_sources(EventSourceScope::System).activity.is_empty());
        drop(observed); drop(published); drop(consumed);
    }).await;
}
