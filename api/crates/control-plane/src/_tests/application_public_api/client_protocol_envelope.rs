use control_plane::application_public_api::client_protocol_envelope::{
    anthropic_messages_envelope_with_beta, capture_client_protocol_envelope,
    merge_anthropic_messages_envelopes, ClientProtocolIngressPolicy,
    ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
};
use control_plane::application_public_api::{
    mapping::ApplicationApiMappingConfig,
    native::{NativeInputMapper, NativeRunRequest},
};
use plugin_framework::provider_contract::{
    ClientProtocolEnvelope, CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY,
};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn anthropic_policy_captures_allowlisted_protocol_headers() {
    let envelope = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::AnthropicMessages,
        [
            ("Anthropic-Version", "2023-06-01"),
            ("anthropic-beta", "prompt-caching,computer-use"),
            ("x-claude-code-session-id", "session-123"),
            ("User-Agent", "ClaudeCode/1.0"),
            ("anthropic-client-version", "0.1.0"),
        ],
    )
    .expect("anthropic policy should capture allowlisted headers");

    assert_eq!(envelope.source_protocol, "anthropic_messages");
    assert_eq!(envelope.policy, "anthropic_messages_v1");
    assert_eq!(
        envelope
            .headers
            .get("anthropic-version")
            .map(String::as_str),
        Some("2023-06-01")
    );
    assert_eq!(
        envelope
            .headers
            .get("x-claude-code-session-id")
            .map(String::as_str),
        Some("session-123")
    );
    assert_eq!(
        envelope.headers.get("user-agent").map(String::as_str),
        Some("ClaudeCode/1.0")
    );
}

#[test]
fn protocol_policy_filters_platform_auth_transport_and_unknown_headers() {
    let envelope = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::AnthropicMessages,
        [
            ("authorization", "Bearer platform-key"),
            ("x-api-key", "platform-key"),
            ("cookie", "session=secret"),
            ("x-csrf-token", "csrf"),
            ("host", "api.example.test"),
            ("content-length", "42"),
            ("connection", "keep-alive"),
            ("transfer-encoding", "chunked"),
            ("accept-encoding", "gzip"),
            ("x-future-provider-header", "future"),
            ("anthropic-version", "2023-06-01"),
        ],
    )
    .expect("one allowlisted header should keep the envelope");

    assert_eq!(envelope.headers.len(), 1);
    assert_eq!(
        envelope
            .headers
            .get("anthropic-version")
            .map(String::as_str),
        Some("2023-06-01")
    );
}

#[test]
fn default_policy_does_not_capture_unknown_protocol_context() {
    let envelope = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::DefaultDeny,
        [
            ("anthropic-version", "2023-06-01"),
            ("x-claude-code-session-id", "session-123"),
            ("user-agent", "ClaudeCode/1.0"),
        ],
    );

    assert!(envelope.is_none());
}

#[test]
fn generated_one_m_beta_merges_with_captured_anthropic_headers() {
    let captured = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::AnthropicMessages,
        [
            ("anthropic-version", "2023-06-01"),
            ("anthropic-beta", "prompt-caching"),
        ],
    );
    let generated = Some(anthropic_messages_envelope_with_beta(
        ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
    ));

    let envelope = merge_anthropic_messages_envelopes(captured, generated)
        .expect("captured and generated headers should keep an envelope");

    assert_eq!(
        envelope
            .headers
            .get("anthropic-version")
            .map(String::as_str),
        Some("2023-06-01")
    );
    assert_eq!(
        envelope.headers.get("anthropic-beta").map(String::as_str),
        Some("prompt-caching,context-1m-2025-08-07")
    );
}

#[test]
fn generated_one_m_beta_is_not_duplicated_when_already_captured() {
    let captured = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::AnthropicMessages,
        [
            ("anthropic-version", "2023-06-01"),
            ("anthropic-beta", "context-1m-2025-08-07"),
        ],
    );
    let generated = Some(anthropic_messages_envelope_with_beta(
        ANTHROPIC_CONTEXT_1M_BETA_HEADER_VALUE,
    ));

    let envelope = merge_anthropic_messages_envelopes(captured, generated)
        .expect("captured and generated headers should keep an envelope");

    assert_eq!(
        envelope.headers.get("anthropic-beta").map(String::as_str),
        Some("context-1m-2025-08-07")
    );
}

#[test]
fn native_input_mapper_places_envelope_in_runtime_reserved_payload() {
    let mut request: NativeRunRequest = serde_json::from_value(json!({
        "query": "hello",
        "model": "claude",
        "inputs": { "topic": "refund" }
    }))
    .unwrap();
    request.client_protocol_envelope = Some(ClientProtocolEnvelope {
        source_protocol: "anthropic_messages".to_string(),
        policy: "anthropic_messages_v1".to_string(),
        headers: BTreeMap::from([("anthropic-version".to_string(), "2023-06-01".to_string())]),
    });

    let mapped = NativeInputMapper::map(&request, &ApplicationApiMappingConfig::default_native())
        .expect("native input mapping should succeed");

    assert_eq!(
        mapped.node_input_payload[CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY]["headers"]
            ["anthropic-version"],
        json!("2023-06-01")
    );
    assert!(mapped.node_input_payload["node-start"]
        .get(CLIENT_PROTOCOL_ENVELOPE_PAYLOAD_KEY)
        .is_none());
}
