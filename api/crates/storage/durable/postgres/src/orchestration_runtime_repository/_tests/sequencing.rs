use super::*;
use crate::{_tests::provider_protocol_capsule_store_tests::seeded_flow_run, PgControlPlaneStore};
use control_plane_contracts::ports::AppendRuntimeEventInput;
use serde_json::json;

fn event(flow: Uuid) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
        flow_run_id: flow,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "sequence_test".into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: json!({"exact": "body\0value"}),
        visibility: domain::RuntimeEventVisibility::Internal,
        durability: domain::RuntimeEventDurability::Durable,
    }
}

fn model_call(flow: Uuid, node: Uuid, count: usize) -> AppendRuntimeEventInput {
    let mut input = event(flow);
    let key = Uuid::now_v7();
    let messages: Vec<_> = (0..count)
        .map(|index| json!({"role":"tool","tool_call_id":index.to_string(),"content":"same bytes"}))
        .collect();
    let entries: Vec<_> = (0..count)
        .map(|index| {
            json!({"event_id":Uuid::now_v7(),"metadata":{
            "step_key":format!("submitted:{index}"),"kind":"tool_result","status":"recorded",
            "direction":"prepared","tool_call_id":index.to_string(),
            "body_ref":{"step_key":key,"pointer":format!("/messages/{index}")}}})
        })
        .collect();
    input.node_run_id = Some(node);
    input.event_type = "provider_semantic_step".into();
    input.payload = json!({"source":"ai_native","invocation_id":Uuid::now_v7(),
        "provider_attempt_index":0,"step_key":key,"kind":"model_call","status":"recorded",
        "direction":"prepared","body":json!({"messages":messages}).to_string(),
        "_context_occurrences":{"version":1,"entries":entries}});
    input
}

async fn seed() -> (PgControlPlaneStore, Uuid, Uuid) {
    let (pool, flow) = seeded_flow_run().await;
    let node = Uuid::now_v7();
    sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,'llm','llm','LLM','running' from flow_runs where id=$2")
        .bind(node).bind(flow).execute(&pool).await.unwrap();
    (PgControlPlaneStore::new(pool), flow, node)
}

async fn high_water(store: &PgControlPlaneStore, flow: Uuid) -> i64 {
    sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn reserve(store: &PgControlPlaneStore, flow: Uuid) -> i64 {
    let mut tx = store.pool().begin().await.unwrap();
    lock_flow_run_event_sequence(&mut tx, flow).await.unwrap();
    let sequence = next_runtime_event_sequence(&mut tx, flow).await.unwrap();
    tx.commit().await.unwrap();
    sequence
}

#[tokio::test]
async fn batch_reservations_and_counter_only_writes_survive_repository_restart() {
    let (store, flow, node) = seed().await;
    let rows = store
        .append_runtime_events(&[
            event(flow),
            model_call(flow, node, 3),
            event(flow),
            model_call(flow, node, 4),
        ])
        .await
        .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        [1, 2, 6, 7]
    );
    assert_eq!(high_water(&store, flow).await, 11);
    // Client directories consume this same allocator without inserting a journal row.
    assert_eq!(reserve(&store, flow).await, 12);
    assert_eq!(reserve(&store, flow).await, 13);
    let restarted = PgControlPlaneStore::new(store.pool().clone());
    let next = restarted.append_runtime_event(&event(flow)).await.unwrap();
    assert_eq!(next.sequence, 14);
    assert_eq!(next.payload, event(flow).payload);
}

#[tokio::test]
async fn indexed_tail_fallback_preserves_reserved_slots_after_stale_high_water() {
    let (store, flow, node) = seed().await;
    let root = store
        .append_runtime_event(&model_call(flow, node, 4))
        .await
        .unwrap();
    assert_eq!(root.sequence, 1);
    assert_eq!(high_water(&store, flow).await, 5);
    // Defensive compatibility with a restored stale counter: the last physical row
    // must include its virtual child range, rather than using max(sequence) alone.
    sqlx::query("update flow_runs set runtime_event_sequence_high_water=0 where id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    let next = store.append_runtime_event(&event(flow)).await.unwrap();
    assert_eq!(next.sequence, 6);
    assert_eq!(high_water(&store, flow).await, 6);
}

#[tokio::test]
async fn indexed_tail_fallback_repairs_legacy_import_tail_without_changing_identity() {
    let (store, flow, _) = seed().await;
    let first = store.append_runtime_event(&event(flow)).await.unwrap();
    // Historical/imported sequence updates do not invoke the insert trigger.
    sqlx::query("update runtime_events set sequence=9000 where id=$1")
        .bind(first.id)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(high_water(&store, flow).await, 1);
    let next = store.append_runtime_event(&event(flow)).await.unwrap();
    assert_eq!(next.sequence, 9001);
    let retained: (Uuid, i64) =
        sqlx::query_as("select id,sequence from runtime_events where id=$1")
            .bind(first.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(retained, (first.id, 9000));
}

#[tokio::test]
async fn rolled_back_and_failed_appends_do_not_commit_a_reservation() {
    let (store, flow, _) = seed().await;
    let first = store.append_runtime_event(&event(flow)).await.unwrap();
    let mut tx = store.pool().begin().await.unwrap();
    lock_flow_run_event_sequence(&mut tx, flow).await.unwrap();
    assert_eq!(
        next_runtime_event_sequence(&mut tx, flow).await.unwrap(),
        first.sequence + 1
    );
    tx.rollback().await.unwrap();
    assert_eq!(high_water(&store, flow).await, first.sequence);
    let mut invalid = event(flow);
    invalid.node_run_id = Some(Uuid::now_v7());
    assert!(store.append_runtime_event(&invalid).await.is_err());
    let retry = PgControlPlaneStore::new(store.pool().clone())
        .append_runtime_event(&event(flow))
        .await
        .unwrap();
    assert_eq!(retry.sequence, first.sequence + 1);
    assert_eq!(high_water(&store, flow).await, retry.sequence);
    let count: i64 = sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn sequence_overflow_is_an_error_and_preserves_committed_rows() {
    let (store, flow, _) = seed().await;
    let first = store.append_runtime_event(&event(flow)).await.unwrap();
    sqlx::query("update flow_runs set runtime_event_sequence_high_water=$2 where id=$1")
        .bind(flow)
        .bind(i64::MAX)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store.append_runtime_event(&event(flow)).await.is_err());
    assert_eq!(high_water(&store, flow).await, i64::MAX);
    let rows: Vec<(Uuid, i64)> =
        sqlx::query_as("select id,sequence from runtime_events where flow_run_id=$1")
            .bind(flow)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(rows, [(first.id, first.sequence)]);
}

#[tokio::test]
async fn concurrent_batches_and_direct_reservations_have_disjoint_monotonic_ranges() {
    let (store, flow, node) = seed().await;
    let mut workers = tokio::task::JoinSet::new();
    for index in 0..32 {
        let store = PgControlPlaneStore::new(store.pool().clone());
        workers.spawn(async move {
            if index % 2 == 0 {
                vec![reserve(&store, flow).await]
            } else {
                let rows = store
                    .append_runtime_events(&[event(flow), model_call(flow, node, 3), event(flow)])
                    .await
                    .unwrap();
                let first = rows[0].sequence;
                assert_eq!(
                    rows.iter().map(|row| row.sequence).collect::<Vec<_>>(),
                    [first, first + 1, first + 5]
                );
                (first..=first + 5).collect()
            }
        });
    }
    let mut reserved = Vec::new();
    while let Some(worker) = workers.join_next().await {
        reserved.extend(worker.unwrap());
    }
    reserved.sort_unstable();
    assert_eq!(reserved, (1..=112).collect::<Vec<_>>());
    assert_eq!(high_water(&store, flow).await, 112);
    let final_event = store.append_runtime_event(&event(flow)).await.unwrap();
    assert_eq!(final_event.sequence, 113);
}

#[tokio::test]
async fn atomic_range_reservation_repairs_reserved_tail_and_rolls_back_without_gaps() {
    let (store, flow, node) = seed().await;
    let root = store
        .append_runtime_event(&model_call(flow, node, 4))
        .await
        .unwrap();
    sqlx::query("update flow_runs set runtime_event_sequence_high_water=0 where id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    let mut tx = store.pool().begin().await.unwrap();
    lock_flow_run_event_sequence(&mut tx, flow).await.unwrap();
    assert_eq!(
        reserve_runtime_event_sequences(&mut tx, flow, 4)
            .await
            .unwrap(),
        root.sequence + 5
    );
    tx.rollback().await.unwrap();
    assert_eq!(high_water(&store, flow).await, 0);
    let mut tx = store.pool().begin().await.unwrap();
    lock_flow_run_event_sequence(&mut tx, flow).await.unwrap();
    assert_eq!(
        reserve_runtime_event_sequences(&mut tx, flow, 4)
            .await
            .unwrap(),
        root.sequence + 5
    );
    tx.commit().await.unwrap();
    assert_eq!(reserve(&store, flow).await, root.sequence + 9);
}
