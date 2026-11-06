use super::*;
use control_plane_contracts::ports::*;
use domain::event_center::EventSourceScope;
use extension_contracts::{AfterCommitFact, LifecycleFactId, LifecycleTransactionId};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Recorded(Mutex<Vec<LifecycleObservationContext>>);
impl LifecycleFactObserver for Recorded {
    fn published(&self, input: &RecordLifecycleFactInput) {
        self.0
            .lock()
            .unwrap()
            .push(input.observation_context.clone().unwrap());
    }
    fn consumed(&self, _: &LifecycleOutboxRecord, _: &str) {}
}

#[tokio::test]
async fn optional_zero_observes_only_explicit_committed_owner_and_keeps_spawned_lineage() {
    let recorded = Arc::new(Recorded::default());
    let ancestors = vec![Uuid::now_v7(), Uuid::now_v7()];
    let store = store()
        .await
        .with_lifecycle_observer(recorded.clone())
        .with_workflow_ancestor_ids(ancestors.clone());
    let workspace = Uuid::now_v7();
    let make_input = || {
        let event = Uuid::now_v7();
        let transaction = Uuid::now_v7();
        let time = OffsetDateTime::now_utc();
        let body = AfterCommitFact::new(
            LifecycleFactId::new(event.to_string()).unwrap(),
            LifecycleTransactionId::new(transaction.to_string()).unwrap(),
            (time.unix_timestamp_nanos() / 1_000_000) as i64,
            CreditNotificationFact {
                workspace_id: workspace,
                account_id: None,
                event_type: CreditNotificationEventType::CreditGranted,
                payload: serde_json::json!({"workspace_id":"malicious-other-workspace","workflow_ancestor_ids":[]}),
            },
        );
        PublishOptionalNotificationInput {
            source_event_id: event,
            transaction_id: transaction,
            workspace_id: Some(workspace),
            occurred_at: time,
            contract: OptionalNotificationContract::Credit,
            canonical_payload: serde_json::to_vec(&body).unwrap(),
        }
    };
    let mut tx = store.pool().begin().await.unwrap();
    let (_, guard) = store
        .publish_optional_notification_in_transaction(&mut tx, make_input())
        .await
        .unwrap();
    assert!(recorded.0.lock().unwrap().is_empty());
    tx.rollback().await.unwrap();
    drop(guard);
    assert!(recorded.0.lock().unwrap().is_empty());
    let input = make_input();
    let spawned_store = store.clone();
    tokio::spawn(async move {
        spawned_store
            .publish_optional_notification(&input)
            .await
            .unwrap()
    })
    .await
    .unwrap();
    let events = recorded.0.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].scope,
        EventSourceScope::Workspace {
            workspace_id: workspace
        }
    );
    assert_eq!(events[0].workflow_ancestor_ids, ancestors);
}

#[tokio::test]
async fn durable_queue_snapshot_is_read_only_scoped_and_survives_retry_and_store_recovery() {
    let recorded = Arc::new(Recorded::default());
    let store = store().await.with_lifecycle_observer(recorded.clone());
    let workspace = Uuid::now_v7();
    let other = Uuid::now_v7();
    let lineage = vec![Uuid::now_v7()];
    let mut input = RecordLifecycleFactInput {
        observation_context: Some(LifecycleObservationContext {
            scope: EventSourceScope::Workspace {
                workspace_id: workspace,
            },
            producer_id: "trusted-owner".into(),
            workflow_ancestor_ids: lineage.clone(),
        }),
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "registered.native.fact".into(),
        contract_version: "v1".into(),
        canonical_payload: br#"{"scope_id":"payload-is-not-authority"}"#.to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["native/string-subscriber"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    store.record_lifecycle_fact(&input).await.unwrap();
    assert_eq!(
        recorded.0.lock().unwrap().len(),
        1,
        "idempotent replay is not another publication"
    );
    input.event_id = Uuid::now_v7();
    input.transaction_id = Uuid::now_v7();
    input.observation_context.as_mut().unwrap().scope = EventSourceScope::Workspace {
        workspace_id: other,
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    for _ in 0..2 {
        let snapshots = store.lifecycle_source_queue_snapshots().await.unwrap();
        assert_eq!(snapshots.len(), 2);
        let source = snapshots
            .iter()
            .find(|s| {
                s.scope
                    == EventSourceScope::Workspace {
                        workspace_id: workspace,
                    }
            })
            .unwrap();
        assert_eq!(source.queues[0].buffered, Some(1));
        assert_eq!(source.queues[0].in_flight, Some(0));
        assert_eq!(source.queues[0].subscriber_id, "native/string-subscriber");
    }
    let recovered = PgControlPlaneStore::new(store.pool().clone());
    let worker = Uuid::now_v7();
    let claimed = recovered
        .claim_lifecycle_facts(worker, 10, Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(
        claimed.len(),
        2,
        "reading snapshots never takes original messages"
    );
    let original = claimed
        .iter()
        .find(|r| {
            r.observation_context.as_ref().unwrap().scope
                == EventSourceScope::Workspace {
                    workspace_id: workspace,
                }
        })
        .unwrap();
    assert_eq!(
        original
            .observation_context
            .as_ref()
            .unwrap()
            .workflow_ancestor_ids,
        lineage
    );
    let in_flight = recovered.lifecycle_source_queue_snapshots().await.unwrap();
    assert!(in_flight
        .iter()
        .all(|s| s.queues[0].buffered == Some(0) && s.queues[0].in_flight == Some(1)));
    recovered
        .retry_lifecycle_fact(
            original.event_id,
            &original.subscriber_id,
            worker,
            original.claim_id.unwrap(),
            OffsetDateTime::now_utc(),
            "original handler failed",
        )
        .await
        .unwrap();
    let retried = recovered.lifecycle_source_queue_snapshots().await.unwrap();
    let source = retried
        .iter()
        .find(|s| {
            s.scope
                == EventSourceScope::Workspace {
                    workspace_id: workspace,
                }
        })
        .unwrap();
    assert_eq!(source.queues[0].buffered, Some(1));
    assert_eq!(source.queues[0].in_flight, Some(0));
    assert_eq!(
        recorded.0.lock().unwrap().len(),
        2,
        "recovery/snapshot/retry never publish"
    );
}

#[tokio::test]
async fn historical_null_scope_backlog_is_unbound_and_never_assigned_to_a_workspace() {
    let store = store().await;
    let input = RecordLifecycleFactInput {
        observation_context: None,
        event_id: Uuid::now_v7(),
        transaction_id: Uuid::now_v7(),
        contract_id: "historical.legacy-fact".into(),
        contract_version: "v1".into(),
        canonical_payload: b"payload must not infer scope".to_vec(),
        occurred_at: OffsetDateTime::now_utc(),
        publication: publication(&["legacy-original"]),
    };
    store.record_lifecycle_fact(&input).await.unwrap();
    let facts = store.lifecycle_source_queue_snapshots().await.unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].scope, EventSourceScope::Unbound);
    assert_eq!(facts[0].queues[0].buffered, Some(1));
    assert_eq!(facts[0].queues[0].in_flight, Some(0));
    assert_eq!(facts[0].queues[0].subscriber_id, "legacy-original");
    sqlx::query("delete from lifecycle_outbox where event_id=$1")
        .bind(input.event_id)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .lifecycle_source_queue_snapshots()
        .await
        .unwrap()
        .is_empty());
}
