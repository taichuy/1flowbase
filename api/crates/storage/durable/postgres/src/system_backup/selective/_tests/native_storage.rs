use super::*;
use control_plane_contracts::ports::{AppendRuntimeEventInput, OrchestrationRuntimeRepository};
use serde_json::json;

#[tokio::test]
async fn selective_application_backup_restores_exact_native_snapshot_and_owners() {
    let (db, run) = crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let body = " {\"messages\":[{\"text\":\"original\\u0000value\"},{\"n\":9007199254740993}],\"tools\":[{\"schema\":{\"nul\\u0000key\":null}}]} ";
    let event = store.append_runtime_event(&AppendRuntimeEventInput {
        flow_run_id: run,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "provider_semantic_step".into(),
        layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host,
        trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None,
        ledger_ref: None,
        payload: json!({"source":"ai_native","kind":"model_call","step_key":Uuid::now_v7(),"body":body}),
        visibility: domain::RuntimeEventVisibility::User,
        durability: domain::RuntimeEventDurability::Durable,
    }).await.unwrap();
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let bytes = capture(&repo, select("applications", true, true)).await;
    // Identical immutable rows must permit restoration, including repeated restore.
    let preview = repo
        .preflight(reader(bytes.clone()), "key", "key")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    repo.restore(reader(bytes.clone()), "key", "key", true)
        .await
        .unwrap();
    sqlx::query("delete from runtime_events where id=$1")
        .bind(event.id)
        .execute(&db)
        .await
        .unwrap();
    let emptied: (i64, i64, i64) = sqlx::query_as("select (select count(*) from runtime_native_snapshot_manifests),(select count(*) from runtime_native_snapshot_items),(select count(*) from runtime_native_snapshot_references)").fetch_one(&db).await.unwrap();
    assert_eq!(emptied, (0, 0, 0));
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    let restored: serde_json::Value = sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1").bind(event.id).fetch_one(&db).await.unwrap();
    assert_eq!(restored, event.payload);
    assert_eq!(restored["body"], body);
    let owners: (i64, i64, i64) = sqlx::query_as("select (select count(*) from runtime_native_snapshot_manifests),(select count(*) from runtime_native_snapshot_items),(select count(*) from runtime_native_snapshot_references)").fetch_one(&db).await.unwrap();
    assert_eq!(owners, (1, 3, 3));
}
