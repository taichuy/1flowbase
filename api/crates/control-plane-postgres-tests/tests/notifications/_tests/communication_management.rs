use super::communications::{authorization, config, create_channel, received, KEY};
use control_plane::{notifications::NotificationService, ports::*};
use domain::*;
use uuid::Uuid;

#[tokio::test]
async fn communication_management_pages_workspace_history_without_channel_preselection() {
    let (store, w, root) = super::support::seed_store().await;
    let actor = store
        .load_actor_context(root.id, w.tenant_id, w.id, None)
        .await
        .unwrap();
    let service = NotificationService::new(store.clone(), KEY.into());
    let first = create_channel(&store, w.id).await;
    let second = create_channel(&store, w.id).await;
    for (channel, base) in [(first, 100), (second, 200)] {
        service
            .save_binding(&actor, channel, config(), format!("tools-{channel}"))
            .await
            .unwrap();
        let revision = store
            .resolve_notification_authorization(w.id, channel, KEY)
            .await
            .unwrap()
            .revision;
        let mut older = received("older", "alice", true);
        older.received_at_ms = base;
        let mut newer = received("newer", "alice", true);
        newer.received_at_ms = base + 1;
        newer.text = Some("测".repeat(200));
        service
            .commit_receive(CommitCommunicationReceiveInput {
                authorization: authorization(w.id, channel, revision),
                candidate_cursor: None,
                messages: vec![older, newer],
            })
            .await
            .unwrap();
    }
    let defaults = CommunicationManagementQuery::default();
    let all = service
        .management_messages(&actor, &defaults)
        .await
        .unwrap();
    assert_eq!(
        (all.total, all.page, all.page_size, all.items.len()),
        (4, 1, 20, 4)
    );
    assert_eq!(
        all.items
            .iter()
            .map(|m| m.received_at_ms)
            .collect::<Vec<_>>(),
        vec![201, 200, 101, 100]
    );
    assert_eq!(
        all.items[0]
            .content_preview
            .as_ref()
            .unwrap()
            .chars()
            .count(),
        160
    );
    let summary = serde_json::to_value(&all.items[0]).unwrap();
    assert!(summary.get("content").is_none());
    assert!(summary.get("final_reply").is_none());
    let paged = service
        .management_messages(
            &actor,
            &CommunicationManagementQuery {
                page: Some(2),
                page_size: Some(1),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!((paged.total, paged.items[0].id), (4, all.items[1].id));
    let empty = service
        .management_messages(
            &actor,
            &CommunicationManagementQuery {
                page: Some(9),
                page_size: Some(1),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(empty.total, 4);
    assert!(empty.items.is_empty());
    let clamped = service
        .management_messages(
            &actor,
            &CommunicationManagementQuery {
                page: Some(0),
                page_size: Some(1000),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!((clamped.page, clamped.page_size), (1, 100));
    let filtered = service
        .management_messages(
            &actor,
            &CommunicationManagementQuery {
                instance_id: Some(first),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(filtered.total, 2);
    assert!(filtered.items.iter().all(|m| m.instance_id == first));
    let newest = &all.items[0];
    sqlx::query("UPDATE communication_messages SET processing_status='failed',failure_code='fixture_failed' WHERE id=$1").bind(newest.id).execute(store.pool()).await.unwrap();
    let failed = service
        .management_messages(
            &actor,
            &CommunicationManagementQuery {
                processing_status: Some(CommunicationProcessingStatus::Failed),
                send_status: Some(CommunicationSendStatus::None),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (failed.total, failed.items[0].id, failed.items[0].can_retry),
        (1, newest.id, true)
    );
    let conversations = service
        .management_conversations(&actor, &defaults)
        .await
        .unwrap();
    assert_eq!(conversations.total, 2);
    assert_eq!(conversations.items[0].conversation.instance_id, second);
    assert_eq!(
        conversations.items[0]
            .latest_message
            .as_ref()
            .unwrap()
            .processing_status,
        CommunicationProcessingStatus::Failed
    );
    let conversations_page = service
        .management_conversations(
            &actor,
            &CommunicationManagementQuery {
                page: Some(2),
                page_size: Some(1),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(conversations_page.total, 2);
    assert_eq!(conversations_page.items[0].conversation.instance_id, first);
    let conversation = service
        .management_conversations(
            &actor,
            &CommunicationManagementQuery {
                conversation_id: Some(newest.conversation_id),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(conversation.total, 1);
    let by_conversation = service
        .management_messages(
            &actor,
            &CommunicationManagementQuery {
                conversation_id: Some(newest.conversation_id),
                ..defaults.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(by_conversation.total, 2);
    // Exact detail must work independently of the current list page and only include this message's files.
    let detail = service
        .management_message(&actor, all.items[3].id)
        .await
        .unwrap();
    assert_eq!(detail.message.content.as_deref(), Some("hello"));
    assert_eq!(detail.attachments.len(), 1);
    assert!(detail
        .attachments
        .iter()
        .all(|a| a.message_id == detail.message.id));
    let wire = serde_json::to_value(detail).unwrap().to_string();
    assert!(!wire.contains("private-download-token"));
    assert!(!wire.contains("encrypted_reference"));
    assert!(service
        .management_message(&actor, Uuid::now_v7())
        .await
        .is_err());
    let mut other_workspace = actor.clone();
    other_workspace.current_workspace_id = Uuid::now_v7();
    assert_eq!(
        service
            .management_messages(&other_workspace, &defaults)
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        service
            .management_conversations(&other_workspace, &defaults)
            .await
            .unwrap()
            .total,
        0
    );
    assert!(service
        .management_message(&other_workspace, newest.id)
        .await
        .is_err());
    let mut unentitled = actor.clone();
    unentitled.is_root = false;
    unentitled.user_id = Uuid::now_v7();
    assert!(service
        .management_messages(&unentitled, &defaults)
        .await
        .is_err());
    assert!(service
        .management_conversations(&unentitled, &defaults)
        .await
        .is_err());
    assert!(service
        .management_message(&unentitled, newest.id)
        .await
        .is_err());
    service.delete_instance(&actor, second).await.unwrap();
    let retained = service.management_message(&actor, newest.id).await.unwrap();
    assert!(retained.instance_display_name.is_none());
    assert!(!retained.message.can_retry);
    assert_eq!(
        service
            .management_messages(&actor, &defaults)
            .await
            .unwrap()
            .total,
        4
    );
    assert_eq!(
        service
            .management_conversations(&actor, &defaults)
            .await
            .unwrap()
            .total,
        2
    );
}
