use super::*;

#[tokio::test]
async fn native_http_full_input_preserves_client_fallback_and_explicit_node_policy() {
    for (cursor, policy, expected) in [
        (None, "inherit", RecoveryTransport::ProviderHttp),
        (
            Some(serde_json::Value::Null),
            "inherit",
            RecoveryTransport::ProviderHttp,
        ),
        (
            None,
            "force_websocket",
            RecoveryTransport::AiNativeWebSocket,
        ),
        (None, "force_http_sse", RecoveryTransport::ProviderHttp),
        (
            Some(serde_json::json!("resp_previous")),
            "inherit",
            RecoveryTransport::AiNativeWebSocket,
        ),
    ] {
        let coordinator = TransportSessionCoordinator::new_with_clock(
            Arc::new(FakeTransportRuntime::new([])),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap();
        let mut input = invocation_input("http-client-fallback", ProviderWireOperation::Generate);
        input.provider_config = serde_json::json!({"transport_mode":"responses_websocket"});
        input.model_parameters.clear();
        input.model_parameters.insert(
            "responses_transport_policy".into(),
            serde_json::json!(policy),
        );
        let mut body = serde_json::json!({"model":"model-a","input":[{"role":"user","content":"full client input"}]});
        if let Some(cursor) = cursor {
            body["previous_response_id"] = cursor;
        }
        input.native_transport = Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: "openai_responses".into(),
                wire_body: body,
                digest: "fixture".into(),
                size_bytes: 0,
            },
        );
        let original = input.native_transport.clone();
        let prepared = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prepared.transport, expected, "policy={policy}");
        assert_eq!(
            input.native_transport, original,
            "transport choice must preserve raw protocol input"
        );
    }
}

#[tokio::test]
async fn native_websocket_full_input_retains_websocket_carrier() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let mut input = scope_input(&scope, "model-a");
    input.provider_config = serde_json::json!({"transport_mode":"http_sse"});
    input.model_parameters.clear();
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let prepared = coordinator
        .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input.client_transport,
        Some(ProviderClientTransport::Websocket)
    );
    assert_eq!(prepared.transport, RecoveryTransport::AiNativeWebSocket);
}
