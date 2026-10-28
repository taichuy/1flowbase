//! Central QA fixtures: real service + migrated PostgreSQL, no fake repository.
use control_plane::{notifications::NotificationService, ports::*};
use domain::*;
use serde_json::json;
use uuid::Uuid;

pub(super) const KEY: &str = "communications-fixture-master-key";
pub(super) fn config() -> CommunicationBindingInput {
    CommunicationBindingInput {
        command_settings: vec![],
        application_id: Uuid::now_v7(),
        model: None,
        reasoning_effort: None,
        file_table_id: None,
        mcp_instance_ids: vec![],
        system_prompt: "Answer briefly".into(),
    }
}
pub(super) fn authorization(w: Uuid, i: Uuid, r: i64) -> SaveNotificationAuthorizationInput {
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
pub(super) fn received(key: &str, sender: &str, attachment: bool) -> CommunicationReceiveMessage {
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
pub(super) async fn create_channel(
    store: &storage_durable_postgres::PgControlPlaneStore,
    w: Uuid,
) -> Uuid {
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
    let (store, w, root) = super::support::seed_store().await;
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
        3,
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
    let (store, w, root) = super::support::seed_store().await;
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
    let (store, w, root) = super::support::seed_store().await;
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
        "UPDATE communication_inputs SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
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
    let reserved_run_id = Uuid::now_v7();
    assert!(service
        .record_progress(
            &key,
            CommunicationProgress::RunStarted {
                run_id: reserved_run_id
            }
        )
        .await
        .unwrap());
    sqlx::query(
        "UPDATE communication_inputs SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(first.message.id)
    .execute(store.pool())
    .await
    .unwrap();
    let third = service
        .claim_pending_input(w.id, channel, 60)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(third.message.run_id, Some(reserved_run_id));
    let (restored_scope, restored_binding) = store
        .communication_context_for_run(w.id, reserved_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored_scope.instance_id, channel);
    assert_eq!(restored_scope.external_identity, "alice");
    assert_eq!(restored_binding.actor_user_id, root.id);
    assert!(store
        .communication_context_for_run(Uuid::now_v7(), reserved_run_id)
        .await
        .unwrap()
        .is_none());

    assert!(!third.message.can_retry);
    let key = CommunicationClaim {
        claim_id: third.claim_id,
        ..key
    };
    assert!(!service
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
        "UPDATE communication_inputs SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
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
        Some(CommunicationSendStatus::Unknown)
    );
    assert!(!service
        .retry_message(&actor, channel, first.message.id)
        .await
        .unwrap());
}

#[tokio::test]
async fn communication_can_retry_projects_status_lease_and_channel_eligibility() {
    let (store, w, root) = super::support::seed_store().await;
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
    sqlx::query("UPDATE communication_inputs SET claim_id=$2,claim_expires_at=now()+interval '1 hour' WHERE id=$1")
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
        "UPDATE communication_inputs SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
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
    sqlx::query("UPDATE communication_inputs SET send_status='unknown' WHERE id=$1")
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
    sqlx::query("UPDATE communication_inputs SET processing_status='succeeded',send_status='failed' WHERE id=$1").bind(claim.message.id).execute(store.pool()).await.unwrap();
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
    let (store, w, root) = super::support::seed_store().await;
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
    assert_eq!(
        page.items[0].send_status,
        Some(CommunicationSendStatus::Failed)
    );
    assert_eq!(
        page.items[0].failure_code.as_deref(),
        Some("send_preflight_failed")
    );
    assert!(page.items[0].can_retry);
}

#[tokio::test]
async fn communication_physical_scope_enforces_owner_chain_and_public_workspace_roundtrip() {
    let (store, w, root) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    let binding = service
        .save_binding(&actor, channel, config(), "scope-tools".into())
        .await
        .unwrap();
    assert_eq!(binding.workspace_id, w.id);
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    service
        .commit_receive(CommitCommunicationReceiveInput {
            authorization: authorization(w.id, channel, revision),
            candidate_cursor: Some(json!("scope-cursor")),
            messages: vec![received("scope-message", "alice", true)],
        })
        .await
        .unwrap()
        .unwrap();
    let conversations = service
        .conversations(&actor, channel, None, 100)
        .await
        .unwrap();
    assert_eq!(conversations.items[0].workspace_id, w.id);
    let conversation = conversations.items[0].id;
    let message = service
        .message_history(&actor, channel, conversation, None, 100)
        .await
        .unwrap()
        .items[0]
        .id;
    let attachment = service
        .files(&actor, channel, conversation, None, 100)
        .await
        .unwrap()
        .items[0]
        .id;
    for (table, id_column, id) in [
        ("communication_bindings", "instance_id", channel),
        ("communication_conversations", "id", conversation),
        ("communication_messages", "id", message),
        ("communication_messages", "id", attachment),
    ] {
        // Closed fixture table/column set; all identifiers are static values above.
        let sql = format!("SELECT scope_id FROM {table} WHERE {id_column}=$1");
        assert_eq!(
            sqlx::query_scalar::<_, Uuid>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(id)
                .fetch_one(store.pool())
                .await
                .unwrap(),
            w.id
        );
        let sql = format!(
            "UPDATE {table} SET updated_at='1970-01-01' WHERE {id_column}=$1 RETURNING updated_at>=created_at"
        );
        assert!(
            sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(sql.as_str()))
                .bind(id)
                .fetch_one(store.pool())
                .await
                .unwrap(),
            "{table} must maintain update time on every write"
        );
    }
    let old_alias_count: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name=ANY($1) AND column_name='workspace_id'")
        .bind(vec!["communication_bindings","communication_conversations","communication_inputs","communication_messages"]).fetch_one(store.pool()).await.unwrap();
    assert_eq!(
        old_alias_count, 0,
        "workspace_id is public DTO semantics, not a second physical scope alias"
    );
    let other_scope = Uuid::now_v7();
    assert!(
        sqlx::query("UPDATE communication_bindings SET scope_id=$2 WHERE instance_id=$1")
            .bind(channel)
            .bind(other_scope)
            .execute(store.pool())
            .await
            .is_err()
    );
    let foreign_keys:i64=sqlx::query_scalar("SELECT count(*) FROM pg_constraint WHERE conrelid IN ('communication_inputs'::regclass,'communication_messages'::regclass,'communication_conversations'::regclass) AND contype='f'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(foreign_keys, 0);
    sqlx::query("DELETE FROM communication_conversations WHERE id=$1")
        .bind(conversation)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        service
            .message_history(&actor, channel, conversation, None, 5)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    assert!(store
        .communication_binding(other_scope, channel)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .communication_conversations(other_scope, channel, None, None, 100)
        .await
        .unwrap()
        .items
        .is_empty());
    // Configuration is channel-owned; received history deliberately outlives channel deletion.
    assert!(store
        .delete_notification_instance(w.id, channel)
        .await
        .unwrap());
    assert!(store
        .communication_binding(w.id, channel)
        .await
        .unwrap()
        .is_none());
    let scope = CommunicationHistoryScope {
        workspace_id: w.id,
        instance_id: channel,
        external_identity: "alice".into(),
    };
    assert_eq!(
        service
            .scoped_message_history(&scope, conversation, None, 100)
            .await
            .unwrap()
            .items[0]
            .id,
        message
    );
    assert_eq!(
        service.scoped_file(&scope, attachment).await.unwrap().id,
        attachment
    );
}
