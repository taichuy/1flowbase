//! Central QA fixtures: real service + migrated PostgreSQL, no fake repository.
use control_plane::{notifications::NotificationService, ports::*};
use domain::*;
use serde_json::json;
use uuid::Uuid;

const KEY: &str = "communications-fixture-master-key";
fn config() -> CommunicationBindingInput {
    CommunicationBindingInput {
        application_id: Uuid::now_v7(),
        model: None,
        reasoning_effort: None,
        file_table_id: None,
        mcp_instance_ids: vec![],
        system_prompt: "Answer briefly".into(),
    }
}
fn authorization(w: Uuid, i: Uuid, r: i64) -> SaveNotificationAuthorizationInput {
    SaveNotificationAuthorizationInput {
        workspace_id: w,
        instance_id: i,
        expected_revision: r,
        authorization: serde_json::from_value(json!({"status":"authorized"})).unwrap(),
        managed_secrets: std::collections::BTreeMap::from([(
            "token".into(),
            json!("private-secret"),
        )]),
        master_key: KEY.into(),
    }
}
fn received(key: &str, sender: &str, attachment: bool) -> CommunicationReceiveMessage {
    CommunicationReceiveMessage {
        provider_message_id: key.into(),
        conversation_id: "private-chat".into(),
        sender_id: sender.into(),
        sender_display_name: Some(sender.into()),
        received_at_ms: 42,
        text: Some("hello".into()),
        attachments: if attachment {
            vec![CommunicationReceiveAttachment {
                provider_attachment_id: "image-1".into(),
                file_name: "picture.png".into(),
                media_type: "image/png".into(),
                size_bytes: Some(1024),
                reference: json!({"download_token":"private-download-token"}),
            }]
        } else {
            vec![]
        },
    }
}
async fn create_channel(store: &storage_durable_postgres::PgControlPlaneStore, w: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    store
        .save_notification_instance(SaveNotificationInstanceInput {
            record: NotificationInstanceRecord {
                id,
                workspace_id: w,
                type_code: "fixture.communication".into(),
                installation_id: None,
                display_name: "Communication fixture".into(),
                status: NotificationInstanceStatus::Enabled,
                config_json: json!({}),
                configured_secret_fields: vec![],
                created_at: now,
                updated_at: now,
            },
            secret_config_json: json!({}),
            master_key: KEY.into(),
            create: true,
            initial_authorization: None,
        })
        .await
        .unwrap();
    id
}
#[tokio::test]
async fn communication_inbox_cursor_dedup_scope_and_leases_are_durable() {
    let (store, w, root) = super::super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    let binding = service
        .save_binding(&actor, channel, config(), "plugin-channel-tools".into())
        .await
        .unwrap();
    assert_eq!(binding.actor_user_id, root.id);
    assert_eq!(
        binding.config.mcp_instance_ids,
        vec!["plugin-channel-tools"]
    );
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    let commit = |r, messages| CommitCommunicationReceiveInput {
        authorization: authorization(w.id, channel, r),
        candidate_cursor: Some(json!("cursor-private")),
        messages,
    };
    let next = service
        .commit_receive(commit(
            revision,
            vec![
                received("one", "alice", true),
                received("two", "alice", false),
                received("other", "bob", false),
            ],
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        service.receive_cursor(w.id, channel).await.unwrap(),
        Some(json!("cursor-private"))
    );
    assert!(service
        .commit_receive(commit(revision, vec![received("stale", "alice", false)]))
        .await
        .unwrap()
        .is_none());
    service
        .commit_receive(commit(next, vec![received("one", "alice", true)]))
        .await
        .unwrap()
        .unwrap();
    let revision_now = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, revision_now),
            candidate_cursor: None,
            messages: vec![],
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        service.receive_cursor(w.id, channel).await.unwrap(),
        Some(json!("cursor-private")),
        "absent candidate does not clear committed cursor"
    );
    let scope = CommunicationHistoryScope {
        workspace_id: w.id,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    let before_bad = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    let mut bad_attachment = received("bad-media", "alice", true);
    bad_attachment.attachments[0].provider_attachment_id.clear();
    assert!(service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, before_bad),
            candidate_cursor: Some(json!("must-not-commit")),
            messages: vec![received("rolled-back", "alice", false), bad_attachment]
        })
        .await
        .is_err());
    assert_eq!(
        store
            .resolve_notification_authorization(w.id, channel, KEY)
            .await
            .unwrap()
            .revision,
        before_bad
    );
    assert_eq!(
        service.receive_cursor(w.id, channel).await.unwrap(),
        Some(json!("cursor-private"))
    );
    let conversations = service
        .scoped_conversations(&scope, None, 100)
        .await
        .unwrap();
    assert_eq!(conversations.items.len(), 1);
    let conversation = conversations.items[0].id;
    let history = service
        .scoped_message_history(&scope, conversation, None, 100)
        .await
        .unwrap();
    assert_eq!(
        history.items.len(),
        2,
        "duplicate and stale batch cannot insert input"
    );
    let bob = CommunicationHistoryScope {
        external_identity: "bob".into(),
        ..scope.clone()
    };
    assert!(service
        .scoped_message_history(&bob, conversation, None, 100)
        .await
        .unwrap()
        .items
        .is_empty());
    let files = service
        .scoped_files(&scope, conversation, None, 100)
        .await
        .unwrap();
    assert_eq!(files.items.len(), 1);
    let encoded = serde_json::to_string(&files).unwrap();
    assert!(!encoded.contains("download_token"));
    assert!(service.scoped_file(&bob, files.items[0].id).await.is_err());
    assert!(service
        .scoped_file(
            &CommunicationHistoryScope {
                instance_id: Uuid::now_v7(),
                ..scope.clone()
            },
            files.items[0].id
        )
        .await
        .is_err());
    let claim = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claim.message.external_message_key, "one");
    let parallel = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        parallel.message.external_message_key, "other",
        "another sender can run while Alice's second message waits"
    );
    assert!(
        service
            .claim_pending_input(w.id, channel, 60)
            .await
            .unwrap()
            .is_none(),
        "same conversation must not be claimed concurrently"
    );

    assert_eq!(
        claim.attachments[0].reference,
        json!({"download_token":"private-download-token"})
    );
    let claim_key = CommunicationClaim {
        workspace_id: w.id,
        instance_id: channel,
        message_id: claim.message.id,
        claim_id: claim.claim_id,
    };
    assert!(
        !service
            .record_progress(
                &claim_key,
                CommunicationProgress::RunStarted {
                    run_id: Uuid::now_v7()
                }
            )
            .await
            .unwrap(),
        "run cannot start before files are staged"
    );
    service
        .record_attached_file(
            &claim_key,
            CommunicationAttachmentUpdate {
                attachment_id: files.items[0].id,
                file_table_id: Some(Uuid::now_v7()),
                file_record_id: Some(Uuid::now_v7()),
                failure_code: None,
            },
        )
        .await
        .unwrap();
    let run_id = Uuid::now_v7();
    assert!(service
        .record_progress(&claim_key, CommunicationProgress::RunStarted { run_id })
        .await
        .unwrap());
    assert!(!service
        .record_progress(
            &claim_key,
            CommunicationProgress::RunStarted {
                run_id: Uuid::now_v7()
            }
        )
        .await
        .unwrap());
    assert!(service
        .record_progress(
            &claim_key,
            CommunicationProgress::RunSucceeded {
                final_reply: "final only".into()
            }
        )
        .await
        .unwrap());
    assert!(service
        .record_progress(&claim_key, CommunicationProgress::SendStarted)
        .await
        .unwrap());
    assert!(service
        .record_progress(
            &claim_key,
            CommunicationProgress::SendUnknown {
                failure_code: "connection_lost".into()
            }
        )
        .await
        .unwrap());
    assert!(
        !service
            .retry_message(&actor, channel, claim.message.id)
            .await
            .unwrap(),
        "unknown send must not be retried"
    );
    let later = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(later.message.external_message_key, "two");
    assert!(!service
        .renew_claim(
            &CommunicationClaim {
                claim_id: Uuid::now_v7(),
                ..claim_key
            },
            60
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn communication_attachment_failure_is_retained_and_explicitly_retryable() {
    let (store, w, root) = super::super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    service
        .save_binding(&actor, channel, config(), "files-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, revision),
            candidate_cursor: Some(json!(1)),
            messages: vec![received("file-message", "alice", true)],
        })
        .await
        .unwrap()
        .unwrap();
    let claim = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    let key = CommunicationClaim {
        workspace_id: w.id,
        instance_id: channel,
        message_id: claim.message.id,
        claim_id: claim.claim_id,
    };
    assert!(service
        .record_attached_file(
            &key,
            CommunicationAttachmentUpdate {
                attachment_id: claim.attachments[0].record.id,
                file_table_id: None,
                file_record_id: None,
                failure_code: Some("upload_failed".into())
            }
        )
        .await
        .unwrap());
    assert!(service
        .record_progress(
            &key,
            CommunicationProgress::RunFailed {
                failure_code: "upload_failed".into()
            }
        )
        .await
        .unwrap());
    assert!(service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .is_none());
    assert!(service
        .retry_message(&actor, channel, claim.message.id)
        .await
        .unwrap());
    let retried = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retried.message.id, claim.message.id);
    assert_eq!(
        retried.attachments[0].record.status,
        CommunicationAttachmentStatus::Pending
    );
    assert_ne!(retried.claim_id, claim.claim_id);
    assert!(!service
        .record_progress(
            &key,
            CommunicationProgress::RunFailed {
                failure_code: "old_worker".into()
            }
        )
        .await
        .unwrap());
}

#[tokio::test]
async fn communication_expired_claims_recover_without_repeating_unknown_sends() {
    let (store, w, root) = super::super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    service
        .save_binding(&actor, channel, config(), "lease-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, revision),
            candidate_cursor: Some(json!(1)),
            messages: vec![received("lease-message", "alice", false)],
        })
        .await
        .unwrap()
        .unwrap();
    let first = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    sqlx::query(
        "UPDATE communication_messages SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(first.message.id)
    .execute(store.pool())
    .await
    .unwrap();
    let second = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.message.id, first.message.id);
    assert_ne!(second.claim_id, first.claim_id);
    let key = CommunicationClaim {
        workspace_id: w.id,
        instance_id: channel,
        message_id: second.message.id,
        claim_id: second.claim_id,
    };
    assert!(service
        .record_progress(
            &key,
            CommunicationProgress::RunStarted {
                run_id: Uuid::now_v7()
            }
        )
        .await
        .unwrap());
    sqlx::query(
        "UPDATE communication_messages SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(first.message.id)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(
        service
            .claim_pending_input(w.id, channel, 60)
            .await
            .unwrap()
            .is_none(),
        "reserved interrupted run requires explicit retry"
    );
    assert!(service
        .retry_message(&actor, channel, first.message.id)
        .await
        .unwrap());
    let third = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    let key = CommunicationClaim {
        claim_id: third.claim_id,
        ..key
    };
    assert!(service
        .record_progress(
            &key,
            CommunicationProgress::RunStarted {
                run_id: Uuid::now_v7()
            }
        )
        .await
        .unwrap());
    assert!(service
        .record_progress(
            &key,
            CommunicationProgress::RunSucceeded {
                final_reply: "answer".into()
            }
        )
        .await
        .unwrap());
    assert!(service
        .record_progress(&key, CommunicationProgress::SendStarted)
        .await
        .unwrap());
    sqlx::query(
        "UPDATE communication_messages SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(first.message.id)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .is_none());
    let scope = CommunicationHistoryScope {
        workspace_id: w.id,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    let history = service
        .scoped_message_history(&scope, third.message.conversation_id, None, 100)
        .await
        .unwrap();
    assert_eq!(
        history.items[0].send_status,
        CommunicationSendStatus::Unknown
    );
    assert!(!service
        .retry_message(&actor, channel, first.message.id)
        .await
        .unwrap());
}

#[tokio::test]
async fn communication_can_retry_projects_status_lease_and_channel_eligibility() {
    let (store, w, root) = super::super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    service
        .save_binding(&actor, channel, config(), "retry-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, revision),
            candidate_cursor: None,
            messages: vec![received("retry-eligibility", "alice", false)],
        })
        .await
        .unwrap()
        .unwrap();
    let claim = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert!(!claim.message.can_retry);
    let scope = CommunicationHistoryScope {
        workspace_id: w.id,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    let key = CommunicationClaim {
        workspace_id: w.id,
        instance_id: channel,
        message_id: claim.message.id,
        claim_id: claim.claim_id,
    };
    assert!(service
        .record_progress(
            &key,
            CommunicationProgress::RunFailed {
                failure_code: "fixture_failure".into()
            }
        )
        .await
        .unwrap());
    assert!(
        service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
    // Model both live/expired crash claims, not just the normally released terminal lease.
    sqlx::query("UPDATE communication_messages SET claim_id=$2,claim_expires_at=now()+interval '1 hour' WHERE id=$1")
        .bind(claim.message.id).bind(claim.claim_id).execute(store.pool()).await.unwrap();
    assert!(
        !service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
    assert!(!service
        .retry_message(&actor, channel, claim.message.id)
        .await
        .unwrap());
    sqlx::query(
        "UPDATE communication_messages SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(claim.message.id)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(
        service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
    sqlx::query("UPDATE notification_instances SET status='disabled' WHERE id=$1")
        .bind(channel)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(
        !service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
    assert!(!service
        .retry_message(&actor, channel, claim.message.id)
        .await
        .unwrap());
    sqlx::query("UPDATE notification_instances SET status='enabled' WHERE id=$1")
        .bind(channel)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE communication_messages SET send_status='unknown' WHERE id=$1")
        .bind(claim.message.id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(
        !service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
    assert!(!service
        .retry_message(&actor, channel, claim.message.id)
        .await
        .unwrap());
    sqlx::query("UPDATE communication_messages SET processing_status='succeeded',send_status='failed' WHERE id=$1").bind(claim.message.id).execute(store.pool()).await.unwrap();
    assert!(
        service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
    assert!(service
        .retry_message(&actor, channel, claim.message.id)
        .await
        .unwrap());
    assert!(
        !service
            .scoped_message_history(&scope, claim.conversation.id, None, 100)
            .await
            .unwrap()
            .items[0]
            .can_retry
    );
}

#[tokio::test]
async fn communication_preflight_failure_is_durable_before_any_external_send() {
    let (store, w, root) = super::super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    service
        .save_binding(&actor, channel, config(), "plugin-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, revision),
            candidate_cursor: None,
            messages: vec![received("preflight-message", "alice", false)],
        })
        .await
        .unwrap()
        .unwrap();
    let input = service
        .claim_pending_input(w.id, channel, 120)
        .await
        .unwrap()
        .unwrap();
    let claim = CommunicationClaim {
        workspace_id: w.id,
        instance_id: channel,
        message_id: input.message.id,
        claim_id: input.claim_id,
    };
    assert!(service
        .record_progress(
            &claim,
            CommunicationProgress::RunStarted {
                run_id: Uuid::now_v7()
            }
        )
        .await
        .unwrap());
    assert!(service
        .record_progress(
            &claim,
            CommunicationProgress::RunSucceeded {
                final_reply: "complete answer".into()
            }
        )
        .await
        .unwrap());
    // Failed authority/capability checks must not mark a provider attempt as started.
    assert!(service
        .record_progress(
            &claim,
            CommunicationProgress::SendFailed {
                failure_code: "send_preflight_failed".into()
            }
        )
        .await
        .unwrap());
    let page = service
        .message_history(&actor, channel, input.conversation.id, None, 30)
        .await
        .unwrap();
    assert_eq!(page.items[0].send_status, CommunicationSendStatus::Failed);
    assert_eq!(
        page.items[0].failure_code.as_deref(),
        Some("send_preflight_failed")
    );
    assert!(page.items[0].can_retry);
}
