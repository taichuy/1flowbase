use super::*;
use control_plane_contracts::ports::{RuntimeEventDeliveryBatch, RuntimeEventDeliveryScope};

async fn fixture() -> (
    PgControlPlaneStore,
    RuntimeSeedState,
    domain::FlowRunRecord,
    domain::NodeRunRecord,
) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let seeded = seed_runtime_base(&store).await;
    let compiled = seed_compiled_plan(&store, &seeded).await;
    let run = seed_flow_run(&store, &seeded, &compiled, OffsetDateTime::now_utc()).await;
    let node = seed_node_run(&store, &run, OffsetDateTime::now_utc()).await;
    (store, seeded, run, node)
}

fn input(
    run: Uuid,
    scope: RuntimeEventDeliveryScope,
    limit: usize,
) -> ClaimRuntimeEventDeliveriesInput {
    ClaimRuntimeEventDeliveriesInput {
        flow_run_id: run,
        scope,
        limit,
        lease_seconds: 30,
    }
}

async fn wait(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    run: &domain::FlowRunRecord,
    node: &domain::NodeRunRecord,
    ordinal: usize,
    count: usize,
) -> control_plane_contracts::ports::PersistedWaitingState {
    let mut value = callback_wait_input(seeded, run, node, ordinal, None, None, count);
    let template = value.tool_delivery_events[0].clone();
    value.tool_delivery_events = (0..count)
        .map(|i| {
            let mut e = template.clone();
            e.payload = json!({"committed_delivery":true,"output_index":i,
            "item":{"id":format!("ctc-{ordinal}-{i}"),"type":"custom_tool_call",
                "call_id":format!("call-{ordinal}-{i}"),"name":"exec","input":"exact\nbytes",
                "opaque_extension":{"retain":[1,2,3]}}});
            e
        })
        .collect();
    store.persist_waiting_state(&value).await.unwrap().unwrap()
}

fn ids(batch: &RuntimeEventDeliveryBatch) -> Vec<Uuid> {
    batch.events.iter().map(|e| e.id).collect()
}

#[tokio::test]
async fn response_scope_precedes_limit_and_reconnect_excludes_descendants_after_failed_ack() {
    let (store, seeded, run, node) = fixture().await;
    // More historical rows than the existing host batch size, in one real waiting transaction.
    let old = wait(&store, &seeded, &run, &node, 1, 129).await;
    let predecessor = old.callback_task.as_ref().unwrap().id;
    let initial = store
        .claim_runtime_event_deliveries(&input(run.id, RuntimeEventDeliveryScope::Start, 128))
        .await
        .unwrap();
    assert_eq!(initial.claims.len(), 128);
    // Projected client output has arrived; its ACK did not persist. Leave claims
    // unacked, consume the callback through its real inbox, then expire the lease.
    let consumed = store
        .commit_tool_callback_results(&CommitToolCallbackResultsInput {
            responses_continuation: None,
            scope_id: seeded.workspace_id,
            application_id: seeded.application_id,
            flow_run_id: run.id,
            checkpoint_id: old.checkpoint.id,
            callback_task_id: predecessor,
            results: (0..129)
                .map(|i| tool_result(&format!("call-1-{i}"), json!("done")))
                .collect(),
        })
        .await
        .unwrap();
    assert_eq!(consumed.callback_task.status, CallbackTaskStatus::Completed);
    sqlx::query("update runtime_events set delivery_lease_expires_at=now()-interval '1 second' where flow_run_id=$1 and delivery_status='claimed'")
        .bind(run.id).execute(store.pool()).await.unwrap();
    let scope = RuntimeEventDeliveryScope::Resume {
        callback_task_id: predecessor,
    };
    let empty = store
        .claim_runtime_event_deliveries(&input(run.id, scope, 128))
        .await
        .unwrap();
    assert!(
        empty.claims.is_empty() && empty.events.is_empty(),
        "not yet committed upper is not permanently open"
    );
    let own = wait(&store, &seeded, &run, &node, 2, 2).await;
    let future = wait(&store, &seeded, &run, &node, 3, 2).await;
    // A delayed async waiting copy is later than both real business boundaries.
    let mut diagnostic = callback_wait_input(&seeded, &run, &node, 4, None, None, 1).waiting_event;
    diagnostic.payload = json!({"callback_task_id":predecessor});
    store.append_runtime_event(&diagnostic).await.unwrap();
    let batch = store
        .claim_runtime_event_deliveries(&input(run.id, scope, 128))
        .await
        .unwrap();
    let own_ids = own
        .tool_delivery_events
        .iter()
        .map(|e| e.id)
        .collect::<Vec<_>>();
    assert_eq!(ids(&batch), own_ids);
    assert_eq!(
        batch.claims.iter().map(|c| c.event.id).collect::<Vec<_>>(),
        own_ids
    );
    let generation_counts: Vec<(i64,i64)> = sqlx::query_as("select delivery_generation,count(*) from runtime_events where flow_run_id=$1 and sequence<$2 and delivery_status in ('pending','claimed') group by delivery_generation order by delivery_generation")
        .bind(run.id).bind(old.waiting_event.sequence).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        generation_counts,
        vec![(0, 1), (1, 128)],
        "historical rows cannot be claimed then filtered/released"
    );
    let future_generations: Vec<i64> = sqlx::query_scalar(
        "select delivery_generation from runtime_events where id=any($1) order by sequence",
    )
    .bind(
        future
            .tool_delivery_events
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(future_generations, vec![0, 0]);
    // No claim is required to prove exact same-response acked/uncertain facts.
    let first = &batch.claims[0];
    let second = &batch.claims[1];
    store
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: first.event.id,
            claim_token: first.claim_token,
            expected_generation: first.generation,
            acknowledged_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    store
        .mark_runtime_event_delivery_uncertain(&MarkRuntimeEventDeliveryUncertainInput {
            event_id: second.event.id,
            claim_token: second.claim_token,
            expected_generation: second.generation,
            marked_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let replay = store
        .claim_runtime_event_deliveries(&input(run.id, scope, 128))
        .await
        .unwrap();
    assert!(replay.claims.is_empty());
    assert_eq!(ids(&replay), own_ids);
    assert_eq!(
        replay
            .events
            .iter()
            .map(|e| e.payload.clone())
            .collect::<Vec<_>>(),
        own.tool_delivery_events
            .iter()
            .map(|e| e.payload.clone())
            .collect::<Vec<_>>()
    );
    let reconnected = store
        .claim_runtime_event_deliveries(&input(run.id, RuntimeEventDeliveryScope::Start, 128))
        .await
        .unwrap();
    assert_eq!(
        ids(&reconnected),
        old.tool_delivery_events
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    );
    assert!(reconnected
        .claims
        .iter()
        .all(|c| c.event.sequence < old.waiting_event.sequence));
}

#[tokio::test]
async fn scoped_concurrent_claims_and_stale_receipts_keep_token_generation_fencing() {
    let (store, seeded, run, node) = fixture().await;
    let old = wait(&store, &seeded, &run, &node, 1, 2).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    let own = wait(&store, &seeded, &run, &node, 2, 2).await;
    let scope = RuntimeEventDeliveryScope::Resume {
        callback_task_id: callback,
    };
    let value = input(run.id, scope, 128);
    let other_scope = input(run.id, RuntimeEventDeliveryScope::Start, 128);
    let (left, right, old_left, old_right) = tokio::join!(
        store.claim_runtime_event_deliveries(&value),
        store.claim_runtime_event_deliveries(&value),
        store.claim_runtime_event_deliveries(&other_scope),
        store.claim_runtime_event_deliveries(&other_scope)
    );
    let left = left.unwrap();
    let right = right.unwrap();
    let claims = left
        .claims
        .into_iter()
        .chain(right.claims)
        .collect::<Vec<_>>();
    let unique = claims
        .iter()
        .map(|c| c.event.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(claims.len(), 2);
    assert_eq!(unique.len(), 2);
    assert_eq!(
        unique,
        own.tool_delivery_events
            .iter()
            .map(|e| e.id)
            .collect::<std::collections::BTreeSet<_>>()
    );
    let old_claims = old_left
        .unwrap()
        .claims
        .into_iter()
        .chain(old_right.unwrap().claims)
        .collect::<Vec<_>>();
    assert_eq!(old_claims.len(), 2);
    assert_eq!(
        old_claims
            .iter()
            .map(|c| c.event.id)
            .collect::<std::collections::BTreeSet<_>>(),
        old.tool_delivery_events
            .iter()
            .map(|e| e.id)
            .collect::<std::collections::BTreeSet<_>>()
    );
    let stale = &claims[0];
    sqlx::query(
        "update runtime_events set delivery_lease_expires_at=now()-interval '1 second' where id=$1",
    )
    .bind(stale.event.id)
    .execute(store.pool())
    .await
    .unwrap();
    let reclaimed = store.claim_runtime_event_deliveries(&value).await.unwrap();
    assert_eq!(reclaimed.claims.len(), 1);
    let fresh = &reclaimed.claims[0];
    assert_eq!(fresh.event.id, stale.event.id);
    assert_eq!(fresh.generation, stale.generation + 1);
    assert_ne!(fresh.claim_token, stale.claim_token);
    assert!(store
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: stale.event.id,
            claim_token: stale.claim_token,
            expected_generation: stale.generation,
            acknowledged_at: OffsetDateTime::now_utc()
        })
        .await
        .is_err());
    assert!(store
        .release_runtime_event_delivery(&ReleaseRuntimeEventDeliveryInput {
            event_id: stale.event.id,
            claim_token: stale.claim_token,
            expected_generation: stale.generation
        })
        .await
        .is_err());
    assert!(store
        .mark_runtime_event_delivery_uncertain(&MarkRuntimeEventDeliveryUncertainInput {
            event_id: stale.event.id,
            claim_token: stale.claim_token,
            expected_generation: stale.generation,
            marked_at: OffsetDateTime::now_utc()
        })
        .await
        .is_err());
    store
        .ack_runtime_event_delivery(&AckRuntimeEventDeliveryInput {
            event_id: fresh.event.id,
            claim_token: fresh.claim_token,
            expected_generation: fresh.generation,
            acknowledged_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn original_human_stop_is_an_upper_boundary_without_any_stream_generation_marker() {
    let (store, seeded, run, node) = fixture().await;
    let old = wait(&store, &seeded, &run, &node, 1, 1).await;
    let callback = old.callback_task.as_ref().unwrap().id;
    let mut human = callback_wait_input(&seeded, &run, &node, 2, None, None, 1);
    human.kind = PersistWaitingKind::Human;
    human.tool_delivery_events.clear();
    human.recovery_idempotency_key = format!("waiting_human:{}", node.id);
    human.waiting_event.event_type = "waiting_human".into();
    let stopped = store.persist_waiting_state(&human).await.unwrap().unwrap();
    assert_eq!(
        stopped.recovery_history.state_code,
        RecoveryStateCode::WaitingHuman
    );
    let mut future = callback_wait_input(&seeded, &run, &node, 3, None, None, 1);
    future.expected_status = FlowRunStatus::WaitingHuman;
    let later = store.persist_waiting_state(&future).await.unwrap().unwrap();
    let batch = store
        .claim_runtime_event_deliveries(&input(
            run.id,
            RuntimeEventDeliveryScope::Resume {
                callback_task_id: callback,
            },
            128,
        ))
        .await
        .unwrap();
    assert!(batch.claims.is_empty() && batch.events.is_empty());
    let generation: i64 =
        sqlx::query_scalar("select delivery_generation from runtime_events where id=$1")
            .bind(later.tool_delivery_events[0].id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(generation, 0);
}
