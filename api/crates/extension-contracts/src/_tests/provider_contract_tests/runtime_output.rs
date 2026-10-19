use super::*;

#[test]
fn provider_invocation_result_exposes_native_response_cursor() {
    let result = ProviderInvocationResult {
        final_content: Some("hello".to_string()),
        response_id: Some("resp_current".to_string()),
        ..ProviderInvocationResult::default()
    };

    let payload = serde_json::to_value(result).unwrap();
    let decoded: ProviderInvocationResult = serde_json::from_value(payload.clone()).unwrap();

    assert_eq!(payload["response_id"], "resp_current");
    assert_eq!(decoded.response_id.as_deref(), Some("resp_current"));
}

#[test]
fn provider_runtime_line_result_is_not_a_stream_event() {
    let line = ProviderRuntimeLine::Result {
        result: ProviderInvocationResult {
            final_content: Some("hello".into()),
            ..ProviderInvocationResult::default()
        },
    };

    assert_eq!(line.into_stream_event(), None);
}

#[test]
fn provider_runtime_line_text_maps_to_stream_event() {
    let line = ProviderRuntimeLine::TextDelta {
        delta: "hello".into(),
    };

    assert_eq!(
        line.into_stream_event(),
        Some(ProviderStreamEvent::TextDelta {
            delta: "hello".into()
        })
    );
}

#[test]
fn provider_runtime_line_reasoning_signature_maps_exactly_to_stream_event() {
    let line: ProviderRuntimeLine = serde_json::from_value(json!({
        "type": "reasoning_signature_delta",
        "signature": "opaque-signature-fixture"
    }))
    .unwrap();

    assert_eq!(
        line.into_stream_event(),
        Some(ProviderStreamEvent::ReasoningSignatureDelta {
            signature: "opaque-signature-fixture".into()
        })
    );
}

#[test]
fn provider_runtime_line_error_preserves_upstream_details() {
    let line = ProviderRuntimeLine::Error {
        error: ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::ProviderUpstreamError,
            "400 Bad Request: upstream rejected request",
        )
        .with_provider_summary("x-request-id=req_123")
        .with_provider_details(json!({
            "status": 400,
            "content_type": "application/json",
            "headers": {
                "x-request-id": "req_123"
            },
            "raw_body": "{\"error\":{\"message\":\"missing instructions\"}}\n"
        })),
    };

    let encoded = serde_json::to_value(&line).unwrap();
    assert_eq!(encoded["error"]["kind"], "provider_upstream_error");
    assert_eq!(
        encoded["error"]["provider_details"]["raw_body"],
        "{\"error\":{\"message\":\"missing instructions\"}}\n"
    );

    let decoded: ProviderRuntimeLine = serde_json::from_value(encoded).unwrap();
    match decoded.into_stream_event() {
        Some(ProviderStreamEvent::Error { error }) => {
            assert_eq!(error.kind, ProviderRuntimeErrorKind::ProviderUpstreamError);
            assert_eq!(
                error.provider_details.unwrap()["headers"]["x-request-id"],
                "req_123"
            );
        }
        other => panic!("expected upstream error stream event, got {other:?}"),
    }
}

#[test]
fn provider_runtime_line_preserves_recoverable_output_protocol_failure() {
    let line = ProviderRuntimeLine::OutputProtocolFailure {
        failure: ProviderOutputProtocolFailure {
            protocol: "dsml".to_string(),
            error_code: "incomplete_envelope".to_string(),
            message: "provider returned incomplete tool-call markup".to_string(),
            retry_feedback: "Emit one complete tool call or answer normally.".to_string(),
            provider_details: json!({"candidate": "</DSML>"}),
        },
    };

    let encoded = serde_json::to_value(&line).unwrap();
    assert_eq!(encoded["type"], "output_protocol_failure");
    assert_eq!(encoded["failure"]["protocol"], "dsml");
    assert_eq!(
        encoded["failure"]["provider_details"]["candidate"],
        "</DSML>"
    );

    let decoded: ProviderRuntimeLine = serde_json::from_value(encoded).unwrap();
    match decoded.into_stream_event() {
        Some(ProviderStreamEvent::OutputProtocolFailure { failure }) => {
            assert_eq!(failure.error_code, "incomplete_envelope");
            assert_eq!(
                failure.retry_feedback,
                "Emit one complete tool call or answer normally."
            );
        }
        other => panic!("expected output protocol failure event, got {other:?}"),
    }
}

#[test]
fn provider_runtime_line_tool_commit_preserves_arguments() {
    let line = ProviderRuntimeLine::ToolCallCommit {
        call: ProviderToolCall {
            id: "call-1".into(),
            name: "lookup_order".into(),
            arguments: json!({ "order_id": "A-1" }),
            provider_metadata: json!({}),
        },
    };

    match line.into_stream_event() {
        Some(ProviderStreamEvent::ToolCallCommit { call }) => {
            assert_eq!(call.arguments, json!({ "order_id": "A-1" }));
        }
        other => panic!("expected tool call commit stream event, got {other:?}"),
    }
}

#[test]
fn provider_runtime_line_provider_output_item_preserves_verified_item() {
    let encoded = json!({
        "type": "output_item",
        "phase": "added",
        "output_index": 2,
        "item": {
            "id": "tool_search_1",
            "type": "tool_search_call",
            "arguments": "{}"
        }
    });

    let line: ProviderRuntimeLine = serde_json::from_value(encoded).unwrap();
    assert_eq!(
        line.into_stream_event(),
        Some(ProviderStreamEvent::OutputItem {
            phase: ProviderOutputItemPhase::Added,
            output_index: 2,
            item: json!({
                "id": "tool_search_1",
                "type": "tool_search_call",
                "arguments": "{}"
            }),
        })
    );
}

#[test]
fn provider_runtime_line_rejects_unknown_provider_output_item_type() {
    let error = serde_json::from_value::<ProviderRuntimeLine>(json!({
        "type": "output_item",
        "phase": "done",
        "output_index": 0,
        "item": { "id": "unknown_1", "type": "computer_call" }
    }))
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("provider output item type is not supported by the typed Responses projection"));
}

// #2028 AC-007: real worker NDJSON must admit canonical native tools unchanged.
#[test]
fn issue_2028_provider_runtime_line_preserves_native_client_tool_items() {
    for item in [
        json!({"id":"ct_1","type":"custom_tool_call","call_id":"call_original","name":"exec","input":"text(await tools.exec_command({cmd: 'cat fixture'}));"}),
        json!({"id":"fc_1","type":"function_call","call_id":"call_original","name":"read","arguments":"{\"path\":\"fixture\"}"}),
    ] {
        for phase in ["added", "done"] {
            let line: ProviderRuntimeLine = serde_json::from_value(
                json!({"type":"output_item","phase":phase,"output_index":1,"item":item}),
            )
            .expect("native tool item must cross the host contract");
            match line.into_stream_event().unwrap() {
                ProviderStreamEvent::OutputItem {
                    item: actual,
                    output_index,
                    ..
                } => {
                    assert_eq!(actual, item);
                    assert_eq!(output_index, 1);
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }
    }
}

// Root #2028 AC-010: the actual provider wire projection rejects old manifests.
#[test]
fn native_output_v1_is_required_even_when_host_input_only_has_legacy_marker() {
    let input = ProviderInvocationInput {
        required_capabilities: [ProviderInvocationCapability::ResponsesNativePassthrough].into(),
        ..Default::default()
    };
    let error = input
        .to_current_provider_generate_wire_value(&["responses.native_passthrough".into()])
        .unwrap_err();
    assert!(error.to_string().contains("responses.native_output.v1"));
    let (wire, _) = input
        .to_current_provider_generate_wire_value(&[
            "responses.native_passthrough".into(),
            "responses.native_output.v1".into(),
        ])
        .unwrap();
    assert!(wire["required_capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("responses.native_output.v1")));
}

#[test]
fn native_output_delta_wire_rejects_diagnostic_and_missing_identity() {
    let delta = json!({"type":"response.output_text.delta","output_index":0,"item_id":"msg_1","content_index":0,"delta":"hello"});
    let line: ProviderRuntimeLine =
        serde_json::from_value(json!({"type":"responses_output_delta","event":delta})).unwrap();
    assert_eq!(
        serde_json::to_value(line.into_stream_event().unwrap()).unwrap(),
        json!({"type":"responses_output_delta","event":delta})
    );
    for event in [
        json!({"type":"response.completed"}),
        json!({"type":"response.output_text.delta","delta":"unowned"}),
    ] {
        assert!(serde_json::from_value::<ProviderRuntimeLine>(
            json!({"type":"responses_output_delta","event":event})
        )
        .is_err());
    }
}

// AC-003/007: execution mode is supplied by the host, never inferred from missing metadata.
