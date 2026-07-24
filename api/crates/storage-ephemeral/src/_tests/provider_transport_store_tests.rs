use control_plane::ports::{
    ProviderTransportPayload, ProviderTransportSlotId, ProviderTransportStore,
};
use serde_json::json;
use storage_ephemeral::MemoryProviderTransportStore;
use time::Duration;
use uuid::Uuid;

fn responses_payload() -> ProviderTransportPayload {
    ProviderTransportPayload::openai_responses(json!({
        "model": "gpt-test",
        "tools": [{
            "type": "mcp",
            "server_url": "https://mcp.example.test",
            "authorization": "Bearer transport-secret"
        }],
        "future_extension": { "preserve": true }
    }))
    .expect("fixture payload must be valid")
}

#[test]
fn d4_ac_027_provider_transport_digest_is_canonical_across_object_key_order() {
    let left = ProviderTransportPayload::openai_responses(
        serde_json::from_str(r#"{"model":"gpt-test","input":"hi"}"#).unwrap(),
    )
    .unwrap();
    let right = ProviderTransportPayload::openai_responses(
        serde_json::from_str(r#"{"input":"hi","model":"gpt-test"}"#).unwrap(),
    )
    .unwrap();

    assert_eq!(left.digest(), right.digest());
}

#[tokio::test]
async fn d4_ac_016_provider_transport_store_round_trips_opaque_wire_payload() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let slot = ProviderTransportSlotId::for_flow_run(Uuid::now_v7());
    let payload = responses_payload();

    store.put(slot, payload.clone()).await.unwrap();

    assert_eq!(store.get(slot).await.unwrap(), Some(payload));
}

#[tokio::test]
async fn d4_ac_027_provider_transport_store_deletes_payload_after_handoff() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let slot = ProviderTransportSlotId::for_flow_run(Uuid::now_v7());
    store.put(slot, responses_payload()).await.unwrap();

    assert!(store.delete(slot).await.unwrap());
    assert_eq!(store.get(slot).await.unwrap(), None);
}

#[tokio::test]
async fn d4_ac_027_provider_transport_store_expires_payload_without_durable_fallback() {
    let store = MemoryProviderTransportStore::new(Duration::milliseconds(20), 64 * 1024);
    let slot = ProviderTransportSlotId::for_flow_run(Uuid::now_v7());
    store.put(slot, responses_payload()).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    assert_eq!(store.get(slot).await.unwrap(), None);
}

#[tokio::test]
async fn d4_ac_027_provider_transport_store_rejects_payload_over_its_bound() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 32);
    let slot = ProviderTransportSlotId::for_flow_run(Uuid::now_v7());
    let error = store
        .put(slot, responses_payload())
        .await
        .expect_err("oversized wire payload must be rejected");

    assert!(error
        .to_string()
        .contains("provider_transport_payload_too_large"));
    assert_eq!(store.get(slot).await.unwrap(), None);
}
