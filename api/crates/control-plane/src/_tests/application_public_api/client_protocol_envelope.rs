use std::collections::BTreeMap;

use control_plane::application_public_api::client_protocol_envelope::{
    capture_client_protocol_body, capture_client_protocol_envelope, capture_client_protocol_query,
    merge_client_protocol_envelopes, ClientProtocolIngressPolicy,
};
use control_plane::application_public_api::{
    mapping::ApplicationApiMappingConfig,
    native::{NativeInputMapper, NativeRunRequest},
};
use control_plane::ports::ProviderProtocolContextLocator;
use plugin_framework::provider_contract::{
    ProtocolContextEnvelope, CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
};
use serde_json::json;

#[test]
fn anthropic_policy_preserves_repeated_safe_headers_and_subtracts_typed_or_unsafe_headers() {
    let envelope = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::AnthropicMessages,
        [
            ("Anthropic-Version", "2023-06-01"),
            ("anthropic-beta", "prompt-caching"),
            ("anthropic-beta", "context-1m-2025-08-07, private-beta"),
            ("x-claude-code-session-id", "typed-session"),
            ("authorization", "Bearer platform-key"),
            ("x-api-key", "platform-key"),
            ("cookie", "session=secret"),
            ("host", "api.example.test"),
            ("content-length", "42"),
            ("connection", "keep-alive, x-hop-secret"),
            ("x-hop-secret", "must-not-cross"),
            ("x-internal-route", "must-not-cross"),
        ],
    )
    .expect("safe Anthropic headers should produce protocol context");

    assert_eq!(envelope.source_protocol, "anthropic_messages");
    assert_eq!(envelope.headers["anthropic-version"], vec!["2023-06-01"]);
    assert_eq!(
        envelope.headers["anthropic-beta"],
        vec!["prompt-caching", "private-beta"]
    );
    for stripped in [
        "x-claude-code-session-id",
        "authorization",
        "x-api-key",
        "cookie",
        "host",
        "content-length",
        "connection",
        "x-hop-secret",
        "x-internal-route",
    ] {
        assert!(!envelope.headers.contains_key(stripped), "{stripped}");
    }
}

#[test]
fn protocol_query_preserves_repeated_safe_values_and_strips_credentials_or_internal_keys() {
    let envelope = capture_client_protocol_query(
        ClientProtocolIngressPolicy::OpenAiResponses,
        [
            ("preview", "one"),
            ("preview", "two"),
            ("authorization", "Bearer query-secret"),
            ("__client_protocol_envelope", "internal"),
        ],
    )
    .expect("safe query residual should produce protocol context");

    assert_eq!(envelope.source_protocol, "openai_responses");
    assert_eq!(envelope.query["preview"], vec!["one", "two"]);
    assert!(!envelope.query.contains_key("authorization"));
    assert!(!envelope.query.contains_key("__client_protocol_envelope"));
}

#[test]
fn protocol_body_subtracts_typed_roots_and_sanitizes_safe_unknown_residuals() {
    let body = json!({
        "model": "claude-test",
        "messages": [{"role": "user", "content": "hello"}],
        "context_management": {
            "edits": [{
                "type": "clear_thinking_20251015",
                "authorization": "nested-secret"
            }]
        },
        "future_extension": {
            "shape": "opaque",
            "cookie": "nested-cookie"
        },
        "authorization": "root-secret",
        "__native_transport": {"must_not_cross": true}
    });
    let envelope = capture_client_protocol_body(
        ClientProtocolIngressPolicy::AnthropicMessages,
        body.as_object().expect("fixture body is an object"),
        &["model", "messages"],
    )
    .expect("safe body residual should produce protocol context");

    assert_eq!(
        envelope.body["context_management"]["edits"][0]["type"],
        "clear_thinking_20251015"
    );
    assert!(envelope.body["context_management"]["edits"][0]
        .get("authorization")
        .is_none());
    assert_eq!(envelope.body["future_extension"]["shape"], "opaque");
    assert!(envelope.body["future_extension"].get("cookie").is_none());
    for stripped in ["model", "messages", "authorization", "__native_transport"] {
        assert!(!envelope.body.contains_key(stripped), "{stripped}");
    }
}

#[test]
fn query_headers_and_body_merge_into_one_protocol_context_envelope() {
    let headers = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::OpenAiChat,
        [("openai-organization", "org-test")],
    );
    let query = capture_client_protocol_query(
        ClientProtocolIngressPolicy::OpenAiChat,
        [("preview", "one"), ("preview", "two")],
    );
    let body = json!({"future_chat_option": {"mode": "exact"}});
    let body = capture_client_protocol_body(
        ClientProtocolIngressPolicy::OpenAiChat,
        body.as_object().expect("fixture body is an object"),
        &[],
    );

    let envelope = merge_client_protocol_envelopes(
        ClientProtocolIngressPolicy::OpenAiChat,
        merge_client_protocol_envelopes(ClientProtocolIngressPolicy::OpenAiChat, headers, query),
        body,
    )
    .expect("all safe residual locations should share one envelope");

    assert_eq!(envelope.source_protocol, "openai_chat");
    assert_eq!(envelope.query["preview"], vec!["one", "two"]);
    assert_eq!(envelope.headers["openai-organization"], vec!["org-test"]);
    assert_eq!(envelope.body["future_chat_option"]["mode"], "exact");
}

#[test]
fn wp_d1c_native_input_mapper_places_only_the_ephemeral_locator_in_durable_input() {
    let mut request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello",
        "model": "claude",
        "inputs": { "topic": "refund" }
    }))
    .unwrap();
    request.client_protocol_envelope = Some(ProtocolContextEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        query: BTreeMap::from([("preview".to_string(), vec!["one".to_string()])]),
        headers: BTreeMap::from([(
            "anthropic-version".to_string(),
            vec!["2023-06-01".to_string()],
        )]),
        body: BTreeMap::from([("context_management".to_string(), json!({"edits": []}))]),
    });

    let mapped = NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
        .expect("native input mapping should succeed");

    let durable_locator = &mapped.node_input_payload[CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY];
    let locator = ProviderProtocolContextLocator::parse(durable_locator)
        .expect("durable context projection should be valid")
        .expect("durable context projection should be a locator");
    assert!(locator.digest().starts_with("sha256:"));
    assert!(locator.size_bytes() > 0);
    let durable_text = durable_locator.to_string();
    assert!(durable_text.contains("anthropic_messages"));
    assert!(!durable_text.contains("anthropic-version"));
    assert!(!durable_text.contains("context_management"));
    assert!(!durable_text.contains("2023-06-01"));
    assert!(mapped.node_input_payload["node-start"]
        .get(CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY)
        .is_none());
}
