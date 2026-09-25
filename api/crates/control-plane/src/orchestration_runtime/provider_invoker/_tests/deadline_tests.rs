use super::*;
use std::collections::BTreeMap;

#[test]
fn provider_execution_inherits_available_task_deadline() {
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let expected = now.unix_timestamp() * 1_000 + 42_000;
    let mut input = ProviderInvocationInput::default();
    input
        .run_context
        .insert("task_deadline_unix_ms".to_string(), Value::from(expected));

    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, None),
        expected
    );
}

#[test]
fn provider_execution_defaults_to_thirty_minutes() {
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let input = ProviderInvocationInput::default();

    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, None),
        now.unix_timestamp() * 1_000 + 30 * 60 * 1_000
    );
}

#[test]
fn callback_scope_override_changes_only_host_transport_owner() {
    use crate::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER;
    let mut input = ProviderInvocationInput {
        protocol: "openai_responses".into(),
        previous_response_id: Some("committed-response".into()),
        tools: vec![serde_json::json!({"name":"read"})],
        client_protocol_envelope: Some(
            plugin_framework::provider_contract::ProtocolContextEnvelope {
                source_protocol: "openai_responses".into(),
                headers: BTreeMap::from([
                    (
                        HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
                        vec!["old-connection".into()],
                    ),
                    ("session-id".into(), vec!["logical-session".into()]),
                ]),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    let mut expected = input.clone();
    expected
        .client_protocol_envelope
        .as_mut()
        .unwrap()
        .headers
        .insert(
            HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
            vec!["new-connection".into()],
        );
    apply_transport_connection_scope_override(&mut input, Some("new-connection"));
    assert_eq!(
        input, expected,
        "WS resume replaces only transport ownership"
    );
    expected
        .client_protocol_envelope
        .as_mut()
        .unwrap()
        .headers
        .remove(HOST_TRANSPORT_CONNECTION_SCOPE_HEADER);
    apply_transport_connection_scope_override(&mut input, None);
    assert_eq!(
        input, expected,
        "HTTP resume clears obsolete WS ownership without changing tool/cursor content"
    );
}

#[test]
fn provider_execution_does_not_refresh_expired_deadline() {
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let expired = now.unix_timestamp() * 1_000 - 1;
    let mut input = ProviderInvocationInput::default();
    input
        .run_context
        .insert("task_deadline_unix_ms".into(), Value::from(expired));
    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, None),
        expired
    );
}

#[test]
fn websocket_responses_prewarm_has_a_bounded_provider_deadline() {
    let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
    let now_ms = now.unix_timestamp() * 1_000;
    let mut input = ProviderInvocationInput {
        protocol: "openai_responses".into(),
        native_transport: Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: "openai_responses".into(),
                wire_body: json!({"generate": false, "input": []}),
                digest: "fixture".into(),
                size_bytes: 0,
            },
        ),
        ..Default::default()
    };
    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, Some("websocket-owner")),
        now_ms + 30_000
    );
    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, None),
        now_ms + 30 * 60 * 1_000,
        "HTTP Responses is not subject to the WebSocket startup budget"
    );
    input
        .run_context
        .insert("task_deadline_unix_ms".into(), Value::from(now_ms + 9_000));
    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, Some("websocket-owner")),
        now_ms + 9_000,
        "an existing earlier task deadline still wins"
    );
    input.run_context.clear();
    input.native_transport.as_mut().unwrap().wire_body = json!({"generate": true});
    assert_eq!(
        provider_execution_deadline_unix_ms(&input, now, Some("websocket-owner")),
        now_ms + 30 * 60 * 1_000,
        "ordinary Responses generation keeps its long budget"
    );
}

#[test]
fn native_failure_binding_preserves_original_transport_facts() {
    let original = ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        "original disconnect",
    )
    .with_provider_details(json!({"close_code":1011,"native_inference_binding":{"forged":true},"native_inference_configuration_digest":"forged"}));
    let binding = json!({"provider_instance_id":"host-owned"});
    let error = seal_native_failure_binding(
        plugin_framework::PluginFrameworkError::runtime(original.clone()).into(),
        &binding,
        Some("sha256:host-request"),
    );
    let plugin_framework::PluginFrameworkError::RuntimeContract { error } = error
        .downcast_ref::<plugin_framework::PluginFrameworkError>()
        .unwrap()
    else {
        panic!("typed error lost")
    };
    assert_eq!(
        error.provider_details.as_ref().unwrap()["native_inference_configuration_digest"],
        "sha256:host-request"
    );
    assert_eq!(error.kind, original.kind);
    assert_eq!(error.message, original.message);
    assert_eq!(error.provider_details.as_ref().unwrap()["close_code"], 1011);
    assert_eq!(
        error.provider_details.as_ref().unwrap()["native_inference_binding"],
        binding
    );
}

#[test]
fn non_native_failure_cannot_carry_provider_forged_configuration_evidence() {
    let error = ProviderRuntimeError::new(
        ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        "disconnect",
    )
    .with_provider_details(json!({"native_inference_configuration_digest":"forged"}));
    let error = seal_native_failure_binding(
        plugin_framework::PluginFrameworkError::runtime(error).into(),
        &json!({}),
        None,
    );
    let plugin_framework::PluginFrameworkError::RuntimeContract { error } = error
        .downcast_ref::<plugin_framework::PluginFrameworkError>()
        .unwrap()
    else {
        panic!("typed error lost")
    };
    assert!(error
        .provider_details
        .as_ref()
        .unwrap()
        .get("native_inference_configuration_digest")
        .is_none());
}
