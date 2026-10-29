//! Real PostgreSQL ordering/fencing fixtures for the Root's single concentrated Test Batch.
use super::communications::{authorization, config, create_channel, received, KEY};
use control_plane::{notifications::NotificationService, ports::*};
use domain::*;
use serde_json::json;
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

type Service = NotificationService<PgControlPlaneStore>;

async fn fixture() -> (
    PgControlPlaneStore,
    Service,
    CommunicationConversationScope,
    Uuid,
) {
    let (store, workspace, root) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, workspace.tenant_id, workspace.id, None)
        .await
        .unwrap();
    let channel = create_channel(&store, workspace.id).await;
    let service = Service::new(store.clone(), KEY.into());
    let mut binding = config();
    binding.model = Some("binding-model".into());
    service
        .save_binding(&actor, channel, binding, "session-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(workspace.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    let mut first = received("first", "alice", false);
    first.text = Some("original question".into());
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(workspace.id, channel, revision),
            candidate_cursor: None,
            messages: vec![first],
        })
        .await
        .unwrap()
        .unwrap();
    let history_scope = CommunicationHistoryScope {
        workspace_id: workspace.id,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    let conversation = service
        .scoped_conversations(&history_scope, None, 5)
        .await
        .unwrap()
        .items[0]
        .id;
    let scope = CommunicationConversationScope {
        workspace_id: workspace.id,
        instance_id: channel,
        external_identity: "alice".into(),
        conversation_id: conversation,
    };
    let input = service.pending_admissions(&scope).await.unwrap()[0];
    (store, service, scope, input)
}

async fn receive(
    store: &PgControlPlaneStore,
    service: &Service,
    scope: &CommunicationConversationScope,
    key: &str,
    file: bool,
) -> Uuid {
    let revision = store
        .resolve_notification_authorization(scope.workspace_id, scope.instance_id, KEY)
        .await
        .unwrap()
        .revision;
    let mut message = received(key, &scope.external_identity, file);
    message.text = Some(format!("text for {key}"));
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(scope.workspace_id, scope.instance_id, revision),
            candidate_cursor: None,
            messages: vec![message],
        })
        .await
        .unwrap()
        .unwrap();
    *service
        .pending_admissions(scope)
        .await
        .unwrap()
        .last()
        .unwrap()
}

fn checkpoint(revision: i64, consumed: i64) -> CommunicationTurnCheckpoint {
    CommunicationTurnCheckpoint {
        expected_checkpoint_revision: revision,
        consumed_watermark: consumed,
        checkpoint: json!({"version":1,"completed_nodes":{"tool":{"effect_id":"already-committed"}},"transcript":[{"role":"tool","content":"persisted result"}],"consumed":consumed}),
    }
}

fn candidate(watermark: i64, revision: i64) -> CommunicationTurnFinalCandidate {
    CommunicationTurnFinalCandidate {
        expected_received_watermark: watermark,
        expected_checkpoint_revision: revision,
        final_reply: "one complete answer".into(),
    }
}

#[tokio::test]
async fn communication_session_first_admission_and_turn_claim_have_one_owner() {
    let (store, service, scope, input) = fixture().await;
    let (left, right) = tokio::join!(
        service.admit_input(&scope, input),
        service.admit_input(&scope, input)
    );
    let left = left.unwrap().unwrap();
    let right = right.unwrap().unwrap();
    assert_eq!(left.session.id, right.session.id);
    assert_eq!(left.turn.id, right.turn.id);
    assert_eq!(left.input_sequence, 1);
    assert_eq!(right.input_sequence, 1);
    assert_ne!(
        left.session.id, scope.conversation_id,
        "AI identity is independent of history identity"
    );
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM communication_sessions),(SELECT count(*) FROM communication_reply_turns)").fetch_one(store.pool()).await.unwrap();
    assert_eq!(counts, (1, 1));
    let (left, right) = tokio::join!(
        service.claim_pending_turn(scope.workspace_id, scope.instance_id, 60),
        service.claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_ne!(left.is_some(), right.is_some());
    let claimed = left.or(right).unwrap();
    assert_eq!(
        claimed.pending.inputs[0].content.as_deref(),
        Some("original question")
    );
    assert!(
        service
            .claim_pending_input(scope.workspace_id, scope.instance_id, 60)
            .await
            .unwrap()
            .is_none(),
        "constituents cannot enter the legacy dispatcher"
    );
    for forged in [
        CommunicationConversationScope {
            workspace_id: Uuid::now_v7(),
            ..scope.clone()
        },
        CommunicationConversationScope {
            instance_id: Uuid::now_v7(),
            ..scope.clone()
        },
        CommunicationConversationScope {
            external_identity: "bob".into(),
            ..scope.clone()
        },
        CommunicationConversationScope {
            conversation_id: Uuid::now_v7(),
            ..scope.clone()
        },
    ] {
        assert!(service.admit_input(&forged, input).await.unwrap().is_none());
        assert!(service
            .pending_admissions(&forged)
            .await
            .unwrap()
            .is_empty());
        assert!(service.active_session(&forged).await.unwrap().is_none());
        let bad = CommunicationTurnClaim {
            scope: forged,
            ..claimed.claim.clone()
        };
        assert!(!service.renew_turn_claim(&bad, 60).await.unwrap());
        assert!(service.turn_inputs(&bad).await.unwrap().is_none());
    }
    let stale = CommunicationTurnClaim {
        session_epoch: claimed.claim.session_epoch + 1,
        ..claimed.claim.clone()
    };
    assert!(!service.link_turn_run(&stale, Uuid::now_v7()).await.unwrap());
    let stale = CommunicationTurnClaim {
        claim_id: Uuid::now_v7(),
        ..claimed.claim.clone()
    };
    assert!(!service.link_turn_run(&stale, Uuid::now_v7()).await.unwrap());
}

#[tokio::test]
async fn communication_turn_append_checkpoint_and_seal_preserve_every_original_once() {
    let (store, service, scope, first) = fixture().await;
    let admission = service.admit_input(&scope, first).await.unwrap().unwrap();
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    let claim = &claimed.claim;
    let run = Uuid::now_v7();
    assert!(service.link_turn_run(claim, run).await.unwrap());
    assert!(!service.link_turn_run(claim, Uuid::now_v7()).await.unwrap());
    for _ in 0..2 {
        let peek = service.turn_inputs(claim).await.unwrap().unwrap();
        assert_eq!(peek.inputs[0].input_id, first);
        assert_eq!(peek.turn.consumed_watermark, 0, "peek does not ACK input");
    }
    assert!(service
        .checkpoint_turn(claim, checkpoint(0, 1))
        .await
        .unwrap());
    let second = receive(&store, &service, &scope, "supplement", true).await;
    let pending = match service.seal_turn(claim, candidate(1, 1)).await.unwrap() {
        CommunicationTurnSeal::Pending(pending) => pending,
        _ => panic!("received-but-unclassified input must prevent send reservation"),
    };
    assert_eq!(pending.pending_admission_input_ids, vec![second]);
    assert_eq!(
        pending.checkpoint.unwrap()["completed_nodes"]["tool"]["effect_id"],
        "already-committed"
    );
    let (added, seal) = tokio::join!(
        service.admit_input(&scope, second),
        service.seal_turn(claim, candidate(1, 1))
    );
    let added = added.unwrap().unwrap();
    assert_eq!(added.turn.id, admission.turn.id);
    assert_eq!(added.input_sequence, 2);
    assert!(matches!(seal.unwrap(), CommunicationTurnSeal::Pending(_)));
    let peek = service.turn_inputs(claim).await.unwrap().unwrap();
    assert_eq!(peek.inputs.len(), 1);
    assert_eq!(peek.inputs[0].input_id, second);
    assert_eq!(
        peek.inputs[0].content.as_deref(),
        Some("text for supplement")
    );
    assert_eq!(peek.turn.run_id, Some(run));
    assert!(
        !service
            .checkpoint_turn(claim, checkpoint(1, 2))
            .await
            .unwrap(),
        "unimported files cannot be consumed"
    );
    let file = peek.inputs[0].attachments[0].record.id;
    let file_table = Uuid::now_v7();
    let file_record = Uuid::now_v7();
    assert!(service
        .record_turn_attached_file(
            claim,
            CommunicationAttachmentUpdate {
                attachment_id: file,
                file_table_id: Some(file_table),
                file_record_id: Some(file_record),
                failure_code: None
            }
        )
        .await
        .unwrap());
    assert!(
        !service
            .checkpoint_turn(claim, checkpoint(0, 2))
            .await
            .unwrap(),
        "old checkpoint writers lose CAS"
    );
    assert!(service
        .checkpoint_turn(claim, checkpoint(1, 2))
        .await
        .unwrap());
    let stale = match service.seal_turn(claim, candidate(1, 1)).await.unwrap() {
        CommunicationTurnSeal::Pending(pending) => pending,
        _ => panic!("stale final watermark must resume, never send"),
    };
    assert_eq!(stale.turn.received_watermark, 2);
    assert_eq!(stale.turn.consumed_watermark, 2);
    assert_eq!(
        stale.checkpoint.unwrap()["transcript"][0]["content"],
        "persisted result"
    );
    let reserved = match service.seal_turn(claim, candidate(2, 2)).await.unwrap() {
        CommunicationTurnSeal::Reserved { turn, final_reply } => {
            assert_eq!(final_reply, "one complete answer");
            turn
        }
        _ => panic!("current final watermark must reserve exactly one send"),
    };
    assert_eq!(reserved.send_status, CommunicationSendStatus::Sending);
    assert!(matches!(
        service.seal_turn(claim, candidate(2, 2)).await.unwrap(),
        CommunicationTurnSeal::Fenced
    ));
    let third = receive(&store, &service, &scope, "after-seal", false).await;
    let next = service.admit_input(&scope, third).await.unwrap().unwrap();
    assert_eq!(next.session.id, admission.session.id);
    assert_ne!(next.turn.id, admission.turn.id);
    assert_eq!(next.input_sequence, 1);
    assert!(
        service
            .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
            .await
            .unwrap()
            .is_none(),
        "prior send remains ordered before next turn"
    );
    assert!(service
        .record_turn_progress(claim, CommunicationTurnProgress::SendSucceeded)
        .await
        .unwrap());
    assert!(!service
        .record_turn_progress(claim, CommunicationTurnProgress::SendSucceeded)
        .await
        .unwrap());
    assert_eq!(
        service
            .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
            .await
            .unwrap()
            .unwrap()
            .claim
            .turn_id,
        next.turn.id
    );
    let counts: (i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM communication_inputs),(SELECT count(*) FROM communication_reply_turns),(SELECT count(*) FROM communication_messages WHERE direction='outbound'),(SELECT count(*) FROM communication_inputs WHERE run_id=$1)").bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        counts,
        (3, 2, 1, 1),
        "all originals, one anchor run and one outbound reply"
    );
    let history_scope = CommunicationHistoryScope {
        workspace_id: scope.workspace_id,
        instance_id: scope.instance_id,
        external_identity: scope.external_identity.clone(),
    };
    let history = service
        .scoped_message_history(&history_scope, scope.conversation_id, None, 5)
        .await
        .unwrap();
    assert_eq!(history.items.len(), 5);
    let grouped: Vec<_> = history
        .items
        .iter()
        .filter(|message| message.input_id != third)
        .collect();
    assert_eq!(grouped.len(), 4);
    assert!(grouped.iter().all(|message| message.run_id == Some(run)
        && message.send_status == Some(CommunicationSendStatus::Succeeded)));
    let stored_file = service
        .scoped_file(&history_scope, file)
        .await
        .unwrap()
        .file
        .unwrap();
    assert_eq!(stored_file.file_record_id, Some(file_record));
}

#[tokio::test]
async fn communication_session_rotation_is_idempotent_fenced_and_chat_model_scoped() {
    let (store, service, scope, first) = fixture().await;
    let old = service.admit_input(&scope, first).await.unwrap().unwrap();
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    let run = Uuid::now_v7();
    assert!(service.link_turn_run(&claimed.claim, run).await.unwrap());
    assert!(service
        .checkpoint_turn(&claimed.claim, checkpoint(0, 1))
        .await
        .unwrap());
    assert!(service
        .select_model(&scope, Some("chat-selected-model"))
        .await
        .unwrap());
    assert_eq!(
        claimed.pending.turn.selected_model.as_deref(),
        Some("binding-model"),
        "current execution model was frozen"
    );
    let (left, right) = tokio::join!(
        service.rotate_session(&scope, "command-provider-id"),
        service.rotate_session(&scope, "command-provider-id")
    );
    let left = left.unwrap().unwrap();
    let right = right.unwrap().unwrap();
    assert_ne!(left.replayed, right.replayed);
    assert_eq!(left.session.id, right.session.id);
    assert_eq!(left.session.epoch, old.session.epoch + 1);
    assert_eq!(left.invalidated_session_id, Some(old.session.id));
    assert_eq!(left.cancel_run_ids, vec![run]);
    assert_eq!(
        left.session.selected_model.as_deref(),
        Some("chat-selected-model")
    );
    assert!(!service.renew_turn_claim(&claimed.claim, 60).await.unwrap());
    assert!(service.turn_inputs(&claimed.claim).await.unwrap().is_none());
    assert!(!service
        .checkpoint_turn(&claimed.claim, checkpoint(1, 1))
        .await
        .unwrap());
    assert!(matches!(
        service
            .seal_turn(&claimed.claim, candidate(1, 1))
            .await
            .unwrap(),
        CommunicationTurnSeal::Fenced
    ));
    assert!(!service
        .record_turn_progress(
            &claimed.claim,
            CommunicationTurnProgress::RunFailed {
                failure_code: "late-result".into()
            }
        )
        .await
        .unwrap());
    assert!(!store
        .retry_communication_message(scope.workspace_id, scope.instance_id, first)
        .await
        .unwrap());
    let input = receive(&store, &service, &scope, "new-session-input", false).await;
    let next = service.admit_input(&scope, input).await.unwrap().unwrap();
    assert_eq!(next.session.id, left.session.id);
    assert_eq!(
        next.turn.selected_model.as_deref(),
        Some("chat-selected-model")
    );
    assert_eq!(next.turn.conversation_id, scope.conversation_id);
    let wrong = CommunicationConversationScope {
        external_identity: "bob".into(),
        ..scope.clone()
    };
    assert!(!service
        .select_model(&wrong, Some("forged-model"))
        .await
        .unwrap());
    assert!(service
        .rotate_session(&wrong, "command-provider-id")
        .await
        .unwrap()
        .is_none());
    let revision = store
        .resolve_notification_authorization(scope.workspace_id, scope.instance_id, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(scope.workspace_id, scope.instance_id, revision),
            candidate_cursor: None,
            messages: vec![received("bob-first", "bob", false)],
        })
        .await
        .unwrap()
        .unwrap();
    let bob_history = CommunicationHistoryScope {
        workspace_id: scope.workspace_id,
        instance_id: scope.instance_id,
        external_identity: "bob".into(),
    };
    let bob_conversation = service
        .scoped_conversations(&bob_history, None, 5)
        .await
        .unwrap()
        .items[0]
        .id;
    let bob = CommunicationConversationScope {
        conversation_id: bob_conversation,
        ..wrong
    };
    let bob_input = service.pending_admissions(&bob).await.unwrap()[0];
    let bob_admission = service.admit_input(&bob, bob_input).await.unwrap().unwrap();
    assert_ne!(bob_admission.session.id, next.session.id);
    assert_eq!(bob_admission.session.selected_model, None);
    assert_eq!(
        bob_admission.turn.selected_model.as_deref(),
        Some("binding-model")
    );
    assert_eq!(
        service.active_session(&scope).await.unwrap().unwrap().id,
        left.session.id
    );
}

#[tokio::test]
async fn communication_turn_reclaim_and_explicit_send_retry_keep_run_and_unknown_send_rules() {
    let (store, service, scope, first) = fixture().await;
    service.admit_input(&scope, first).await.unwrap().unwrap();
    let original = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    let run = Uuid::now_v7();
    assert!(service.link_turn_run(&original.claim, run).await.unwrap());
    assert!(service
        .checkpoint_turn(&original.claim, checkpoint(0, 1))
        .await
        .unwrap());
    sqlx::query("UPDATE communication_reply_turns SET claim_expires_at=clock_timestamp()-interval '1 second' WHERE id=$1").bind(original.claim.turn_id).execute(store.pool()).await.unwrap();
    let resumed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.claim.turn_id, original.claim.turn_id);
    assert_ne!(resumed.claim.claim_id, original.claim.claim_id);
    assert_eq!(resumed.pending.turn.run_id, Some(run));
    assert!(resumed.pending.inputs.is_empty());
    assert_eq!(
        resumed.pending.checkpoint.unwrap()["completed_nodes"]["tool"]["effect_id"],
        "already-committed"
    );
    assert!(!service
        .link_turn_run(&resumed.claim, Uuid::now_v7())
        .await
        .unwrap());
    assert!(matches!(
        service
            .seal_turn(&original.claim, candidate(1, 1))
            .await
            .unwrap(),
        CommunicationTurnSeal::Fenced
    ));
    assert!(matches!(
        service
            .seal_turn(&resumed.claim, candidate(1, 1))
            .await
            .unwrap(),
        CommunicationTurnSeal::Reserved { .. }
    ));
    assert!(service
        .record_turn_progress(
            &resumed.claim,
            CommunicationTurnProgress::SendFailed {
                failure_code: "known-rejected".into()
            }
        )
        .await
        .unwrap());
    assert!(store
        .retry_communication_message(scope.workspace_id, scope.instance_id, first)
        .await
        .unwrap());
    let retried = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retried.pending.turn.run_id, Some(run));
    assert_eq!(retried.final_reply.as_deref(), Some("one complete answer"));
    assert_eq!(
        service
            .reserve_turn_send(&retried.claim)
            .await
            .unwrap()
            .as_deref(),
        Some("one complete answer")
    );
    assert!(service
        .reserve_turn_send(&retried.claim)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE communication_reply_turns SET claim_expires_at=clock_timestamp()-interval '1 second' WHERE id=$1").bind(retried.claim.turn_id).execute(store.pool()).await.unwrap();
    assert!(service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .is_none());
    assert!(!store
        .retry_communication_message(scope.workspace_id, scope.instance_id, first)
        .await
        .unwrap());
    let facts: (String, i64) = sqlx::query_as("SELECT t.send_status,(SELECT count(*) FROM communication_messages WHERE direction='outbound') FROM communication_reply_turns t WHERE t.id=$1").bind(retried.claim.turn_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(facts, ("unknown".into(), 1));
}

#[tokio::test]
async fn communication_command_exclusion_and_rotation_revoke_unsent_retry() {
    let (store, service, scope, command) = fixture().await;
    assert!(service.exclude_input(&scope, command).await.unwrap());
    assert!(service.exclude_input(&scope, command).await.unwrap());
    assert!(service
        .admit_input(&scope, command)
        .await
        .unwrap()
        .is_none());
    assert!(service
        .claim_pending_input(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .is_none());
    let ordinary = receive(&store, &service, &scope, "ordinary", false).await;
    service
        .admit_input(&scope, ordinary)
        .await
        .unwrap()
        .unwrap();
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .link_turn_run(&claimed.claim, Uuid::now_v7())
        .await
        .unwrap());
    assert!(service
        .checkpoint_turn(&claimed.claim, checkpoint(0, 1))
        .await
        .unwrap());
    assert!(matches!(
        service
            .seal_turn(&claimed.claim, candidate(1, 1))
            .await
            .unwrap(),
        CommunicationTurnSeal::Reserved { .. }
    ));
    assert!(service
        .record_turn_progress(
            &claimed.claim,
            CommunicationTurnProgress::SendFailed {
                failure_code: "not-submitted".into()
            }
        )
        .await
        .unwrap());
    assert!(store
        .retry_communication_message(scope.workspace_id, scope.instance_id, ordinary)
        .await
        .unwrap());
    let retry = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    service
        .rotate_session(&scope, "new-after-failure")
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .reserve_turn_send(&retry.claim)
        .await
        .unwrap()
        .is_none());
    assert!(!store
        .retry_communication_message(scope.workspace_id, scope.instance_id, ordinary)
        .await
        .unwrap());
    let originals: i64 =
        sqlx::query_scalar("SELECT count(*) FROM communication_messages WHERE direction='inbound'")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        originals, 2,
        "exclusion and fencing never delete raw history"
    );
}

#[tokio::test]
async fn communication_pending_scan_keeps_file_only_inputs_and_command_order_barrier() {
    let (store, service, scope, command) = fixture().await;
    sqlx::query("UPDATE communication_messages SET content='/model selected-model' WHERE id=$1")
        .bind(command)
        .execute(store.pool())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(scope.workspace_id, scope.instance_id, KEY)
        .await
        .unwrap()
        .revision;
    let mut file_only = received("file-only", "alice", true);
    file_only.text = None;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(scope.workspace_id, scope.instance_id, revision),
            candidate_cursor: None,
            messages: vec![file_only],
        })
        .await
        .unwrap()
        .unwrap();
    let scan = service
        .scan_pending_admissions(scope.workspace_id, scope.instance_id, 100)
        .await
        .unwrap();
    assert_eq!(
        scan.len(),
        1,
        "scan exposes only the earliest input in each chat"
    );
    assert_eq!(scan[0].input_id, command);
    assert_eq!(scan[0].content.as_deref(), Some("/model selected-model"));
    let file_input = service.pending_admissions(&scope).await.unwrap()[1];
    assert_eq!(
        service
            .scan_pending_admissions(scope.workspace_id, scope.instance_id, 1)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(service
        .scan_pending_admissions(Uuid::now_v7(), scope.instance_id, 100)
        .await
        .unwrap()
        .is_empty());
    assert!(
        service
            .admit_input(&scope, file_input)
            .await
            .unwrap()
            .is_none(),
        "later ordinary input cannot overtake an unresolved command"
    );
    assert!(service
        .select_model(&scope, Some("selected-model"))
        .await
        .unwrap());
    // The command lane commits its own effect/reply receipt before this completion call.
    assert!(service.exclude_input(&scope, command).await.unwrap());
    let scan = service
        .scan_pending_admissions(scope.workspace_id, scope.instance_id, 100)
        .await
        .unwrap();
    assert_eq!(scan.len(), 1);
    assert_eq!(scan[0].input_id, file_input);
    assert_eq!(scan[0].content, None);
    assert!(scan[0].has_attachments);
    assert_eq!(scan[0].conversation.id, scope.conversation_id);
    let admitted = service
        .admit_input(&scope, file_input)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        admitted.turn.selected_model.as_deref(),
        Some("selected-model")
    );
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.pending.inputs[0].input_id, file_input);
    assert_eq!(claimed.pending.inputs[0].content, None);
    assert_eq!(claimed.pending.inputs[0].attachments.len(), 1);
}

#[tokio::test]
async fn communication_turn_explicit_failed_run_retry_updates_all_members_together() {
    let (store, service, scope, first) = fixture().await;
    service.admit_input(&scope, first).await.unwrap().unwrap();
    let second = receive(&store, &service, &scope, "second", false).await;
    service.admit_input(&scope, second).await.unwrap().unwrap();
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .link_turn_run(&claimed.claim, Uuid::now_v7())
        .await
        .unwrap());
    assert!(service
        .checkpoint_turn(&claimed.claim, checkpoint(0, 2))
        .await
        .unwrap());
    assert!(service
        .record_turn_progress(
            &claimed.claim,
            CommunicationTurnProgress::RunFailed {
                failure_code: "runtime-failed".into()
            }
        )
        .await
        .unwrap());
    let statuses: Vec<String> =
        sqlx::query_scalar("SELECT processing_status FROM communication_inputs ORDER BY id")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(statuses, vec!["failed", "failed"]);
    assert!(store
        .retry_communication_message(scope.workspace_id, scope.instance_id, second)
        .await
        .unwrap());
    assert!(!store
        .retry_communication_message(scope.workspace_id, scope.instance_id, first)
        .await
        .unwrap());
    let retried = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retried.claim.turn_id, claimed.claim.turn_id);
    assert_eq!(
        retried.pending.turn.run_id, None,
        "only explicit operator retry may reserve a replacement failed run"
    );
    assert_eq!(retried.pending.turn.consumed_watermark, 0);
    assert_eq!(retried.pending.turn.checkpoint_revision, 0);
    assert!(retried.pending.checkpoint.is_none());
    assert_eq!(
        retried
            .pending
            .inputs
            .iter()
            .map(|input| input.input_id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[tokio::test]
async fn communication_rotation_retains_unknown_submission_and_accepts_only_its_terminal_fact() {
    let (store, service, scope, first) = fixture().await;
    service.admit_input(&scope, first).await.unwrap().unwrap();
    let claimed = service
        .claim_pending_turn(scope.workspace_id, scope.instance_id, 60)
        .await
        .unwrap()
        .unwrap();
    assert!(service
        .link_turn_run(&claimed.claim, Uuid::now_v7())
        .await
        .unwrap());
    assert!(service
        .checkpoint_turn(&claimed.claim, checkpoint(0, 1))
        .await
        .unwrap());
    assert!(matches!(
        service
            .seal_turn(&claimed.claim, candidate(1, 1))
            .await
            .unwrap(),
        CommunicationTurnSeal::Reserved { .. }
    ));
    service
        .rotate_session(&scope, "during-provider-send")
        .await
        .unwrap()
        .unwrap();
    let send: String =
        sqlx::query_scalar("SELECT send_status FROM communication_reply_turns WHERE id=$1")
            .bind(claimed.claim.turn_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        send, "unknown",
        "rotation cannot claim an in-flight provider submission was undone"
    );
    assert!(service
        .reserve_turn_send(&claimed.claim)
        .await
        .unwrap()
        .is_none());
    assert!(!store
        .retry_communication_message(scope.workspace_id, scope.instance_id, first)
        .await
        .unwrap());
    assert!(
        service
            .record_turn_progress(&claimed.claim, CommunicationTurnProgress::SendSucceeded)
            .await
            .unwrap(),
        "a response from the already submitted send may settle its original receipt"
    );
    assert!(!service
        .record_turn_progress(&claimed.claim, CommunicationTurnProgress::SendSucceeded)
        .await
        .unwrap());
    let sent: (String, String) =
        sqlx::query_as("SELECT state,send_status FROM communication_reply_turns WHERE id=$1")
            .bind(claimed.claim.turn_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(sent, ("invalidated".into(), "succeeded".into()));
}
