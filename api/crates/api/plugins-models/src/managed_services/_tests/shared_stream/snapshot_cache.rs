use super::*;

#[tokio::test]
async fn unary_projection_cache_is_operation_scoped_and_known_failure_invalidates_ready_state() {
    let store = storage_ephemeral::MemoryKvStore::new("shared-projection-fixture");
    let source = key();
    cache::put_projection(
        &store,
        &source,
        "r1",
        "fixture.snapshot",
        1000,
        json!({"value":42}),
    )
    .await
    .unwrap();
    assert_eq!(
        cache::get_projection(&store, &source, "r1", "fixture.snapshot")
            .await
            .unwrap(),
        Some(json!({"value":42}))
    );
    assert!(
        cache::get_projection(&store, &source, "r1", "other.snapshot")
            .await
            .unwrap()
            .is_none()
    );
    cache::put(&store, &source, "r1", 1000, json!({"ready":true}))
        .await
        .unwrap();
    cache::invalidate(&store, &source, "r1", &["fixture.snapshot".into()])
        .await
        .unwrap();
    assert!(cache::get(&store, &source, "r1").await.unwrap().is_none());
    assert!(
        cache::get_projection(&store, &source, "r1", "fixture.snapshot")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn host_latest_cache_is_revision_and_generation_scoped_with_checked_cadence_ttl() {
    let store = storage_ephemeral::MemoryKvStore::new("shared-source-fixture");
    let source = key();
    let event = json!({"type":"sample","snapshot":{"value":42}});
    cache::put(&store, &source, "r1", 1000, event.clone())
        .await
        .unwrap();
    assert_eq!(
        cache::get(&store, &source, "r1").await.unwrap(),
        Some(event)
    );
    assert!(cache::get(&store, &source, "r2").await.unwrap().is_none());
    let mut generation = source.clone();
    generation.generation = "generation-2".into();
    assert!(cache::get(&store, &generation, "r1")
        .await
        .unwrap()
        .is_none());
    assert_eq!(cache::ttl_ms(5000).unwrap(), 15000);
    use control_plane_contracts::ports::EphemeralKvStore;
    store.set_json(&cache::key(&source,"r1"),json!({"revision":"r1","expires_unix_ms":cache::now_ms()-1,"event":{"type":"sample","snapshot":{"value":99}}}),None).await.unwrap();
    assert!(
        cache::get(&store, &source, "r1").await.unwrap().is_none(),
        "Host freshness rejects expired envelopes even if backend retention has not expired"
    );
    assert!(cache::ttl_ms(0).is_err());
    assert!(cache::ttl_ms(u64::MAX).is_err());
}

#[test]
fn source_payload_uses_declared_selector_interval_and_optional_schema_default() {
    let operation:plugin_framework::ManagedServiceOperation=serde_json::from_value(json!({
        "interface_id":"fixture.source","contribution_id":"fixture.source","method":"POST","path":"/fixture",
        "summary":"Fixture","description":"Fixture source","input_schema":{"type":"object","properties":{"body":{"type":"object","properties":{"machine":{"type":"string"},"cadence":{"type":"integer","default":7000}}}}},"output_schema":{"type":"object"},
        "record_permission":{"resource_code":"records","action_code":"open","selector":{"kind":"record","pointer":"/body/machine"}},
        "stream":{"input_schema":{"type":"object"},"event_schema":{"type":"object"},"shared_source":{"resolve_interface_id":"fixture.resolve","interval_pointer":"/body/cadence"}}
    })).unwrap();
    let payload = super::super::host::record_payload(&operation, "canonical-record", 2000).unwrap();
    assert_eq!(
        payload,
        json!({"path":{},"query":{},"body":{"machine":"canonical-record","cadence":2000}})
    );
    assert_eq!(
        super::super::host::interval(&operation, &payload).unwrap(),
        2000
    );
    assert_eq!(
        super::super::host::interval(&operation, &json!({"body":{"machine":"canonical-record"}}))
            .unwrap(),
        7000
    );
}
