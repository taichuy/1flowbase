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
    legacy.remove("command_settings");
    let legacy: CommunicationBindingInput = serde_json::from_value(legacy.into()).unwrap();
    assert!(legacy.command_settings.is_empty());
    let mut config = legacy;
    config.command_settings = vec![CommunicationCommandSetting {
        command_id: "host/model".into(),
        enabled: false,
    }];
    let saved = service
        .save_binding(&actor, scope.instance_id, config, "command-tools".into())
        .await
        .unwrap();
    assert!(
        !service
            .command_catalog(&actor, &saved.config.command_settings)
            .await
            .unwrap()
            .iter()
            .find(|row| row.command_id == "host/model")
            .unwrap()
            .enabled
    );
    let intent = service
        .resolve_command(&actor, &saved.config.command_settings, "/model")
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
