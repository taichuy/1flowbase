use super::communications::{authorization, config, create_channel, received, KEY};
use control_plane::{notifications::*, ports::*};
use domain::*;
use extension_contracts::CommunicationCommandReply;
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

async fn fixture() -> (
    PgControlPlaneStore,
    NotificationService<PgControlPlaneStore>,
    ActorContext,
    CommunicationConversationScope,
    Vec<Uuid>,
) {
    let (store, workspace, root) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, workspace.id).await;
    service
        .save_binding(&actor, channel, config(), "command-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(workspace.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    let mut commands = vec![
        received("cmd-one", "alice", false),
        received("cmd-two", "alice", false),
    ];
    commands[0].text = Some("/help".into());
    commands[1].text = Some("/model".into());
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(workspace.id, channel, revision),
            candidate_cursor: None,
            messages: commands,
        })
        .await
        .unwrap()
        .unwrap();
    let pending = service
        .scan_pending_admissions(workspace.id, channel, 10)
        .await
        .unwrap();
    let conversation_id = pending[0].conversation.id;
    let scope = CommunicationConversationScope {
        workspace_id: workspace.id,
        instance_id: channel,
        external_identity: "alice".into(),
        conversation_id,
    };
    let inputs = service.pending_admissions(&scope).await.unwrap();
    (store, service, actor, scope, inputs)
}

async fn send_log(
    store: &PgControlPlaneStore,
    actor: &ActorContext,
    instance: Uuid,
    text: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    store
        .create_notification_send_log(NotificationSendLog {
            id,
            workspace_id: actor.current_workspace_id,
            actor_user_id: actor.user_id,
            instance_id: instance,
            display_name: None,
            type_code: None,
            source: NotificationSource::Workflow,
            status: "pending".into(),
            failure_code: None,
            title: None,
            content: None,
            recipient_count: None,
            extra: serde_json::json!({}),
            created_at: time::OffsetDateTime::now_utc(),
            completed_at: None,
        })
        .await
        .unwrap();
    store
        .snapshot_notification_send_log(
            actor.current_workspace_id,
            id,
            extension_contracts::NotificationLogSnapshot {
                title: String::new(),
                content: text.into(),
                extra: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn channel_send_history_reuses_command_reply_and_keeps_proactive_and_unknown_outcomes_distinct(
) {
    let (store, service, actor, scope, inputs) = fixture().await;
    service
        .enqueue_command(
            &scope,
            inputs[0],
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Help),
        )
        .await
        .unwrap();
    let claim = service
        .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    service
        .complete_builtin_command(
            &claim,
            Uuid::parse_str(&claim.record.request.session_id).unwrap(),
            CommunicationCommandReply {
                text: "八字报告".into(),
            },
            None,
        )
        .await
        .unwrap();
    let first = send_log(&store, &actor, scope.instance_id, "八字报告").await;
    let target = service
        .command_response_target(&scope, inputs[0])
        .await
        .unwrap()
        .unwrap();
    store
        .prepare_communication_send(PrepareCommunicationSend {
            workspace_id: scope.workspace_id,
            instance_id: scope.instance_id,
            send_log_id: first,
            target: Some(target.clone()),
            context: Some(CommunicationSendContext {
                scope: scope.clone(),
                input_id: inputs[0],
                run_id: None,
            }),
        })
        .await
        .unwrap();
    let receipt = extension_contracts::NotificationSendReceipt {
        request_id: first.to_string(),
        recipient_id: target.sender_id.clone(),
        conversation_id: target.conversation_id.clone(),
        provider_message_id: None,
        client_message_id: Some(format!("client-{first}")),
    };
    for _ in 0..2 {
        store
            .record_communication_send_receipt(RecordCommunicationSendReceipt {
                workspace_id: scope.workspace_id,
                send_log_id: first,
                send_status: CommunicationSendStatus::Succeeded,
                log_extra: None,
                failure_code: None,
                receipt: Some(receipt.clone()),
            })
            .await
            .unwrap();
    }
    let history_scope = CommunicationHistoryScope {
        workspace_id: scope.workspace_id,
        instance_id: scope.instance_id,
        external_identity: scope.external_identity.clone(),
    };
    let history = service
        .scoped_message_history(&history_scope, scope.conversation_id, None, 5)
        .await
        .unwrap();
    let replies = history
        .items
        .iter()
        .filter(|m| m.direction == CommunicationDirection::Outbound)
        .collect::<Vec<_>>();
    assert_eq!(
        replies.len(),
        1,
        "an already reserved command reply must not be inserted twice"
    );
    assert_eq!(replies[0].input_id, Some(inputs[0]));
    assert_eq!(
        replies[0].send_status,
        Some(CommunicationSendStatus::Succeeded)
    );
    assert_eq!(
        store
            .get_notification_send_log(scope.workspace_id, first)
            .await
            .unwrap()
            .unwrap()
            .status,
        "success"
    );

    let proactive = send_log(&store, &actor, scope.instance_id, "独立主动消息").await;
    store
        .prepare_communication_send(PrepareCommunicationSend {
            workspace_id: scope.workspace_id,
            instance_id: scope.instance_id,
            send_log_id: proactive,
            target: None,
            context: None,
        })
        .await
        .unwrap();
    // Reject only this send's archive write. The success log and proactive message must
    // both roll back, and retrying the same receipt after storage recovers must insert once.
    let reject = format!("ALTER TABLE communication_message_sends ADD CONSTRAINT fixture_reject_archive CHECK (send_log_id<>'{proactive}'::uuid OR send_status<>'succeeded')");
    // SQL safety: this fixed constraint DDL interpolates only a host-generated UUID.
    sqlx::query(sqlx::AssertSqlSafe(reject))
        .execute(store.pool())
        .await
        .unwrap();
    let proactive_receipt = RecordCommunicationSendReceipt {
        workspace_id: scope.workspace_id,
        send_log_id: proactive,
        send_status: CommunicationSendStatus::Succeeded,
        log_extra: None,
        failure_code: None,
        receipt: Some(extension_contracts::NotificationSendReceipt {
            request_id: proactive.to_string(),
            ..receipt.clone()
        }),
    };
    assert!(store
        .record_communication_send_receipt(proactive_receipt.clone())
        .await
        .is_err());
    assert_eq!(
        store
            .get_notification_send_log(scope.workspace_id, proactive)
            .await
            .unwrap()
            .unwrap()
            .status,
        "pending"
    );
    let absent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_messages WHERE scope_id=$1 AND content='独立主动消息'",
    )
    .bind(scope.workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(absent, 0);
    sqlx::query("ALTER TABLE communication_message_sends DROP CONSTRAINT fixture_reject_archive")
        .execute(store.pool())
        .await
        .unwrap();
    store
        .record_communication_send_receipt(proactive_receipt)
        .await
        .unwrap();
    let history = service
        .scoped_message_history(&history_scope, scope.conversation_id, None, 5)
        .await
        .unwrap();
    let proactive_message = history
        .items
        .iter()
        .find(|m| m.content.as_deref() == Some("独立主动消息"))
        .unwrap();
    assert_eq!(
        proactive_message.input_id, None,
        "a channel send cannot fabricate an inbound input"
    );

    let second = send_log(&store, &actor, scope.instance_id, "紫微报告").await;
    let second_target = extension_contracts::NotificationSendTarget {
        provider_message_id: "cmd-two".into(),
        ..target
    };
    store
        .prepare_communication_send(PrepareCommunicationSend {
            workspace_id: scope.workspace_id,
            instance_id: scope.instance_id,
            send_log_id: second,
            target: Some(second_target),
            context: None,
        })
        .await
        .unwrap();
    let wrong = store
        .record_communication_send_receipt(RecordCommunicationSendReceipt {
            workspace_id: scope.workspace_id,
            send_log_id: second,
            send_status: CommunicationSendStatus::Succeeded,
            log_extra: None,
            failure_code: None,
            receipt: Some(receipt.clone()),
        })
        .await;
    assert!(
        wrong.is_err(),
        "a different send's receipt cannot finish this message"
    );
    // Simulate the existing input owner's cancellation/restart recovery, before any
    // receipt finalizer runs. Its unknown must not be masked by the reserved sidecar.
    sqlx::query("UPDATE communication_inputs SET send_status='unknown',failure_code='send_interrupted' WHERE scope_id=$1 AND id=$2")
        .bind(scope.workspace_id).bind(inputs[1]).execute(store.pool()).await.unwrap();
    let recovered = service
        .scoped_message_history(&history_scope, scope.conversation_id, None, 5)
        .await
        .unwrap();
    let interrupted = recovered
        .items
        .iter()
        .find(|m| m.input_id == Some(inputs[1]) && m.direction == CommunicationDirection::Outbound)
        .unwrap();
    assert_eq!(
        interrupted.send_status,
        Some(CommunicationSendStatus::Unknown)
    );
    assert!(!interrupted.can_retry);
    store
        .record_communication_send_receipt(RecordCommunicationSendReceipt {
            workspace_id: scope.workspace_id,
            send_log_id: second,
            send_status: CommunicationSendStatus::Unknown,
            log_extra: None,
            failure_code: Some("provider_result_unknown".into()),
            receipt: None,
        })
        .await
        .unwrap();
    store
        .record_communication_send_receipt(RecordCommunicationSendReceipt {
            workspace_id: scope.workspace_id,
            send_log_id: second,
            send_status: CommunicationSendStatus::Succeeded,
            log_extra: None,
            failure_code: None,
            receipt: Some(extension_contracts::NotificationSendReceipt {
                request_id: second.to_string(),
                ..receipt
            }),
        })
        .await
        .unwrap();
    let history = service
        .scoped_message_history(&history_scope, scope.conversation_id, None, 5)
        .await
        .unwrap();
    let second_message = history
        .items
        .iter()
        .find(|m| m.input_id == Some(inputs[1]) && m.direction == CommunicationDirection::Outbound)
        .unwrap();
    assert_eq!(
        second_message.send_status,
        Some(CommunicationSendStatus::Unknown)
    );
    let mut other_sender = history_scope;
    other_sender.external_identity = "bob".into();
    assert!(service
        .scoped_message_history(&other_sender, scope.conversation_id, None, 5)
        .await
        .unwrap()
        .items
        .is_empty());
}

#[tokio::test]
async fn communication_command_requests_claim_order_receipts_and_unknown_send_are_durable() {
    let (store, service, _, scope, inputs) = fixture().await;
    let first = service
        .enqueue_command(
            &scope,
            inputs[0],
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Help),
        )
        .await
        .unwrap();
    let replay = service
        .enqueue_command(
            &scope,
            inputs[0],
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Help),
        )
        .await
        .unwrap();
    assert_eq!(first.event_id, replay.event_id);
    service
        .enqueue_command(
            &scope,
            inputs[1],
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Model { model: None }),
        )
        .await
        .unwrap();
    assert_eq!(service.pending_admissions(&scope).await.unwrap(), inputs);
    let claim = service
        .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.record.event_id, first.event_id);
    assert!(
        service
            .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
            .await
            .unwrap()
            .is_none(),
        "later command cannot skip a live claim in same chat"
    );
    assert!(service
        .renew_builtin_command_claim(&claim, 60)
        .await
        .unwrap());
    let session = Uuid::parse_str(&first.request.session_id).unwrap();
    service
        .complete_builtin_command(
            &claim,
            session,
            CommunicationCommandReply {
                text: "Help receipt".into(),
            },
            None,
        )
        .await
        .unwrap();
    let replay = service
        .complete_builtin_command(
            &claim,
            session,
            CommunicationCommandReply {
                text: "must not overwrite".into(),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(replay.reply.unwrap().text, "Help receipt");
    assert_eq!(
        service.pending_admissions(&scope).await.unwrap(),
        vec![inputs[1]]
    );
    assert!(service
        .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        service
            .pending_command_responses(scope.workspace_id, scope.instance_id, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    let send = service
        .reserve_command_response(&scope, first.event_id)
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .reserve_command_response(&scope, first.event_id)
        .await
        .unwrap()
        .is_none());
    assert!(service
        .record_command_response(
            &scope,
            first.event_id,
            send.claim_id,
            CommunicationCommandSendOutcome::Unknown
        )
        .await
        .unwrap());
    assert!(service
        .pending_command_responses(scope.workspace_id, scope.instance_id, 10)
        .await
        .unwrap()
        .is_empty());
    assert!(service
        .reserve_command_response(&scope, first.event_id)
        .await
        .unwrap()
        .is_none());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM communication_messages WHERE input_id=$1 AND direction='outbound'",
    )
    .bind(inputs[0])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    let raw: String = sqlx::query_scalar(
        "SELECT content FROM communication_messages WHERE input_id=$1 AND direction='inbound'",
    )
    .bind(inputs[0])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(raw, "/help");
}

#[tokio::test]
async fn communication_command_settings_legacy_defaults_and_rejections_never_admit_runs() {
    let (store, service, actor, scope, inputs) = fixture().await;
    let legacy = serde_json::to_value(config()).unwrap();
    let mut legacy = legacy.as_object().unwrap().clone();
    legacy.remove("commands");
    let legacy: CommunicationBindingInput = serde_json::from_value(legacy.into()).unwrap();
    assert!(legacy.commands.is_empty());
    let mut config = legacy;
    config.commands = vec![domain::CommunicationCommandConfiguration::Registered(
        CommunicationCommandSetting {
            command_id: "host/model".into(),
            enabled: false,
        },
    )];
    let saved = service
        .save_binding(&actor, scope.instance_id, config, "command-tools".into())
        .await
        .unwrap();
    assert!(
        !service
            .command_catalog(&actor, &saved.config.command_settings())
            .await
            .unwrap()
            .iter()
            .find(|row| row.command_id == "host/model")
            .unwrap()
            .enabled
    );
    let intent = service
        .resolve_command(&actor, &saved.config.command_settings(), "/model")
        .await
        .unwrap();
    assert!(matches!(
        intent,
        CommunicationCommandIntent::Rejected { .. }
    ));
    service
        .enqueue_command(&scope, inputs[0], intent)
        .await
        .unwrap();
    let claim = service
        .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    let CommunicationCommandIntent::Rejected { code, reply } =
        decode_builtin_command(&claim.record).unwrap()
    else {
        panic!("explicit rejection")
    };
    service
        .complete_builtin_command(
            &claim,
            Uuid::parse_str(&claim.record.request.session_id).unwrap(),
            CommunicationCommandReply { text: reply },
            Some(code),
        )
        .await
        .unwrap();
    let state: (String, Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT admission_status,turn_id,run_id FROM communication_inputs WHERE id=$1",
    )
    .bind(inputs[0])
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(state, ("excluded".into(), None, None));
}

#[tokio::test]
async fn unified_commands_migration_preserves_switches_event_mapping_and_channel_configuration() {
    let (store, service, actor, scope, _) = fixture().await;
    let mut old = serde_json::to_value(config()).unwrap();
    old.as_object_mut().unwrap().remove("commands");
    old["command_settings"] = serde_json::json!([{"command_id":"host/help","enabled":false}]);
    old["event_commands"] = serde_json::json!([{
        "command_id":"user/paipan","name":"paipan","description":"Chart","enabled":false,
        "contract_id":"custom.chart","input_mapping":{"arguments":"/arguments","instance_id":"/instance_id"}
    }]);
    sqlx::query(
        "UPDATE communication_bindings SET config_json=$3 WHERE scope_id=$1 AND instance_id=$2",
    )
    .bind(scope.workspace_id)
    .bind(scope.instance_id)
    .bind(&old)
    .execute(store.pool())
    .await
    .unwrap();
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM communication_bindings WHERE scope_id=$1 AND instance_id=$2",
    )
    .bind(scope.workspace_id)
    .bind(scope.instance_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let migration = include_str!("../../../../storage/durable/postgres/migrations/20261111020000_unified_communication_commands.sql");
    sqlx::raw_sql(migration)
        .execute(store.pool())
        .await
        .unwrap();
    let migrated = service
        .get_binding(&actor, scope.instance_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(migrated.revision, revision + 1);
    let mut preserved = serde_json::to_value(&migrated.config).unwrap();
    preserved.as_object_mut().unwrap().remove("commands");
    let mut expected = old.clone();
    expected.as_object_mut().unwrap().remove("command_settings");
    expected.as_object_mut().unwrap().remove("event_commands");
    assert_eq!(preserved, expected);
    assert!(!migrated.config.command_settings()[0].enabled);
    assert_eq!(
        serde_json::to_value(&migrated.config.event_commands()).unwrap(),
        old["event_commands"]
    );
    let wire = serde_json::to_value(&migrated.config).unwrap();
    assert!(wire.get("command_settings").is_none());
    assert!(wire.get("event_commands").is_none());
    sqlx::raw_sql(migration)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        service
            .get_binding(&actor, scope.instance_id)
            .await
            .unwrap()
            .unwrap()
            .revision,
        migrated.revision
    );
}

#[tokio::test]
async fn communication_command_reply_reservation_fences_old_session_and_expired_claim() {
    let (store, service, _, scope, inputs) = fixture().await;
    let first = service
        .enqueue_command(
            &scope,
            inputs[0],
            CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Help),
        )
        .await
        .unwrap();
    let stale = service
        .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE communication_command_events SET effect_claim_expires_at=clock_timestamp()-interval '1 second' WHERE event_id=$1").bind(first.event_id).execute(store.pool()).await.unwrap();
    let current = service
        .claim_builtin_command(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    let session = Uuid::parse_str(&first.request.session_id).unwrap();
    assert!(service
        .complete_builtin_command(
            &stale,
            session,
            CommunicationCommandReply {
                text: "stale".into()
            },
            None
        )
        .await
        .is_err());
    service
        .complete_builtin_command(
            &current,
            session,
            CommunicationCommandReply {
                text: "ready".into(),
            },
            None,
        )
        .await
        .unwrap();
    service
        .rotate_session(&scope, "new-session")
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .reserve_command_response(&scope, first.event_id)
        .await
        .unwrap()
        .is_none());
    assert!(service
        .pending_command_responses(scope.workspace_id, scope.instance_id, 10)
        .await
        .unwrap()
        .is_empty());
}
