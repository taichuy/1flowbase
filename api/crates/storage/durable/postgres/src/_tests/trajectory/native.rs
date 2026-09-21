use super::*;

async fn append(
    store: &PgControlPlaneStore,
    flow: Uuid,
    node: Uuid,
    kind: &str,
    payload: serde_json::Value,
) -> Uuid {
    store
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id: flow,
            node_run_id: Some(node),
            span_id: None,
            parent_span_id: None,
            event_type: kind.into(),
            layer: domain::RuntimeEventLayer::RuntimeItem,
            source: domain::RuntimeEventSource::Host,
            trust_level: domain::RuntimeTrustLevel::HostFact,
            item_id: None,
            ledger_ref: None,
            payload,
            visibility: domain::RuntimeEventVisibility::Internal,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await
        .unwrap()
        .id
}

fn step(key: &str, body: &str) -> serde_json::Value {
    json!({"source":"ai_native","invocation_id":"native","provider_attempt_index":0,
        "step_key":key,"kind":"model_reply","status":"recorded","body":body})
}
fn integrity(status: &str, count: i64, failed: i64) -> serde_json::Value {
    json!({"invocation_id":"native","provider_attempt_index":0,"status":status,
        "observed_count":count,"persist_failed_count":failed,"dropped_count":0})
}
async fn setup() -> (PgControlPlaneStore, Uuid, Uuid) {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let node = Uuid::now_v7();
    sqlx::query("insert into node_runs (id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,'llm','llm','LLM','running' from flow_runs where id=$2")
        .bind(node).bind(flow).execute(store.pool()).await.unwrap();
    (store, flow, node)
}

#[tokio::test]
async fn native_latest_lossless_body_stable_cursor_and_independent_integrity() {
    let (store, flow, node) = setup().await;
    append(
        &store,
        flow,
        node,
        "native_trajectory_integrity",
        integrity("pending", 0, 0),
    )
    .await;
    let id = append(
        &store,
        flow,
        node,
        "provider_semantic_step",
        step("reply", "old"),
    )
    .await;
    let before = store
        .provider_trajectory_page(flow, node, None, 1)
        .await
        .unwrap();
    assert_eq!(before.integrity, "incomplete");
    let latest = "{\"text\":\"actual NUL \0 and escaped \\u0000\"}";
    append(
        &store,
        flow,
        node,
        "provider_semantic_step",
        step("reply", latest),
    )
    .await;
    let mut error = step("error", "{\"error\":\"provider error\"}");
    error["status"] = json!("error");
    append(&store, flow, node, "provider_semantic_step", error).await;
    append(
        &store,
        flow,
        node,
        "native_trajectory_integrity",
        integrity("complete", 3, 0),
    )
    .await;
    let page = store
        .provider_trajectory_page(flow, node, None, 1)
        .await
        .unwrap();
    assert_eq!(page.integrity, "complete");
    assert_eq!(page.protocol_integrity, "not_recorded");
    assert_eq!(page.observation_count, 0);
    assert_eq!(page.items[0].event_id, id);
    assert_eq!(page.items[0].event_sequence, before.items[0].event_sequence);
    assert_eq!(page.items[0].metadata["source"], "ai_native");
    assert!(page.items[0].metadata.get("body").is_none());
    let next = store
        .provider_trajectory_page(flow, node, page.next_cursor, 1)
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert!(next.next_cursor.is_none());
    let body = store
        .provider_trajectory_body(flow, node, id, None, 1, ProviderTrajectoryView::Semantic)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(body.items[0].body, latest);
    assert_eq!(body.source, "ai_native");
    assert_eq!(body.evidence_scope, "step");
    let empty = store
        .provider_trajectory_body(flow, node, id, None, 1, ProviderTrajectoryView::Protocol)
        .await
        .unwrap()
        .unwrap();
    assert!(empty.items.is_empty());
    assert_eq!(empty.evidence_scope, "invocation");
    let raw = |seq| {
        json!({"invocation_id":"native","provider_attempt_index":0,
        "sequence":seq,"body":format!("raw-{seq}"),"encoding":"utf8"})
    };
    let raw_id = append(&store, flow, node, "provider_protocol_observation", raw(1)).await;
    append(&store, flow, node, "provider_protocol_observation", raw(2)).await;
    append(
        &store,
        flow,
        node,
        "provider_protocol_integrity",
        integrity("incomplete", 2, 1),
    )
    .await;
    let page = store
        .provider_trajectory_page(flow, node, None, 100)
        .await
        .unwrap();
    assert_eq!(page.integrity, "complete");
    assert_eq!(page.persist_failed_count, 0);
    assert_eq!(page.protocol_integrity, "incomplete");
    assert_eq!(page.protocol_persist_failed_count, 1);
    assert_eq!(page.observation_count, 2);
    let raw_page = store
        .provider_trajectory_body(flow, node, id, None, 1, ProviderTrajectoryView::Protocol)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw_page.items[0].body, "raw-1");
    assert_eq!(raw_page.source, "supplier_protocol");
    assert_eq!(raw_page.evidence_scope, "invocation");
    let raw_next = store
        .provider_trajectory_body(
            flow,
            node,
            id,
            raw_page.next_cursor,
            1,
            ProviderTrajectoryView::Protocol,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw_next.items[0].body, "raw-2");
    assert!(raw_next.next_cursor.is_none());
    assert!(store
        .provider_trajectory_body(
            flow,
            node,
            raw_id,
            None,
            1,
            ProviderTrajectoryView::Semantic
        )
        .await
        .unwrap()
        .is_none());
    assert!(store
        .provider_trajectory_body(
            flow,
            Uuid::now_v7(),
            id,
            None,
            1,
            ProviderTrajectoryView::Protocol
        )
        .await
        .unwrap()
        .is_none());
    append(
        &store,
        flow,
        node,
        "native_trajectory_integrity",
        integrity("complete", 4, 1),
    )
    .await;
    let failed = store
        .provider_trajectory_page(flow, node, None, 10)
        .await
        .unwrap();
    assert_eq!(failed.integrity, "incomplete");
    assert_eq!(failed.persist_failed_count, 1);
}

#[tokio::test]
async fn legacy_source_mixed_attempts_and_reads_do_not_mutate_history() {
    let (store, flow, node) = setup().await;
    append(
        &store,
        flow,
        node,
        "provider_semantic_step",
        step("reply", "native details"),
    )
    .await;
    append(
        &store,
        flow,
        node,
        "native_trajectory_integrity",
        integrity("complete", 1, 0),
    )
    .await;
    let legacy = append(
        &store,
        flow,
        node,
        "provider_semantic_step",
        json!({"invocation_id":"legacy",
        "provider_attempt_index":1,"step_key":"reply","status":"recorded",
        "raw_sequence_start":1,"raw_sequence_end":1}),
    )
    .await;
    for seq in 1..=2 {
        append(&store,flow,node,"provider_protocol_observation",json!({"invocation_id":"legacy",
            "provider_attempt_index":1,"sequence":seq,"body":format!("legacy-{seq}"),"encoding":"utf8"})).await;
    }
    let before: i64 =
        sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1")
            .bind(flow)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let page = store
        .provider_trajectory_page(flow, node, None, 10)
        .await
        .unwrap();
    assert_eq!(page.integrity, "incomplete");
    assert_eq!(page.items[1].metadata["source"], "supplier_protocol");
    let body = store
        .provider_trajectory_body(
            flow,
            node,
            legacy,
            None,
            1,
            ProviderTrajectoryView::Semantic,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(body.source, "supplier_protocol");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body.items[0].body).unwrap()["source"],
        "supplier_protocol"
    );
    let raw = store
        .provider_trajectory_body(
            flow,
            node,
            legacy,
            None,
            10,
            ProviderTrajectoryView::Protocol,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(raw.evidence_scope, "step");
    assert_eq!(raw.items.len(), 1);
    assert_eq!(raw.items[0].body, "legacy-1");
    let after: i64 = sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1")
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let source: Option<String> = sqlx::query_scalar(
        "select metadata->>'source' from provider_semantic_trajectory_steps where event_id=$1",
    )
    .bind(legacy)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(source, None);
    append(
        &store,
        flow,
        node,
        "provider_protocol_integrity",
        json!({"invocation_id":"legacy","provider_attempt_index":1,
        "status":"complete","observed_count":2,"persist_failed_count":0,"dropped_count":0}),
    )
    .await;
    assert_eq!(
        store
            .provider_trajectory_page(flow, node, None, 10)
            .await
            .unwrap()
            .integrity,
        "complete"
    );
    append(
        &store,
        flow,
        node,
        "native_trajectory_integrity",
        json!({"invocation_id":"missing-terminal","provider_attempt_index":2,
        "status":"pending","observed_count":0,"persist_failed_count":0,"dropped_count":0}),
    )
    .await;
    assert_eq!(
        store
            .provider_trajectory_page(flow, node, None, 10)
            .await
            .unwrap()
            .integrity,
        "incomplete"
    );
}
