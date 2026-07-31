use control_plane::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolContextLocator,
    ProviderProtocolContextSlotId, ProviderProtocolContextValue, ProviderTransportAffinity,
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

fn responses_continuation() -> ProviderContinuation {
    ProviderContinuation::new(
        "provider-response-secret",
        ProviderTransportAffinity::new(
            "provider-instance-a",
            "openai",
            "openai_responses",
            "gpt-test",
        ),
    )
    .expect("fixture continuation must be valid")
}

fn protocol_context_value(canary: &str) -> ProviderProtocolContextValue {
    ProviderProtocolContextValue::new(json!({
        "source_protocol": "anthropic_messages",
        "headers": {"anthropic-beta": [canary]},
        "body": {
            "context_management": {
                "edits": [{"type": "clear_thinking_20251015"}]
            }
        }
    }))
    .expect("fixture protocol context must serialize")
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

#[tokio::test]
async fn wp_d1c_protocol_context_round_trips_only_through_its_ephemeral_locator() {
    const CANARY: &str = "WP-D1C-RAW-CONTEXT-CANARY";
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let flow_run_id = Uuid::now_v7();
    let value = protocol_context_value(CANARY);
    let locator = value.original_locator();
    let locator_value = locator.as_value();
    let slot = ProviderProtocolContextSlotId::for_locator(flow_run_id, &locator);

    store
        .put_protocol_context(slot, value.clone())
        .await
        .unwrap();

    assert_eq!(
        store.get_protocol_context(slot).await.unwrap(),
        Some(value.clone())
    );
    assert_eq!(store.get_protocol_context(slot).await.unwrap(), Some(value));
    assert_eq!(
        ProviderProtocolContextLocator::parse(&locator_value)
            .unwrap()
            .expect("safe locator should parse"),
        locator
    );
    assert!(!locator_value.to_string().contains(CANARY));
    assert!(!format!("{locator:?}").contains(CANARY));
    assert!(!format!("{:?}", protocol_context_value(CANARY)).contains(CANARY));
}

#[tokio::test]
async fn wp_d1c_protocol_context_slot_is_size_bounded_and_ttl_bounded() {
    let flow_run_id = Uuid::now_v7();
    let oversized_store = MemoryProviderTransportStore::new(Duration::minutes(5), 16);
    let oversized_value = protocol_context_value("oversized-context");
    let oversized_slot = ProviderProtocolContextSlotId::for_original_flow_run(flow_run_id);
    assert_eq!(
        oversized_store
            .put_protocol_context(oversized_slot, oversized_value)
            .await
            .unwrap_err()
            .to_string(),
        "ephemeral_protocol_context_too_large"
    );

    let slot_bounded_store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    for index in 0..16 {
        let value = ProviderProtocolContextValue::new(json!({"selected": index})).unwrap();
        let locator = value.derived_locator();
        slot_bounded_store
            .put_protocol_context(
                ProviderProtocolContextSlotId::for_locator(flow_run_id, &locator),
                value,
            )
            .await
            .unwrap();
    }
    let overflow = ProviderProtocolContextValue::new(json!({"selected": "overflow"})).unwrap();
    let overflow_locator = overflow.derived_locator();
    assert_eq!(
        slot_bounded_store
            .put_protocol_context(
                ProviderProtocolContextSlotId::for_locator(flow_run_id, &overflow_locator),
                overflow,
            )
            .await
            .unwrap_err()
            .to_string(),
        "ephemeral_protocol_context_slot_limit_exceeded"
    );

    let expiring_store = MemoryProviderTransportStore::new(Duration::milliseconds(20), 64 * 1024);
    let value = protocol_context_value("expiring-context");
    let locator = value.derived_locator();
    let slot = ProviderProtocolContextSlotId::for_locator(flow_run_id, &locator);
    expiring_store
        .put_protocol_context(slot, value)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    assert_eq!(
        expiring_store.get_protocol_context(slot).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn d3_p3_provider_continuation_restores_opaque_id_and_affinity_only_in_ephemeral_memory() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let previous_run_id = Uuid::now_v7();
    let slot = ProviderContinuationSlotId::for_flow_run(previous_run_id);
    let continuation = responses_continuation();
    store.put_continuation(slot, continuation).await.unwrap();

    let restored = store
        .get_continuation(slot)
        .await
        .unwrap()
        .expect("continuation should remain available inside its TTL");
    let payload = ProviderTransportPayload::openai_responses(json!({
        "model": "gpt-test",
        "previous_response_id": format!("resp_{previous_run_id}"),
        "input": "continue"
    }))
    .unwrap()
    .bind_openai_continuation(restored)
    .unwrap();

    assert_eq!(
        payload.wire_body()["previous_response_id"],
        json!("provider-response-secret")
    );
    assert!(payload.affinity().is_some());
    assert!(!format!("{payload:?}").contains("provider-response-secret"));
}

#[tokio::test]
async fn wp12_sealed_request_and_continuation_are_consumed_once() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let flow_run_id = Uuid::now_v7();
    let request_slot = ProviderTransportSlotId::for_flow_run(flow_run_id);
    let continuation_slot = ProviderContinuationSlotId::for_flow_run(flow_run_id);
    store.put(request_slot, responses_payload()).await.unwrap();
    store
        .put_continuation(continuation_slot, responses_continuation())
        .await
        .unwrap();

    store.consume(request_slot).await.unwrap();
    store.consume_continuation(continuation_slot).await.unwrap();

    assert_eq!(
        store.consume(request_slot).await.unwrap_err().to_string(),
        "ephemeral_transport_missing"
    );
    assert_eq!(
        store
            .consume_continuation(continuation_slot)
            .await
            .expect_err("consumed continuation must be unavailable")
            .to_string(),
        "ephemeral_continuation_missing"
    );
}

#[tokio::test]
async fn wp12_consumed_request_remains_owned_by_the_invocation_for_retries() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let slot = ProviderTransportSlotId::for_flow_run(Uuid::now_v7());
    store.put(slot, responses_payload()).await.unwrap();

    let invocation_payload = store.consume(slot).await.unwrap();

    assert_eq!(invocation_payload.wire_body()["model"], json!("gpt-test"));
    assert_eq!(invocation_payload.wire_body()["model"], json!("gpt-test"));
    assert_eq!(store.get(slot).await.unwrap(), None);
}

#[tokio::test]
async fn wp12_terminal_or_confirmed_no_retry_clears_all_flow_run_secrets() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let flow_run_id = Uuid::now_v7();
    let request_slot = ProviderTransportSlotId::for_flow_run(flow_run_id);
    let continuation_slot = ProviderContinuationSlotId::for_flow_run(flow_run_id);
    store.put(request_slot, responses_payload()).await.unwrap();
    store
        .put_continuation(continuation_slot, responses_continuation())
        .await
        .unwrap();
    let context = protocol_context_value("terminal-context");
    let context_locator = context.derived_locator();
    let context_slot = ProviderProtocolContextSlotId::for_locator(flow_run_id, &context_locator);
    store
        .put_protocol_context(context_slot, context)
        .await
        .unwrap();

    store.clear_flow_run(flow_run_id).await.unwrap();

    assert_eq!(store.get(request_slot).await.unwrap(), None);
    assert_eq!(
        store.get_continuation(continuation_slot).await.unwrap(),
        None
    );
    assert_eq!(
        store.get_protocol_context(context_slot).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn wp_d1c_terminal_cleanup_removes_only_the_owned_protocol_context_lineage() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let terminal_run_id = Uuid::now_v7();
    let other_run_id = Uuid::now_v7();
    let original = protocol_context_value("terminal-original");
    let derived = protocol_context_value("terminal-derived");
    let derived_locator = derived.derived_locator();
    let other = protocol_context_value("other-run");
    let original_slot = ProviderProtocolContextSlotId::for_original_flow_run(terminal_run_id);
    let derived_slot =
        ProviderProtocolContextSlotId::for_locator(terminal_run_id, &derived_locator);
    let other_slot = ProviderProtocolContextSlotId::for_original_flow_run(other_run_id);
    store
        .put_protocol_context(original_slot, original)
        .await
        .unwrap();
    store
        .put_protocol_context(derived_slot, derived)
        .await
        .unwrap();
    store.put_protocol_context(other_slot, other).await.unwrap();
    let continuation_slot = ProviderContinuationSlotId::for_flow_run(terminal_run_id);
    store
        .put_continuation(continuation_slot, responses_continuation())
        .await
        .unwrap();

    assert_eq!(
        store
            .delete_flow_run_protocol_contexts(terminal_run_id)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        store.get_protocol_context(original_slot).await.unwrap(),
        None
    );
    assert_eq!(
        store.get_protocol_context(derived_slot).await.unwrap(),
        None
    );
    assert!(store
        .get_protocol_context(other_slot)
        .await
        .unwrap()
        .is_some());
    assert!(store
        .get_continuation(continuation_slot)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn wp12_expiry_eagerly_clears_request_and_continuation() {
    let store = MemoryProviderTransportStore::new(Duration::milliseconds(20), 64 * 1024);
    let flow_run_id = Uuid::now_v7();
    let request_slot = ProviderTransportSlotId::for_flow_run(flow_run_id);
    let continuation_slot = ProviderContinuationSlotId::for_flow_run(flow_run_id);
    store.put(request_slot, responses_payload()).await.unwrap();
    store
        .put_continuation(continuation_slot, responses_continuation())
        .await
        .unwrap();
    let context = protocol_context_value("expiry-context");
    let context_locator = context.derived_locator();
    let context_slot = ProviderProtocolContextSlotId::for_locator(flow_run_id, &context_locator);
    store
        .put_protocol_context(context_slot, context)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    assert_eq!(store.clear_expired().await.unwrap(), 3);
    assert_eq!(store.get(request_slot).await.unwrap(), None);
    assert_eq!(
        store.get_continuation(continuation_slot).await.unwrap(),
        None
    );
    assert_eq!(
        store.get_protocol_context(context_slot).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn wp12_early_loss_is_an_explicit_failure() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let flow_run_id = Uuid::now_v7();
    let request_slot = ProviderTransportSlotId::for_flow_run(flow_run_id);
    let continuation_slot = ProviderContinuationSlotId::for_flow_run(flow_run_id);
    store.put(request_slot, responses_payload()).await.unwrap();
    store
        .put_continuation(continuation_slot, responses_continuation())
        .await
        .unwrap();
    store.clear_flow_run(flow_run_id).await.unwrap();

    assert_eq!(
        store.consume(request_slot).await.unwrap_err().to_string(),
        "ephemeral_transport_missing"
    );
    assert_eq!(
        store
            .consume_continuation(continuation_slot)
            .await
            .expect_err("cleared continuation must be unavailable")
            .to_string(),
        "ephemeral_continuation_missing"
    );
}

#[tokio::test]
async fn wp12_sealed_body_and_token_do_not_enter_debug_or_error_text() {
    let store = MemoryProviderTransportStore::new(Duration::minutes(5), 64 * 1024);
    let request_slot = ProviderTransportSlotId::for_flow_run(Uuid::now_v7());
    let continuation_slot = ProviderContinuationSlotId::for_flow_run(Uuid::now_v7());

    let request_debug = format!("{:?}", responses_payload());
    let request_error = store.consume(request_slot).await.unwrap_err().to_string();
    let continuation_error = store
        .consume_continuation(continuation_slot)
        .await
        .expect_err("missing continuation must fail explicitly")
        .to_string();

    for safe_text in [request_debug, request_error, continuation_error] {
        assert!(!safe_text.contains("Bearer transport-secret"));
        assert!(!safe_text.contains("provider-response-secret"));
    }
}
