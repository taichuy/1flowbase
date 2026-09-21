use control_plane_contracts::ports::{
    AppendRuntimeEventInput, OrchestrationRuntimeRepository, ProviderTrajectoryRepository,
    ProviderTrajectoryView,
};
use serde_json::json;
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

#[tokio::test]
async fn trajectory_pages_are_body_free_and_selected_bodies_are_lossless_and_scoped() {
    let (pool, flow_run_id) = super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let node_run_id = Uuid::now_v7();
    sqlx::query("insert into node_runs (id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,'llm','llm','LLM','running' from flow_runs where id=$2")
        .bind(node_run_id).bind(flow_run_id).execute(store.pool()).await.unwrap();
    let payload = |sequence, body: &str| json!({"protocol":"openai", "transport":"http", "direction":"sent", "kind":"request", "body":body, "encoding":"utf8", "flow_run_id":flow_run_id, "node_run_id":node_run_id, "node_id":"llm", "invocation_id":"invocation-1", "provider_attempt_index":0, "sequence":sequence});
    let input = |event_type: &str, payload| AppendRuntimeEventInput {
        flow_run_id,
        node_run_id: Some(node_run_id),
        span_id: None,
        parent_span_id: None,
        event_type: event_type.into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload,
        visibility: domain::RuntimeEventVisibility::Internal,
        durability: domain::RuntimeEventDurability::Durable,
    };
    // A pending marker must survive crashes, but a later final marker supersedes it.
    store.append_runtime_event(&input("provider_protocol_integrity", json!({"invocation_id":"invocation-1", "provider_attempt_index":0,"observed_count":0,"persist_failed_count":0,"status":"incomplete"}))).await.unwrap();
    let original = "  {\"text\":\"a\0b\"}\n";
    let first = store
        .append_runtime_event(&input(
            "provider_protocol_observation",
            payload(1, original),
        ))
        .await
        .unwrap();
    store
        .append_runtime_event(&input(
            "provider_protocol_observation",
            payload(2, &"x".repeat(1_000_000)),
        ))
        .await
        .unwrap();
    // Existing raw history remains addressable but is not a fabricated semantic step.
    let historical = store
        .provider_trajectory_page(flow_run_id, node_run_id, None, 1)
        .await
        .unwrap();
    assert!(historical.items.is_empty());
    assert_eq!(historical.observation_count, 2);
    let semantic = |key: &str, kind: &str, end| json!({"flow_run_id":flow_run_id,"node_run_id":node_run_id,"node_id":"llm","invocation_id":"invocation-1","provider_attempt_index":0,"step_key":key,"kind":kind,"status":"recorded","raw_sequence_start":1,"raw_sequence_end":end});
    let call = store
        .append_runtime_event(&input(
            "provider_semantic_step",
            semantic("call", "model_call", 1),
        ))
        .await
        .unwrap();
    let reply = store
        .append_runtime_event(&input(
            "provider_semantic_step",
            semantic("reply", "model_reply", 1),
        ))
        .await
        .unwrap();
    // A later projection snapshot updates the same step, preserving identity and cursor.
    store
        .append_runtime_event(&input(
            "provider_semantic_step",
            semantic("reply", "model_reply", 2),
        ))
        .await
        .unwrap();
    let page = store
        .provider_trajectory_page(flow_run_id, node_run_id, None, 1)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.observation_count, 2);
    assert_eq!(page.integrity, "incomplete");
    assert!(page.items[0].metadata.get("body").is_none());
    assert_eq!(page.items[0].event_id, call.id);
    let next = store
        .provider_trajectory_page(flow_run_id, node_run_id, page.next_cursor, 1)
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].event_id, reply.id);
    assert_eq!(next.items[0].metadata["raw_sequence_end"], 2);
    assert!(next.next_cursor.is_none());
    let body = store
        .provider_trajectory_body(
            flow_run_id,
            node_run_id,
            first.id,
            None,
            1,
            ProviderTrajectoryView::Protocol,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(body.items[0].body, original);
    assert!(body.next_cursor.is_none());
    let evidence = store
        .provider_trajectory_body(
            flow_run_id,
            node_run_id,
            reply.id,
            None,
            1,
            ProviderTrajectoryView::Protocol,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(evidence.items.len(), 1);
    assert_eq!(evidence.items[0].body, original);
    assert_eq!(evidence.next_cursor, Some(1));
    let evidence_next = store
        .provider_trajectory_body(
            flow_run_id,
            node_run_id,
            reply.id,
            evidence.next_cursor,
            1,
            ProviderTrajectoryView::Protocol,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(evidence_next.items[0].body, "x".repeat(1_000_000));
    assert!(evidence_next.next_cursor.is_none());
    assert!(store
        .provider_trajectory_body(
            flow_run_id,
            Uuid::now_v7(),
            reply.id,
            None,
            1,
            ProviderTrajectoryView::Protocol
        )
        .await
        .unwrap()
        .is_none());
    assert!(store
        .provider_trajectory_body(
            Uuid::now_v7(),
            node_run_id,
            reply.id,
            None,
            1,
            ProviderTrajectoryView::Protocol
        )
        .await
        .unwrap()
        .is_none());
    assert!(store
        .provider_trajectory_body(
            flow_run_id,
            Uuid::now_v7(),
            first.id,
            None,
            1,
            ProviderTrajectoryView::Protocol
        )
        .await
        .unwrap()
        .is_none());
    assert!(store
        .provider_trajectory_body(
            Uuid::now_v7(),
            node_run_id,
            first.id,
            None,
            1,
            ProviderTrajectoryView::Protocol
        )
        .await
        .unwrap()
        .is_none());
    store.append_runtime_event(&input("provider_protocol_integrity", json!({"invocation_id":"invocation-1", "provider_attempt_index":0,"observed_count":2,"persist_failed_count":0,"status":"complete"}))).await.unwrap();
    assert_eq!(
        store
            .provider_trajectory_page(flow_run_id, node_run_id, None, 50)
            .await
            .unwrap()
            .integrity,
        "complete"
    );
    assert_eq!(
        store
            .provider_trajectory_page(flow_run_id, Uuid::now_v7(), None, 50)
            .await
            .unwrap()
            .integrity,
        "not_recorded"
    );
    store.append_runtime_event(&input("provider_protocol_integrity", json!({"invocation_id":"invocation-1", "provider_attempt_index":0,"observed_count":2,"persist_failed_count":0,"dropped_count":1,"status":"complete"}))).await.unwrap();
    assert_eq!(
        store
            .provider_trajectory_page(flow_run_id, node_run_id, None, 50)
            .await
            .unwrap()
            .integrity,
        "incomplete"
    );
}

mod native;
