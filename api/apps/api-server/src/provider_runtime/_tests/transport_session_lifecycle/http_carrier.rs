use super::*;

#[tokio::test]
async fn native_http_full_input_preserves_client_fallback_and_explicit_node_policy() {
    for scoped in [false, true] {
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
            (
                Some(serde_json::json!("resp_previous")),
                "force_http_sse",
                RecoveryTransport::ProviderHttp,
            ),
        ] {
            let coordinator = Arc::new(
                TransportSessionCoordinator::new_with_clock(
                    Arc::new(FakeTransportRuntime::new([])),
                    transport_config(),
                    FakeClock::new(2_000_000),
                )
                .unwrap(),
            );
            let scope = scoped.then(|| coordinator.open_http_connection_scope());
            let mut input = match &scope {
                Some(guard) => scope_input(&guard.scope, "model-a"),
                None => invocation_input("http-client-fallback", ProviderWireOperation::Generate),
            };
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
            assert_eq!(
                prepared.transport, expected,
                "scoped={scoped} policy={policy}"
            );
            assert_eq!(
                input.native_transport, original,
                "transport choice must preserve raw protocol input"
            );
            assert!(!input
                .client_protocol_envelope
                .as_ref()
                .unwrap()
                .headers
                .contains_key(
                    control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER
                ));
            let result = if expected == RecoveryTransport::ProviderHttp {
                output_with_result(ProviderInvocationResult::default())
            } else {
                successful_output(prepared.lease.fence.generation.get())
            };
            coordinator.finish(prepared, &result).await.unwrap();
        }
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

#[tokio::test]
async fn unscoped_native_http_without_session_keeps_client_carrier() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let mut input = invocation_input("no-session", ProviderWireOperation::Generate);
    input
        .client_protocol_envelope
        .as_mut()
        .unwrap()
        .headers
        .remove("session-id");
    input.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    assert!(coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .is_none());
    assert_eq!(input.client_transport, Some(ProviderClientTransport::Http));
}

#[tokio::test]
async fn owned_http_cursor_selector_preserves_provenance_and_explicit_policy_precedence() {
    for (policy, expected) in [
        ("inherit", RecoveryTransport::ProviderHttp),
        ("force_http_sse", RecoveryTransport::ProviderHttp),
        ("force_websocket", RecoveryTransport::AiNativeWebSocket),
    ] {
        let coordinator = TransportSessionCoordinator::new_with_clock(
            Arc::new(FakeTransportRuntime::new([])),
            transport_config(),
            FakeClock::new(2_000_000),
        )
        .unwrap();
        let scope = coordinator.open_connection_scope();
        let mut input = scope_input(&scope, "model-a");
        input.provider_config = serde_json::json!({"transport_mode":"responses_websocket"});
        input.model_parameters.clear();
        input.model_parameters.insert(
            "responses_transport_policy".into(),
            serde_json::json!(policy),
        );
        input.client_transport = Some(ProviderClientTransport::Http);
        input.native_transport = Some(
            plugin_framework::provider_contract::ProviderNativeTransport {
                protocol: "openai_responses".into(),
                wire_body: serde_json::json!({"model":"model-a", "previous_response_id":"resp_owned", "input":[]}),
                digest: "fixture".into(),
                size_bytes: 0,
            },
        );
        let prepared = coordinator
            .prepare_dispatched("runtime-a", &mut input, &context(2_010_000))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prepared.transport, expected);
        let output = if expected == RecoveryTransport::ProviderHttp {
            output_with_result(ProviderInvocationResult::default())
        } else {
            successful_output(prepared.lease.fence.generation.get())
        };
        coordinator.finish(prepared, &output).await.unwrap();
    }
}
