use super::*;

#[tokio::test]
async fn reasoning_signature_uses_required_live_lane_and_preserves_order_at_capacity_one() {
    use extension_package_runtime::provider_contract::{ProviderFinishReason, ProviderUsage};

    let (required, mut required_receiver) = tokio::sync::mpsc::channel(1);
    let (diagnostic, _diagnostic_receiver) = tokio::sync::mpsc::channel(1);
    let expected = vec![
        ProviderStreamEvent::TextDelta {
            delta: "same".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "same".to_string(),
        },
        ProviderStreamEvent::ReasoningSignatureDelta {
            signature: "opaque-signature-fixture".to_string(),
        },
        ProviderStreamEvent::ToolCallDelta {
            call_id: "call-1".to_string(),
            delta: serde_json::json!({"arguments":"{}"}),
        },
        ProviderStreamEvent::UsageDelta {
            usage: ProviderUsage::default(),
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        },
    ];
    let produced = expected.clone();
    let producer = tokio::spawn(async move {
        for event in produced {
            forward_provider_live_event(Some(&required), Some(&diagnostic), None, event)
                .await
                .unwrap();
        }
    });

    let mut received = Vec::new();
    while received.len() < expected.len() {
        tokio::task::yield_now().await;
        received.push(required_receiver.recv().await.unwrap());
    }
    producer.await.unwrap();

    assert_eq!(received, expected);
}

#[tokio::test]
async fn saturated_diagnostic_lane_does_not_block_required_live_events() {
    let (required, mut required_receiver) = tokio::sync::mpsc::channel(1);
    let (diagnostic, _diagnostic_receiver) = tokio::sync::mpsc::channel(1);
    let native = ProviderStreamEvent::NativeEvent {
        protocol: "fixture".to_string(),
        event: serde_json::json!({"progress":1}),
    };
    forward_provider_live_event(Some(&required), Some(&diagnostic), None, native.clone())
        .await
        .unwrap();
    forward_provider_live_event(Some(&required), Some(&diagnostic), None, native)
        .await
        .unwrap();

    let required_event = ProviderStreamEvent::ReasoningDelta {
        delta: "truth".to_string(),
    };
    forward_provider_live_event(
        Some(&required),
        Some(&diagnostic),
        None,
        required_event.clone(),
    )
    .await
    .unwrap();

    assert_eq!(required_receiver.recv().await, Some(required_event));
}

#[test]
fn protocol_observation_capability_is_outer_host_owned_and_legacy_compatible() {
    let request = ProviderStdioRequest {
        method: extension_contracts::provider_contract::ProviderStdioMethod::Invoke,
        input: serde_json::json!({"model": "fixture", "run_context": {"host_capabilities": ["untrusted"]}}),
    };
    let wire: Value =
        serde_json::from_slice(&serialize_provider_stdio_request(&request, true).unwrap()).unwrap();
    assert_eq!(
        wire["host_capabilities"],
        serde_json::json!(["protocol_observation_v1"])
    );
    assert_eq!(wire["input"], request.input);
    let off: Value =
        serde_json::from_slice(&serialize_provider_stdio_request(&request, false).unwrap())
            .unwrap();
    assert!(off.get("host_capabilities").is_none());
    // The old stdio envelope ignores new outer metadata without changing strict input.
    let legacy: ProviderStdioRequest = serde_json::from_value(wire).unwrap();
    assert_eq!(legacy, request);
    let unary = ProviderStdioRequest {
        input: serde_json::json!({"operation":"compact"}),
        ..request
    };
    let wire: Value =
        serde_json::from_slice(&serialize_provider_stdio_request(&unary, true).unwrap()).unwrap();
    assert!(wire.get("host_capabilities").is_none());
}

#[tokio::test]
async fn protocol_observation_uses_only_independent_sink() {
    #[derive(Debug, Default)]
    struct Capture(std::sync::Mutex<Vec<ProviderStreamEvent>>);
    impl runtime_core::runtime_backend::RuntimeProtocolObservationSink for Capture {
        fn observe(&self, event: ProviderStreamEvent) {
            self.0.lock().unwrap().push(event);
        }
    }
    let capture = Capture::default();
    let (required, mut receiver) = tokio::sync::mpsc::channel(1);
    let (diagnostic, _diagnostic_receiver) = tokio::sync::mpsc::channel(1);
    diagnostic
        .try_send(ProviderStreamEvent::NativeEvent {
            protocol: "fixture".into(),
            event: serde_json::json!({}),
        })
        .unwrap();
    let line = serde_json::json!({
        "type": "protocol_observation", "protocol": "openai.responses",
        "transport": "sse", "direction": "received", "kind": "response_body",
        "body": "data: [DONE]\n\n", "encoding": "utf8", "status": null,
    });
    let event = serde_json::from_value::<ProviderRuntimeLine>(line.clone())
        .unwrap()
        .into_stream_event()
        .unwrap();
    assert_eq!(serde_json::to_value(&event).unwrap(), line);
    forward_provider_live_event(
        Some(&required),
        Some(&diagnostic),
        Some(&capture),
        event.clone(),
    )
    .await
    .unwrap();
    assert!(receiver.try_recv().is_err());
    assert_eq!(*capture.0.lock().unwrap(), vec![event]);
}

fn assert_expired_timeout_contract(
    timeout_state: ProviderStreamTimeoutState,
    limits: PluginRuntimeLimits,
    expected_kind: ProviderTimeoutKind,
    expected_timeout_ms: u64,
) {
    let (duration, timeout_kind, timeout_ms) = timeout_state.next_read_timeout(&limits);
    assert!(duration.is_zero());
    assert_eq!(timeout_kind, expected_kind);
    assert_eq!(timeout_ms, expected_timeout_ms);

    let error = provider_timeout_error(timeout_kind, timeout_ms);
    let PluginFrameworkError::RuntimeContract { error } = error else {
        panic!("expected provider runtime contract error, got {error:?}");
    };
    assert!(error.message.contains("provider runtime timed out"));
    let expected_summary = format!(
        "timeout_kind={};timeout_ms={timeout_ms}",
        timeout_kind.as_str()
    );
    assert_eq!(
        error.provider_summary.as_deref(),
        Some(expected_summary.as_str())
    );
}

#[test]
fn expired_wall_clock_budget_produces_wall_clock_contract_error() {
    assert_expired_timeout_contract(
        ProviderStreamTimeoutState {
            started_at: Instant::now() - Duration::from_millis(200),
            first_token_seen: false,
            last_stream_event_at: None,
        },
        PluginRuntimeLimits {
            timeout_ms: Some(100),
            ..Default::default()
        },
        ProviderTimeoutKind::WallClock,
        100,
    );
}

#[test]
fn expired_first_token_budget_produces_first_token_contract_error() {
    assert_expired_timeout_contract(
        ProviderStreamTimeoutState {
            started_at: Instant::now() - Duration::from_millis(200),
            first_token_seen: false,
            last_stream_event_at: None,
        },
        PluginRuntimeLimits {
            timeout_ms: Some(2_000),
            first_token_timeout_ms: Some(100),
            ..Default::default()
        },
        ProviderTimeoutKind::FirstToken,
        100,
    );
}

#[test]
fn expired_stream_idle_budget_produces_stream_idle_contract_error() {
    assert_expired_timeout_contract(
        ProviderStreamTimeoutState {
            started_at: Instant::now() - Duration::from_millis(200),
            first_token_seen: true,
            last_stream_event_at: Some(Instant::now() - Duration::from_millis(200)),
        },
        PluginRuntimeLimits {
            timeout_ms: Some(2_000),
            stream_idle_timeout_ms: Some(100),
            ..Default::default()
        },
        ProviderTimeoutKind::StreamIdle,
        100,
    );
}
