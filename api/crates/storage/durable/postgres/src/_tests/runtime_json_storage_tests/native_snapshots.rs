use super::*;
use sha2::{Digest, Sha256};

pub(super) fn event(run: Uuid, body: &str) -> AppendRuntimeEventInput {
    AppendRuntimeEventInput {
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
    }
}

#[tokio::test]
async fn native_snapshot_strings_share_items_preserve_occurrences_and_reclaim_last_owner() {
    let (store, run, _, _) = seed().await;
    let first=" {\"native_request\":{\"wire_body\":{\"input\":[{\"text\":\"a\\u0000b\",\"n\":9007199254740993},{\"text\":\"a\\\\u0000b\"}],\"tools\":[{\"schema\":{\"nul\\u0000key\":null}}]}},\"messages\":[]} ";
    let second = first.replace(
        "\"messages\":[]",
        "\"messages\":[{\"role\":\"assistant\",\"content\":\"new\"}]",
    );
    let a = store
        .append_runtime_event(&event(run, first))
        .await
        .unwrap();
    let batch = store
        .append_runtime_events(&[event(run, first), event(run, &second)])
        .await
        .unwrap();
    assert_eq!(a.payload["body"], first);
    assert_eq!(batch[0].payload["body"], first);
    assert_eq!(batch[1].payload["body"], second);
    assert!(a.sequence < batch[0].sequence && batch[0].sequence < batch[1].sequence);
    let (manifests,items,canonical):(i64,i64,i64)=sqlx::query_as("select (select count(*) from runtime_native_snapshot_manifests),(select count(*) from runtime_native_snapshot_items),(select count(*) from runtime_canonical_contents)")
        .fetch_one(store.pool()).await.unwrap();
    assert_eq!((manifests, items, canonical), (2, 4, 0));
    let id: Uuid =
        sqlx::query_scalar("select observation_body_manifest_id from runtime_events where id=$1")
            .bind(a.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        sqlx::query_scalar::<_, Value>("select runtime_native_snapshot_body($1,$2)")
            .bind(id)
            .bind(Uuid::now_v7())
            .fetch_one(store.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query("update runtime_native_snapshot_manifests set layout='[]' where id=$1")
            .bind(id)
            .execute(store.pool())
            .await
            .is_err()
    );
    sqlx::query("delete from runtime_events where id=any($1)")
        .bind(vec![a.id, batch[0].id])
        .execute(store.pool())
        .await
        .unwrap();
    let retained: i64 = sqlx::query_scalar("select count(*) from runtime_native_snapshot_items")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(
        retained, 4,
        "the second snapshot still owns all shared items"
    );
    sqlx::query("delete from runtime_events where flow_run_id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let remaining:(i64,i64,i64)=sqlx::query_as("select (select count(*) from runtime_native_snapshot_manifests),(select count(*) from runtime_native_snapshot_items),(select count(*) from runtime_native_snapshot_references)")
        .fetch_one(store.pool()).await.unwrap();
    assert_eq!(remaining, (0, 0, 0));
}

#[tokio::test]
async fn native_snapshot_full_equality_rejects_hash_collision_and_missing_item() {
    let (store, run, application, scope) = seed().await;
    let item = "{\"content\":\"original\"}";
    let hash = format!("sha256:{:x}", Sha256::digest(item.as_bytes()));
    let id = Uuid::now_v7();
    sqlx::query("insert into runtime_native_snapshot_items(id,scope_id,application_id,content_hash,body,byte_size) values($1,$2,$3,$4,'wrong',5)")
        .bind(id).bind(scope).bind(application).bind(hash).execute(store.pool()).await.unwrap();
    let body = format!("{{\"messages\":[{item}]}}");
    assert!(store
        .append_runtime_event(&event(run, &body))
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar("select count(*) from runtime_events where flow_run_id=$1")
        .bind(run)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "failed equality must roll back the event");
    sqlx::query("delete from runtime_native_snapshot_items where id=$1")
        .bind(id)
        .execute(store.pool())
        .await
        .unwrap();
    let root = Uuid::now_v7();
    sqlx::query("insert into runtime_native_snapshot_manifests(id,scope_id,application_id,content_hash,byte_size,layout) values($1,$2,$3,$4,$5,$6)")
        .bind(root).bind(scope).bind(application).bind(format!("sha256:{:x}",Sha256::digest(body.as_bytes())))
        .bind(body.len() as i64).bind(json!([["item",Uuid::now_v7()]])).execute(store.pool()).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, Value>("select runtime_native_snapshot_body($1,$2)")
            .bind(root)
            .bind(run)
            .fetch_one(store.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_snapshot_history_switch_is_exact_idempotent_and_preserves_generic_owner() {
    let (store, run, application, scope) = seed().await;
    let body = "{\"messages\":[{\"text\":\"historical\\u0000value\"}],\"tools\":[]}";
    let generic = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: scope,
            application_id: application,
            content: json!(body),
        })
        .await
        .unwrap();
    let old = Uuid::now_v7();
    let projected = json!({"source":"ai_native","kind":"model_call","step_key":"old-occurrence","_observation_body_ref":{"content_id":generic.id,"application_id":application}});
    sqlx::query("insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,visibility,durability,observation_body_content_id) values($1,$2,1,'provider_semantic_step','runtime_item','host','host_fact',$3,'user','durable',$4)")
        .bind(old).bind(run).bind(projected).bind(generic.id).execute(store.pool()).await.unwrap();
    assert!(store.migrate_native_snapshot_event(old, run).await.unwrap());
    assert!(!store.migrate_native_snapshot_event(old, run).await.unwrap());
    let restored:Value=sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
        .bind(old).fetch_one(store.pool()).await.unwrap();
    assert_eq!(restored["body"], body);
    assert_eq!(restored["step_key"], "old-occurrence");
    let generic_retained: bool =
        sqlx::query_scalar("select exists(select 1 from runtime_canonical_contents where id=$1)")
            .bind(generic.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        generic_retained,
        "a pre-existing generic canonical value is not owned by observation migration"
    );
}
