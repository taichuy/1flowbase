use super::*;
use control_plane_contracts::ports::{
    CommunicationFactContent, CommunicationLifecycleFact, CommunicationLifecycleObserver,
    CommunicationSendResultKind,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Facts(Mutex<Vec<CommunicationLifecycleFact>>);
impl CommunicationLifecycleObserver for Facts {
    fn observed(&self, fact: CommunicationLifecycleFact) {
        self.0.lock().unwrap().push(fact);
    }
}

#[tokio::test]
async fn communication_lifecycle_receive_new_one_duplicate_zero_and_rollback_zero() {
    let (store, w, root) = super::super::support::seed_store().await;
    let facts = Arc::new(Facts::default());
    let ancestor = Uuid::now_v7();
    let store = store
        .with_communication_lifecycle_observer(facts.clone())
        .with_workflow_ancestor_ids(vec![ancestor]);
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    service
        .save_binding(&actor, channel, config(), "fact-tools".into())
        .await
        .unwrap();
    let revision = store
        .resolve_notification_authorization(w.id, channel, KEY)
        .await
        .unwrap()
        .revision;
    let commit = |revision, messages| CommitCommunicationReceiveInput {
        authorization: authorization(w.id, channel, revision),
        candidate_cursor: Some(json!("private-cursor")),
        messages,
    };
    let mut message = received("provider-one", "alice", true);
    for (index, mime) in ["audio/ogg", "video/mp4"].into_iter().enumerate() {
        let mut attachment = CommunicationReceiveAttachment {
            provider_attachment_id: String::new(),
            file_name: "media.bin".into(),
            media_type: String::new(),
            size_bytes: Some(100),
            reference: json!({"download_token":"private-download-token"}),
        };
        attachment.provider_attachment_id = format!("media-{index}");
        attachment.media_type = mime.into();
        message.attachments.push(attachment);
    }
    // The observer runs synchronously before commit_receive returns, after the durable cursor.
    let next = service
        .commit_receive(commit(revision, vec![message]))
        .await
        .unwrap()
        .unwrap();
    let fact = {
        let recorded = facts.0.lock().unwrap();
        assert_eq!(recorded.len(), 1);
        let CommunicationLifecycleFact::Received(fact) = &recorded[0] else {
            panic!("receive fact")
        };
        fact.clone()
    };
    assert_eq!(fact.workspace_id, w.id);
    assert_eq!(fact.instance_id, channel);
    assert_eq!(fact.provider_message_id, "provider-one");
    assert_eq!(fact.workflow_ancestor_ids, vec![ancestor]);
    assert_eq!(fact.contents.len(), 4);
    assert!(
        matches!(&fact.contents[0], CommunicationFactContent::Text { message_id: Some(id), content } if *id == fact.input_id && content == "hello")
    );
    let mime = fact
        .contents
        .iter()
        .filter_map(|content| match content {
            CommunicationFactContent::File { media_type, .. } => Some(media_type.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(mime, ["image/png", "audio/ogg", "video/mp4"]);
    let encoded = serde_json::to_string(&fact).unwrap();
    for private in [
        "private-secret",
        "download_token",
        "private-download-token",
        "private-cursor",
        "reference",
        "master_key",
    ] {
        assert!(!encoded.contains(private), "{private}");
    }
    let committed: i64 = sqlx::query_scalar("SELECT count(*) FROM communication_messages WHERE scope_id=$1 AND instance_id=$2 AND input_id=$3").bind(w.id).bind(channel).bind(fact.input_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(committed, 4);
    assert_eq!(
        service.receive_cursor(w.id, channel).await.unwrap(),
        Some(json!("private-cursor"))
    );
    assert!(service
        .commit_receive(commit(revision, vec![received("stale", "alice", false)]))
        .await
        .unwrap()
        .is_none());
    let next = service
        .commit_receive(commit(next, vec![received("provider-one", "alice", true)]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        facts.0.lock().unwrap().len(),
        1,
        "duplicate/stale batches publish zero"
    );
    let mut bad = received("bad", "alice", true);
    bad.attachments[0].provider_attachment_id.clear();
    assert!(service
        .commit_receive(commit(
            next,
            vec![received("rolled-back", "alice", false), bad]
        ))
        .await
        .is_err());
    assert_eq!(
        facts.0.lock().unwrap().len(),
        1,
        "a later invalid attachment rolls back all observations"
    );
    assert_eq!(
        store
            .resolve_notification_authorization(w.id, channel, KEY)
            .await
            .unwrap()
            .revision,
        next
    );
}

#[tokio::test]
async fn communication_lifecycle_legacy_interrupted_claim_emits_unknown_once() {
    let (store, w, root) = super::super::support::seed_store().await;
    let facts = Arc::new(Facts::default());
    let store = store.with_communication_lifecycle_observer(facts.clone());
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let channel = create_channel(&store, w.id).await;
    service
        .save_binding(&actor, channel, config(), "fact-tools".into())
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
            messages: vec![received("reply-anchor", "alice", false)],
        })
        .await
        .unwrap()
        .unwrap();
    facts.0.lock().unwrap().clear();
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
    for progress in [
        CommunicationProgress::RunStarted {
            run_id: Uuid::now_v7(),
        },
        CommunicationProgress::RunSucceeded {
            final_reply: "real reply".into(),
        },
        CommunicationProgress::SendStarted,
    ] {
        assert!(service.record_progress(&key, progress).await.unwrap());
    }
    sqlx::query(
        "UPDATE communication_inputs SET claim_expires_at=now()-interval '1 second' WHERE id=$1",
    )
    .bind(key.message_id)
    .execute(store.pool())
    .await
    .unwrap();
    assert!(service
        .claim_pending_input(Uuid::now_v7(), channel, 60)
        .await
        .unwrap()
        .is_none());
    assert!(
        facts.0.lock().unwrap().is_empty(),
        "another workspace cannot recover or observe this channel"
    );
    for _ in 0..2 {
        assert!(service
            .claim_pending_input(w.id, channel, 60)
            .await
            .unwrap()
            .is_none());
    }
    let recorded = facts.0.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let CommunicationLifecycleFact::SendResult(fact) = &recorded[0] else {
        panic!("send result")
    };
    assert_eq!(fact.result_kind, CommunicationSendResultKind::Unknown);
    assert_eq!(fact.failure_code.as_deref(), Some("send_interrupted"));
    assert_eq!(fact.input_id, Some(key.message_id));
    assert_eq!(fact.workspace_id, w.id);
    assert_eq!(
        fact.target.as_ref().unwrap().provider_message_id,
        "reply-anchor"
    );
    assert!(
        matches!(&fact.content, Some(CommunicationFactContent::Text { content, .. }) if content == "real reply")
    );
    drop(recorded);
    assert!(!service
        .retry_message(&actor, channel, key.message_id)
        .await
        .unwrap());
}
