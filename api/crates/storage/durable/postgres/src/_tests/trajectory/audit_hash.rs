use control_plane_contracts::ports::OrchestrationRuntimeRepository;
use serde_json::json;
use storage_durable_postgres::PgControlPlaneStore;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

async fn seeded_store() -> (PgControlPlaneStore, Uuid) {
    let (pool, run) = crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    (PgControlPlaneStore::new(pool), run)
}

#[tokio::test]
async fn audit_hash_unrelated_cleanup_does_not_block_new_run() {
    let (store, old_run) = seeded_store().await;
    store
        .append_audit_hash(old_run, "runtime_events", Uuid::now_v7(), json!({}))
        .await
        .unwrap();
    let new_run = Uuid::now_v7();
    sqlx::query(
        "insert into flow_runs (id, application_id, flow_id, flow_draft_id, compiled_plan_id, run_mode, status, created_by) select $1, application_id, flow_id, flow_draft_id, compiled_plan_id, run_mode, 'running', created_by from flow_runs where id = $2",
    )
    .bind(new_run)
    .bind(old_run)
    .execute(store.pool())
    .await
    .unwrap();
    // Keep the real cascading DELETE open, as a slow cleanup batch does.
    let mut cleanup = store.pool().begin().await.unwrap();
    sqlx::query("delete from flow_runs where id = $1")
        .bind(old_run)
        .execute(&mut *cleanup)
        .await
        .unwrap();
    let append = timeout(
        Duration::from_secs(1),
        store.append_audit_hash(new_run, "runtime_events", Uuid::now_v7(), json!({})),
    )
    .await;
    cleanup.rollback().await.unwrap();
    let record = append
        .expect("another run's cleanup must not block startup audit")
        .unwrap();
    assert_eq!(record.flow_run_id, new_run);
    assert_eq!(record.prev_hash, None);
}

#[tokio::test]
async fn audit_hash_concurrent_writers_form_one_complete_chain() {
    let (store, run) = seeded_store().await;
    let mut writers = tokio::task::JoinSet::new();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(12));
    for _ in 0..12 {
        let store = PgControlPlaneStore::new(store.pool().clone());
        let barrier = barrier.clone();
        writers.spawn(async move {
            let fact_id = Uuid::now_v7();
            barrier.wait().await;
            store
                .append_audit_hash(run, "runtime_events", fact_id, json!({"id": fact_id}))
                .await
                .unwrap()
        });
    }
    while let Some(writer) = writers.join_next().await {
        writer.unwrap();
    }
    let rows = sqlx::query_as::<_, (Uuid, Option<String>, String)>(
        "select fact_id, prev_hash, row_hash from runtime_audit_hashes where flow_run_id = $1 order by created_at, id",
    )
    .bind(run)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(rows.len(), 12);
    let mut previous = None;
    for (fact_id, prev_hash, row_hash) in rows {
        assert_eq!(prev_hash, previous, "the chain must never fork or reorder");
        assert_eq!(
            row_hash,
            control_plane_contracts::persistence_projection::audit_row_hash(
                previous.as_deref(),
                "runtime_events",
                fact_id,
                &json!({"id": fact_id}),
            )
        );
        previous = Some(row_hash);
    }
}

#[tokio::test]
async fn audit_hash_append_order_survives_a_newer_predecessor_timestamp() {
    let (store, run) = seeded_store().await;
    let first = store
        .append_audit_hash(run, "runtime_events", Uuid::now_v7(), json!({"n": 1}))
        .await
        .unwrap();
    // A waiting transaction's now() can precede the writer it follows.
    // A future predecessor deterministically exposes dependence on wall-clock order.
    sqlx::query(
        "update runtime_audit_hashes set created_at = now() + interval '1 hour' where id = $1",
    )
    .bind(first.id)
    .execute(store.pool())
    .await
    .unwrap();
    let second = store
        .append_audit_hash(run, "runtime_events", Uuid::now_v7(), json!({"n": 2}))
        .await
        .unwrap();
    let third = store
        .append_audit_hash(run, "runtime_events", Uuid::now_v7(), json!({"n": 3}))
        .await
        .unwrap();
    assert_eq!(second.prev_hash.as_deref(), Some(first.row_hash.as_str()));
    assert_eq!(third.prev_hash.as_deref(), Some(second.row_hash.as_str()));
    assert!(third.created_at > second.created_at);
}

async fn append_during_same_run_delete(commit_delete: bool) {
    let (store, run) = seeded_store().await;
    let first = store
        .append_audit_hash(run, "runtime_events", Uuid::now_v7(), json!({}))
        .await
        .unwrap();
    let mut cleanup = store.pool().begin().await.unwrap();
    sqlx::query("delete from flow_runs where id = $1")
        .bind(run)
        .execute(&mut *cleanup)
        .await
        .unwrap();
    let append = store.append_audit_hash(run, "runtime_events", Uuid::now_v7(), json!({}));
    tokio::pin!(append);
    assert!(timeout(Duration::from_millis(200), &mut append)
        .await
        .is_err());
    if commit_delete {
        cleanup.commit().await.unwrap();
        assert!(timeout(Duration::from_secs(2), append)
            .await
            .unwrap()
            .is_err());
        let remaining: i64 =
            sqlx::query_scalar("select count(*) from runtime_audit_hashes where flow_run_id = $1")
                .bind(run)
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(remaining, 0, "deletion must not leave orphan audit records");
    } else {
        cleanup.rollback().await.unwrap();
        let second = timeout(Duration::from_secs(2), append)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(second.prev_hash.as_deref(), Some(first.row_hash.as_str()));
    }
}

#[tokio::test]
async fn audit_hash_same_run_delete_commit_prevents_orphans() {
    append_during_same_run_delete(true).await;
}

#[tokio::test]
async fn audit_hash_same_run_delete_rollback_preserves_chain() {
    append_during_same_run_delete(false).await;
}
