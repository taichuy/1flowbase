use super::*;

#[tokio::test]
async fn observation_bodies_share_exact_content_and_resolve_with_owner() {
    let (store, run, _, _) = seed().await;
    let body = exceptional();
    let event = |step: &str| AppendRuntimeEventInput {
        flow_run_id: run,
        node_run_id: None,
        span_id: None,
        parent_span_id: None,
        event_type: "provider_semantic_step".into(),
        layer: domain::RuntimeEventLayer::ProviderRaw,
        source: domain::RuntimeEventSource::ProviderPlugin,
        trust_level: domain::RuntimeTrustLevel::ExternalOpaque,
        item_id: None,
        ledger_ref: None,
        payload: json!({"body":body,"step_key":step,"source":"ai_native"}),
        visibility: domain::RuntimeEventVisibility::User,
        durability: domain::RuntimeEventDurability::Durable,
    };
    let first = store.append_runtime_event(&event("one")).await.unwrap();
    let batch = store
        .append_runtime_events(&[event("two"), event("three")])
        .await
        .unwrap();
    for record in std::iter::once(&first).chain(batch.iter()) {
        assert_eq!(record.payload["body"], body);
        assert!(record.payload.get("_observation_body_ref").is_none());
        let (physical, originals, reference): (Value,Value,Uuid) = sqlx::query_as(
            "select payload,raw_json_payloads,observation_body_content_id from runtime_events where id=$1",
        ).bind(record.id).fetch_one(store.pool()).await.unwrap();
        assert!(physical.get("body").is_none());
        assert_eq!(originals, json!({}));
        assert_eq!(
            physical["_observation_body_ref"]["content_id"],
            json!(reference)
        );
        let restored:Value=sqlx::query_scalar("select runtime_event_original_payload(payload,raw_json_payloads,flow_run_id) from runtime_events where id=$1")
            .bind(record.id).fetch_one(store.pool()).await.unwrap();
        assert_eq!(restored, record.payload);
        // A valid content UUID alone must not allow resolution under another run.
        assert!(sqlx::query_scalar::<_,Value>("select runtime_event_original_payload(payload,raw_json_payloads,$2) from runtime_events where id=$1")
            .bind(record.id).bind(Uuid::now_v7()).fetch_one(store.pool()).await.is_err());
    }
    let shared:i64=sqlx::query_scalar("select count(distinct observation_body_content_id) from runtime_events where flow_run_id=$1")
        .bind(run).fetch_one(store.pool()).await.unwrap();
    assert_eq!(shared, 1);
    let content: Uuid =
        sqlx::query_scalar("select observation_body_content_id from runtime_events where id=$1")
            .bind(first.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    sqlx::query("delete from runtime_events where id=$1")
        .bind(first.id)
        .execute(store.pool())
        .await
        .unwrap();
    let retained: bool =
        sqlx::query_scalar("select exists(select 1 from runtime_canonical_contents where id=$1)")
            .bind(content)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(retained, "another observation still owns the shared body");
    sqlx::query("delete from runtime_events where flow_run_id=$1")
        .bind(run)
        .execute(store.pool())
        .await
        .unwrap();
    let retained: bool =
        sqlx::query_scalar("select exists(select 1 from runtime_canonical_contents where id=$1)")
            .bind(content)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        !retained,
        "observation-only content must be reclaimed with its last owner"
    );
    let (app, scope): (Uuid, Uuid) =
        sqlx::query_as("select application_id,scope_id from flow_runs where id=$1")
            .bind(run)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let generic = store
        .put_canonical_runtime_content(&PutCanonicalRuntimeContentInput {
            scope_id: scope,
            application_id: app,
            content: body.clone(),
        })
        .await
        .unwrap();
    let reused = store
        .append_runtime_event(&event("generic-owned"))
        .await
        .unwrap();
    sqlx::query("delete from runtime_events where id=$1")
        .bind(reused.id)
        .execute(store.pool())
        .await
        .unwrap();
    let retained: bool =
        sqlx::query_scalar("select exists(select 1 from runtime_canonical_contents where id=$1)")
            .bind(generic.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        retained,
        "pre-existing canonical content retains its original owner lifecycle"
    );
}
