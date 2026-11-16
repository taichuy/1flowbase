use super::*;
use control_plane_contracts::ports::{AppendRuntimeEventInput, OrchestrationRuntimeRepository};
use serde_json::json;
use std::io::{Read, Write};

fn invalid_identity_archive(bytes: &[u8], remove_executor: bool) -> Vec<u8> {
    let mut text = String::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_string(&mut text)
        .unwrap();
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    for line in text.lines() {
        let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
        if record["table"] == "workflow_run_identity_snapshots" {
            if remove_executor {
                record["row"]
                    .as_object_mut()
                    .unwrap()
                    .remove("executor_identity");
            } else {
                record["row"]["created_at"] = serde_json::Value::Null;
            }
        }
        writeln!(encoder, "{record}").unwrap();
    }
    encoder.finish().unwrap()
}

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
    let identity = json!({"user_id":Uuid::now_v7(),"name":"Historical creator"});
    sqlx::query("insert into workflow_run_identity_snapshots(flow_run_id,creator_identity,executor_identity) values($1,$2,'null'::jsonb)")
        .bind(run).bind(&identity).execute(&db).await.unwrap();
    sqlx::query("insert into application_run_native_projection_progress(flow_run_id,projection_version,output_sequence,result_count,source_revision,conflicting_source_keys) values($1,1,0,0,0,array['retained-conflict']) on conflict(flow_run_id) do update set conflicting_source_keys=excluded.conflicting_source_keys")
        .bind(run)
        .execute(&db)
        .await
        .unwrap();
    let bytes = capture(&repo, select("applications", true, true)).await;
    let configuration = capture(&repo, select("applications", true, false)).await;
    let configuration_preview = repo
        .preflight(reader(configuration), "key", "key")
        .await
        .unwrap();
    assert!(!configuration_preview
        .selected_tables
        .contains(&"workflow_run_identity_snapshots".into()));
    for remove_executor in [true, false] {
        let invalid = invalid_identity_archive(&bytes, remove_executor);
        let preview = repo
            .preflight(reader(invalid.clone()), "key", "key")
            .await
            .unwrap();
        assert!(
            !preview.failures.is_empty(),
            "missing JSON keys and non-JSON required nulls must fail"
        );
        assert!(repo
            .restore(reader(invalid), "key", "key", true)
            .await
            .is_err());
        let unchanged: (serde_json::Value, serde_json::Value) = sqlx::query_as("select creator_identity,executor_identity from workflow_run_identity_snapshots where flow_run_id=$1")
            .bind(run).fetch_one(&db).await.unwrap();
        assert_eq!(unchanged, (identity.clone(), serde_json::Value::Null));
    }
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
    sqlx::query("delete from application_run_native_projection_progress where flow_run_id=$1")
        .bind(run)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("delete from workflow_run_identity_snapshots where flow_run_id=$1")
        .bind(run)
        .execute(&db)
        .await
        .unwrap();
    repo.restore(reader(bytes), "key", "key", true)
        .await
        .unwrap();
    let restored: serde_json::Value = sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1").bind(event.id).fetch_one(&db).await.unwrap();
    assert_eq!(restored, event.payload);
    assert_eq!(restored["body"], body);
    let owners: (i64, i64, i64) = sqlx::query_as("select (select count(*) from runtime_native_snapshot_manifests),(select count(*) from runtime_native_snapshot_items),(select count(*) from runtime_native_snapshot_references)").fetch_one(&db).await.unwrap();
    assert_eq!(owners, (1, 3, 3));
    let conflicts: Vec<String> = sqlx::query_scalar(
        "select conflicting_source_keys from application_run_native_projection_progress where flow_run_id=$1",
    )
    .bind(run)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(conflicts, vec!["retained-conflict"]);
    let restored_identity: (serde_json::Value, serde_json::Value) = sqlx::query_as("select creator_identity,executor_identity from workflow_run_identity_snapshots where flow_run_id=$1")
        .bind(run).fetch_one(&db).await.unwrap();
    assert_eq!(restored_identity, (identity, serde_json::Value::Null));
    let executor_is_json_null: bool = sqlx::query_scalar("select executor_identity is not null and jsonb_typeof(executor_identity)='null' from workflow_run_identity_snapshots where flow_run_id=$1")
        .bind(run).fetch_one(&db).await.unwrap();
    assert!(executor_is_json_null, "JSON null must not become SQL NULL");
}
