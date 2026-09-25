use crate::ports::{
    ProviderContinuation, ProviderProtocolContextValue, ProviderTransportAffinity,
    ProviderTransportPayload,
};
use serde_json::json;

#[test]
fn responses_transport_digests_and_size_preserve_wire_semantics() {
    let body = json!({
        "model": "gpt-test",
        "input": [
            {"role": "user", "content": "quote \" and newline\n 雪"},
            {"role": "assistant", "content": "done"}
        ],
        "tools": [{"z": 1, "a": ["x", "\\"]}],
        "stream": true,
        "metadata": {"x": "y"}
    });
    let payload = ProviderTransportPayload::openai_responses(body.clone()).unwrap();

    assert_eq!(
        payload.size_bytes(),
        serde_json::to_vec(&body).unwrap().len()
    );
    assert_eq!(
        payload.digest(),
        "sha256:4462b71413b09bba24cf0046a6993a5c7c861895b36fa11f227468d5eaff7777"
    );
    assert_eq!(
        payload.configuration_digest().unwrap(),
        "sha256:ef9904a459879be995e483fc119e83f8907feea7964aa7cab0d7269bc7f80bf5"
    );
    assert_eq!(
        ProviderTransportPayload::openai_responses_configuration_digest(&body).unwrap(),
        payload.configuration_digest().unwrap()
    );
    assert_eq!(
        payload.user_messages_digest().unwrap().as_deref(),
        Some("sha256:b781c49fc8c4f62559151895815b64246b6a011f06732bd0b8693a2c65e7171a")
    );

    let continuation = ProviderContinuation::new(
        "resp_test",
        ProviderTransportAffinity::new("provider-a", "openai", "openai_responses", "gpt-test"),
    )
    .unwrap();
    let continued = payload.bind_openai_continuation(continuation).unwrap();
    let mut continued_body = body;
    continued_body["previous_response_id"] = json!("resp_test");
    assert_eq!(continued.wire_body(), &continued_body);
    assert_eq!(
        continued.size_bytes(),
        serde_json::to_vec(&continued_body).unwrap().len()
    );
    assert_eq!(
        continued.digest(),
        "sha256:8901802dfe73d856a0eb407ccd3bb00533d29d4220abf57775d0519cd5f59609"
    );
}

#[test]
fn continuation_accepts_only_a_sealed_session_identity() {
    let affinity =
        ProviderTransportAffinity::new("provider-a", "openai", "openai_responses", "gpt-test");
    let identity = "a".repeat(64);
    let continuation = ProviderContinuation::new("resp_test", affinity.clone())
        .unwrap()
        .with_session_identity(Some(&identity))
        .unwrap();
    assert_eq!(continuation.session_identity(), Some(identity.as_str()));
    assert!(ProviderContinuation::new("resp_test", affinity)
        .unwrap()
        .with_session_identity(Some("client-controlled-id"))
        .is_err());
}

#[test]
fn transport_clone_shares_large_wire_until_continuation_mutates_it() {
    let body = json!({"model": "gpt-test", "input": "x".repeat(1024 * 1024)});
    let original = ProviderTransportPayload::openai_responses(body.clone()).unwrap();
    let cloned = original.clone();
    assert!(std::ptr::eq(original.wire_body(), cloned.wire_body()));

    let continuation = ProviderContinuation::new(
        "resp_next",
        ProviderTransportAffinity::new("provider-a", "openai", "openai_responses", "gpt-test"),
    )
    .unwrap();
    let changed = cloned.bind_openai_continuation(continuation).unwrap();
    assert_eq!(original.wire_body(), &body);
    assert_eq!(changed.wire_body()["previous_response_id"], "resp_next");
    assert_ne!(original.digest(), changed.digest());
}

#[test]
fn protocol_context_digest_and_size_match_canonical_wire() {
    let value = json!({
        "source_protocol": "openai_responses",
        "body": {"z": "line\n雪", "a": [1, true, null]}
    });
    let context = ProviderProtocolContextValue::new(value.clone()).unwrap();
    let transport = ProviderTransportPayload::openai_responses(value.clone()).unwrap();
    assert_eq!(context.digest(), transport.digest());
    assert_eq!(
        context.size_bytes(),
        serde_json::to_vec(&value).unwrap().len()
    );
    assert!(context.matches_locator(&context.original_locator()));
}

#[test]
fn reasoning_default_configuration_digest_matches_sealed_wire_without_input() {
    let body = json!({
        "model": "gpt-test",
        "input": "history".repeat(128 * 1024),
        "reasoning": {"summary": "auto"},
        "tools": [{"name": "one"}]
    });
    let mut sealed = body.clone();
    sealed["reasoning"]["effort"] = json!("max");
    assert_eq!(
        ProviderTransportPayload::openai_responses_configuration_digest_with_reasoning_default(
            &body, "max"
        )
        .unwrap(),
        ProviderTransportPayload::openai_responses_configuration_digest(&sealed).unwrap()
    );

    let mut without_reasoning = body.clone();
    without_reasoning
        .as_object_mut()
        .unwrap()
        .remove("reasoning");
    sealed["reasoning"] = json!({"effort": "max"});
    assert_eq!(
        ProviderTransportPayload::openai_responses_configuration_digest_with_reasoning_default(
            &without_reasoning,
            "max"
        )
        .unwrap(),
        ProviderTransportPayload::openai_responses_configuration_digest(&sealed).unwrap()
    );

    assert_eq!(
        ProviderTransportPayload::openai_responses_configuration_digest_with_reasoning_default(
            &sealed, "low"
        )
        .unwrap(),
        ProviderTransportPayload::openai_responses_configuration_digest(&sealed).unwrap()
    );
    without_reasoning["reasoning"] = json!(42);
    assert_eq!(
        ProviderTransportPayload::openai_responses_configuration_digest_with_reasoning_default(
            &without_reasoning,
            "max"
        )
        .unwrap_err()
        .to_string(),
        "provider_transport_reasoning_must_be_object"
    );
}
