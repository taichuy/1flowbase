use super::*;
use crate::{_tests::provider_protocol_capsule_store_tests::seeded_flow_run, PgControlPlaneStore};
use control_plane_contracts::ports::AppendRuntimeEventInput;
use serde_json::json;

fn event(flow: Uuid, event_type: &str) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
        flow_run_id: flow,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: event_type.into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: Some(Uuid::now_v7()),
        ledger_ref: Some("trace-refresh-original-ledger".into()),
        payload: json!({"exact": "original\0payload", "ordinal": 7}),
        visibility: domain::RuntimeEventVisibility::Internal,
        durability: domain::RuntimeEventDurability::Durable,
    }
}

async fn clear_queue(store: &PgControlPlaneStore, flow: Uuid) {
    sqlx::query("delete from application_run_trace_refresh_queue where flow_run_id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
}

async fn seed() -> (PgControlPlaneStore, Uuid) {
    let (pool, flow) = seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    // A newly inserted flow is still a semantic source write.
    assert_eq!(revision(&store, flow).await, Some(1));
    clear_queue(&store, flow).await;
    (store, flow)
}

async fn revision(store: &PgControlPlaneStore, flow: Uuid) -> Option<i64> {
    sqlx::query_scalar(
        "select revision from application_run_trace_refresh_queue where flow_run_id=$1",
    )
    .bind(flow)
    .fetch_optional(store.pool())
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
async fn trace_refresh_allocator_and_noop_flow_updates_do_not_enqueue() {
    let (store, flow) = seed().await;
    assert_eq!(reserve(&store, flow).await, 1);
    sqlx::query("update flow_runs set runtime_event_sequence_high_water=runtime_event_sequence_high_water, status=status where id=$1")
        .bind(flow).execute(store.pool()).await.unwrap();
    assert_eq!(revision(&store, flow).await, None);
    let restarted = PgControlPlaneStore::new(store.pool().clone());
    assert_eq!(reserve(&restarted, flow).await, 2);
    assert_eq!(revision(&store, flow).await, None);
    let high_water: i64 =
        sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
            .bind(flow)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(high_water, 2);
}

#[tokio::test]
async fn trace_refresh_ignored_single_and_batch_events_preserve_original_records() {
    let (store, flow) = seed().await;
    let inputs = [
        event(flow, "text_delta"),
        event(flow, "reasoning_delta"),
        event(flow, "tool_call_delta"),
        event(flow, "visibleXinternal_llm_tool_started"),
        event(flow, "visible_internalXllm_tool_started"),
        event(flow, "visible_internal_llmXtool_started"),
        event(flow, "visible_internal_llm_toolXstarted"),
        event(flow, "xvisible_internal_llm_tool_started"),
        event(flow, "visible_internal_llm_tool"),
    ];
    let mut written = Vec::new();
    for input in &inputs {
        written.push(store.append_runtime_event(input).await.unwrap());
        assert_eq!(revision(&store, flow).await, None);
    }
    written.extend(store.append_runtime_events(&inputs).await.unwrap());
    assert_eq!(revision(&store, flow).await, None);
    assert_eq!(
        written.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        (1..=inputs.len() as i64 * 2).collect::<Vec<_>>()
    );
    let restarted = PgControlPlaneStore::new(store.pool().clone());
    let retained = restarted.list_runtime_events(flow, 0).await.unwrap();
    assert_eq!(retained, written);
    for (row, input) in retained.iter().zip(inputs.iter().cycle()) {
        assert_eq!(row.flow_run_id, input.flow_run_id);
        assert_eq!(row.node_run_id, input.node_run_id);
        assert_eq!(row.span_id, input.span_id);
        assert_eq!(row.parent_span_id, input.parent_span_id);
        assert_eq!(row.event_type, input.event_type);
        assert_eq!(row.layer, input.layer);
        assert_eq!(row.source, input.source);
        assert_eq!(row.trust_level, input.trust_level);
        assert_eq!(row.item_id, input.item_id);
        assert_eq!(row.ledger_ref, input.ledger_ref);
        assert_eq!(row.payload, input.payload);
        assert_eq!(row.visibility, input.visibility);
        assert_eq!(row.durability, input.durability);
    }
    assert_eq!(
        restarted
            .list_runtime_events(flow, inputs.len() as i64)
            .await
            .unwrap(),
        retained[inputs.len()..]
    );
}

#[tokio::test]
async fn trace_refresh_visible_tool_states_and_provider_facts_enqueue_single_and_batch() {
    let (store, flow) = seed().await;
    let types = [
        "visible_internal_llm_tool_started",
        "visible_internal_llm_tool_waiting_callback",
        "visible_internal_llm_tool_completed",
        "visible_internal_llm_tool_failed",
        "visible_internal_llm_tool_future_state",
        "provider_output_item_done",
        "provider_protocol_integrity",
    ];
    // Output completion also writes the log task through the repository's
    // projection owner. That semantic task write contributes one revision.
    let expected_revisions = [1, 2, 3, 4, 5, 7, 8];
    for (event_type, expected_revision) in types.iter().zip(expected_revisions) {
        store
            .append_runtime_event(&event(flow, event_type))
            .await
            .unwrap();
        assert_eq!(
            revision(&store, flow).await,
            Some(expected_revision),
            "{event_type}"
        );
    }
    clear_queue(&store, flow).await;
    let inputs: Vec<_> = types
        .iter()
        .map(|event_type| event(flow, event_type))
        .collect();
    let rows = store.append_runtime_events(&inputs).await.unwrap();
    assert_eq!(revision(&store, flow).await, Some(types.len() as i64 + 1));
    assert_eq!(
        rows.iter().map(|row| row.sequence).collect::<Vec<_>>(),
        (8..=14).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn trace_refresh_combined_high_water_and_source_change_enqueues() {
    let (store, flow) = seed().await;
    sqlx::query("update flow_runs set runtime_event_sequence_high_water=50, title='semantic source changed' where id=$1")
        .bind(flow).execute(store.pool()).await.unwrap();
    assert_eq!(revision(&store, flow).await, Some(1));
    sqlx::query("update flow_runs set updated_at=updated_at+interval '1 second' where id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(revision(&store, flow).await, Some(2));
    assert_eq!(reserve(&store, flow).await, 51);
    assert_eq!(revision(&store, flow).await, Some(2));
}

#[tokio::test]
async fn trace_refresh_stale_finish_keeps_semantic_revision_and_failed_finish_retries() {
    let (store, flow) = seed().await;
    store
        .append_runtime_event(&event(flow, "visible_internal_llm_tool_started"))
        .await
        .unwrap();
    let job = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.flow_run_id, flow);
    assert_eq!(job.revision, 1);
    assert!(store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .is_none());
    store
        .append_runtime_event(&event(flow, "visible_internal_llm_tool_waiting_callback"))
        .await
        .unwrap();
    let before_finish: time::OffsetDateTime = sqlx::query_scalar("select now()")
        .fetch_one(store.pool())
        .await
        .unwrap();
    store
        .finish_application_run_trace_refresh(&job, true)
        .await
        .unwrap();
    assert_eq!(revision(&store, flow).await, Some(2));
    let released: bool = sqlx::query_scalar("select lease_until is null and attempts=1 and available_at >= $2::timestamptz+interval '250 milliseconds' from application_run_trace_refresh_queue where flow_run_id=$1")
        .bind(flow).bind(before_finish).fetch_one(store.pool()).await.unwrap();
    assert!(released);
    // Advance eligibility directly; the test does not wait on wall-clock retry delays.
    sqlx::query(
        "update application_run_trace_refresh_queue set available_at=now() where flow_run_id=$1",
    )
    .bind(flow)
    .execute(store.pool())
    .await
    .unwrap();
    let retry = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retry.revision, 2);
    let before_finish: time::OffsetDateTime = sqlx::query_scalar("select now()")
        .fetch_one(store.pool())
        .await
        .unwrap();
    store
        .finish_application_run_trace_refresh(&retry, false)
        .await
        .unwrap();
    let failed_retry: bool = sqlx::query_scalar("select revision=2 and attempts=2 and lease_until is null and available_at >= $2::timestamptz+interval '10 seconds' from application_run_trace_refresh_queue where flow_run_id=$1")
        .bind(flow).bind(before_finish).fetch_one(store.pool()).await.unwrap();
    assert!(failed_retry);
    assert!(store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .is_none());
    sqlx::query(
        "update application_run_trace_refresh_queue set available_at=now() where flow_run_id=$1",
    )
    .bind(flow)
    .execute(store.pool())
    .await
    .unwrap();
    let retry = store
        .claim_application_run_trace_refresh()
        .await
        .unwrap()
        .unwrap();
    store
        .finish_application_run_trace_refresh(&retry, true)
        .await
        .unwrap();
    assert_eq!(revision(&store, flow).await, None);
}

#[tokio::test]
async fn trace_refresh_rollback_and_deleted_parent_do_not_leak_queue_rows() {
    let (store, flow) = seed().await;
    let mut tx = store.pool().begin().await.unwrap();
    sqlx::query("update flow_runs set title='rolled back source' where id=$1")
        .bind(flow)
        .execute(&mut *tx)
        .await
        .unwrap();
    let queued: i64 = sqlx::query_scalar(
        "select revision from application_run_trace_refresh_queue where flow_run_id=$1",
    )
    .bind(flow)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(queued, 1);
    tx.rollback().await.unwrap();
    assert_eq!(revision(&store, flow).await, None);
    let invalid = AppendRuntimeEventInput {
        node_run_id: Some(Uuid::now_v7()),
        ..event(flow, "visible_internal_llm_tool_started")
    };
    assert!(store
        .append_runtime_events(&[event(flow, "visible_internal_llm_tool_started"), invalid])
        .await
        .is_err());
    assert_eq!(revision(&store, flow).await, None);
    assert!(store.list_runtime_events(flow, 0).await.unwrap().is_empty());
    assert_eq!(reserve(&store, flow).await, 1);

    let application: Uuid = sqlx::query_scalar("select application_id from flow_runs where id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let run = store
        .get_flow_run(application, flow)
        .await
        .unwrap()
        .unwrap();
    store
        .upsert_application_run_log_summary_for_flow_run(&run)
        .await
        .unwrap();
    // A SET NULL cascade on the surviving task row invokes its UPDATE trigger
    // while the referenced flow has already disappeared. Preserve that guard.
    let updated =
        sqlx::query("update application_run_log_tasks set final_output_run_id=$1 where id=$1")
            .bind(flow)
            .execute(store.pool())
            .await
            .unwrap();
    assert_eq!(updated.rows_affected(), 1);
    assert!(revision(&store, flow).await.is_some());
    sqlx::query("delete from flow_runs where id=$1")
        .bind(flow)
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(revision(&store, flow).await, None);
    let task_exists: bool =
        sqlx::query_scalar("select exists(select 1 from application_run_log_tasks where id=$1)")
            .bind(flow)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(!task_exists);
}

#[tokio::test]
async fn trace_refresh_controlled_predecessor_reproduces_allocator_invalidation() {
    let (store, flow) = seed().await;
    // seeded_flow_run owns an isolated schema: replace only that schema's function
    // with the exact predecessor, then restore the official candidate migration.
    sqlx::raw_sql(include_str!(
        "../../../migrations/20260924010000_guard_trace_refresh_for_deleted_runs.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();
    assert_eq!(reserve(&store, flow).await, 1);
    assert_eq!(
        revision(&store, flow).await,
        Some(1),
        "predecessor must violate the no-queue allocator contract"
    );
    sqlx::raw_sql(include_str!(
        "../../../migrations/20261004180000_semantic_trace_refresh_invalidation.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();
    clear_queue(&store, flow).await;
    assert_eq!(reserve(&store, flow).await, 2);
    assert_eq!(revision(&store, flow).await, None);
}
