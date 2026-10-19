use std::collections::{BTreeMap, BTreeSet};

use crate::provider_count_tokens_estimator::estimate_provider_count_tokens;
use plugin_framework::{
    installation::PluginTaskStatus,
    provider_contract::{
        message_block_required_capabilities, semantic_required_capabilities, ModelDiscoveryMode,
        NativeModelRequestContext, NativePromptBlock, NativePromptCacheControl,
        NativePromptCacheControlType, ProtocolContextEnvelope, ProviderBalanceInfo,
        ProviderBalanceResult, ProviderCanonicalBlockKind, ProviderCompactError,
        ProviderCompactProfile, ProviderCompactResult, ProviderCountTokensCoverage,
        ProviderCountTokensError, ProviderCountTokensFallbackReason, ProviderCountTokensInput,
        ProviderCountTokensMethod, ProviderCountTokensResult, ProviderGenerateProjectionError,
        ProviderGenerateTranslationDecision, ProviderInvocationCapability, ProviderInvocationInput,
        ProviderInvocationResult, ProviderMessage, ProviderMessageRole, ProviderNativeTransport,
        ProviderNetworkEgressContext, ProviderNetworkEgressMode, ProviderOutputItemPhase,
        ProviderOutputProtocolFailure, ProviderProjectionErrorCode, ProviderProjectionFidelity,
        ProviderProjectionLossCode, ProviderProjectionSource, ProviderResetCreditOperation,
        ProviderResetCreditResult, ProviderResetCreditRuntimeInput, ProviderRuntimeError,
        ProviderRuntimeErrorKind, ProviderRuntimeLine, ProviderStdioMethod, ProviderStdioRequest,
        ProviderStdioResponse, ProviderStreamEvent, ProviderToolCall, ProviderUsage,
        ProviderUsageWindow, ProviderUsageWindowsResult, ProviderWireOperation,
        PROVIDER_GENERATE_TRANSLATION_RECEIPT_METADATA_KEY,
    },
};
use serde_json::json;

#[test]
fn model_discovery_mode_accepts_all_supported_wire_values() {
    assert_eq!(
        ModelDiscoveryMode::try_from("static").unwrap(),
        ModelDiscoveryMode::Static
    );
    assert_eq!(
        ModelDiscoveryMode::try_from("dynamic").unwrap(),
        ModelDiscoveryMode::Dynamic
    );
    assert_eq!(
        ModelDiscoveryMode::try_from("hybrid").unwrap(),
        ModelDiscoveryMode::Hybrid
    );
    assert!(ModelDiscoveryMode::try_from("unknown").is_err());
}

#[test]
fn provider_usage_total_tokens_falls_back_to_known_segments() {
    let usage = ProviderUsage {
        input_tokens: Some(120),
        input_cache_hit_tokens: Some(80),
        input_cache_miss_tokens: Some(40),
        output_tokens: Some(45),
        reasoning_tokens: Some(12),
        cache_read_tokens: Some(9),
        cache_write_by_ttl_seconds: None,
        cache_write_tokens: Some(3),
        total_tokens: None,
    };

    assert_eq!(usage.total_tokens(), Some(177));
}

#[test]
fn provider_usage_serializes_input_cache_hit_and_miss_tokens() {
    let usage = ProviderUsage {
        input_tokens: Some(100),
        input_cache_hit_tokens: Some(40),
        input_cache_miss_tokens: Some(60),
        output_tokens: Some(12),
        total_tokens: Some(112),
        ..ProviderUsage::default()
    };

    let payload = serde_json::to_value(&usage).unwrap();

    assert_eq!(payload["input_tokens"], 100);
    assert_eq!(payload["input_cache_hit_tokens"], 40);
    assert_eq!(payload["input_cache_miss_tokens"], 60);
    assert_eq!(payload["output_tokens"], 12);
    assert_eq!(payload["total_tokens"], 112);
}

#[test]
fn d1_p01_count_tokens_reuses_the_complete_generate_envelope_without_field_drift() {
    let invocation = ProviderInvocationInput {
        previous_response_id: Some("resp-1".to_string()),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "hello".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([{"type":"text","text":"world"}])),
        }],
        system: vec![NativePromptBlock::text("system")],
        tools: vec![json!({"name":"lookup","input_schema":{"type":"object"}})],
        mcp_bindings: vec![json!({"server":"catalog"})],
        response_format: Some(json!({"type":"json_schema"})),
        model_parameters: BTreeMap::from([("temperature".to_string(), json!(0.2))]),
        native_transport: Some(ProviderNativeTransport {
            protocol: "native".to_string(),
            wire_body: json!({"opaque":"transport"}),
            digest: "digest".to_string(),
            size_bytes: 22,
        }),
        ..ProviderInvocationInput::default()
    };

    let count = ProviderCountTokensInput::from_invocation(invocation.clone());

    assert_eq!(count.messages, invocation.messages);
    assert_eq!(count.tools, invocation.tools);
    assert_eq!(count.mcp_bindings, invocation.mcp_bindings);
    assert_eq!(count.response_format, invocation.response_format);
    assert_eq!(count.model_parameters, invocation.model_parameters);
    assert_eq!(count.native_transport, invocation.native_transport);
}

#[test]
fn d1_p03_generic_estimator_is_total_for_canonical_prompt_block_families() {
    let invocation = ProviderInvocationInput {
        system: vec![NativePromptBlock::text("system text")],
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "plain text".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: Some(json!([
                {"type":"json","json":{"answer":42}},
                {"type":"image","source":{"type":"base64","media_type":"image/png","data":"aGVsbG8="}},
                {"type":"url","url":"https://example.test/context"},
                {"type":"future_canonical_block","payload":{"nested":true}}
            ])),
        }],
        ..ProviderInvocationInput::default()
    };

    let estimate = estimate_provider_count_tokens(&invocation)
        .expect("bounded canonical fixture must be estimable");

    assert!(estimate.input_tokens > 0);
    assert_eq!(estimate.method, ProviderCountTokensMethod::GenericEstimate);
    assert_eq!(estimate.coverage, ProviderCountTokensCoverage::Partial);
    assert_eq!(estimate.unknown_block_count, 1);
    assert_eq!(estimate.fallback_reason, None);
}

#[test]
fn d1_p02_count_tokens_internal_receipt_fields_have_stable_wire_names() {
    let result = ProviderCountTokensResult::generic_estimate(
        19,
        ProviderCountTokensCoverage::Complete,
        0,
        ProviderCountTokensFallbackReason::CapabilityUnavailable,
    );

    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "operation": "count_tokens",
            "input_tokens": 19,
            "method": "generic_estimate",
            "coverage": "complete",
            "unknown_block_count": 0,
            "fallback_reason": "capability_unavailable"
        })
    );
}

#[test]
fn provider_runtime_error_normalizes_common_vendor_failures() {
    let auth_failed = ProviderRuntimeError::normalize(
        "invalid_api_key",
        "401 unauthorized",
        Some("upstream rejected api key"),
    );
    assert_eq!(auth_failed.kind, ProviderRuntimeErrorKind::AuthFailed);

    let endpoint_unreachable =
        ProviderRuntimeError::normalize("upstream_timeout", "connect timeout", None);
    assert_eq!(
        endpoint_unreachable.kind,
        ProviderRuntimeErrorKind::EndpointUnreachable
    );

    let rate_limited = ProviderRuntimeError::normalize("quota_exceeded", "429", None);
    assert_eq!(rate_limited.kind, ProviderRuntimeErrorKind::RateLimited);

    let unknown = ProviderRuntimeError::normalize("unexpected_shape", "bad payload", None);
    assert_eq!(
        unknown.kind,
        ProviderRuntimeErrorKind::ProviderInvalidResponse
    );
}

#[test]
fn plugin_task_status_marks_only_terminal_states() {
    assert!(!PluginTaskStatus::Pending.is_terminal());
    assert!(!PluginTaskStatus::Running.is_terminal());
    assert!(PluginTaskStatus::Success.is_terminal());
    assert!(PluginTaskStatus::Failed.is_terminal());
    assert!(PluginTaskStatus::Canceled.is_terminal());
    assert!(PluginTaskStatus::TimedOut.is_terminal());
}

#[test]
fn provider_stdio_contract_uses_snake_case_methods_and_result_payloads() {
    let request = ProviderStdioRequest {
        method: ProviderStdioMethod::ListModels,
        input: json!({
            "api_key": "secret"
        }),
    };

    let request_payload = serde_json::to_value(&request).unwrap();
    assert_eq!(request_payload["method"], "list_models");
    assert_eq!(request_payload["input"]["api_key"], "secret");

    let response: ProviderStdioResponse = serde_json::from_value(json!({
        "ok": true,
        "result": [
            {
                "model_id": "fixture_dynamic"
            }
        ]
    }))
    .unwrap();
    assert!(response.ok);
    assert_eq!(response.result[0]["model_id"], "fixture_dynamic");
}

#[test]
fn provider_balance_stdio_method_serializes_balance() {
    let request = ProviderStdioRequest {
        method: ProviderStdioMethod::Balance,
        input: json!({ "api_key": "secret" }),
    };

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "method": "balance",
            "input": { "api_key": "secret" }
        })
    );
}

#[test]
fn provider_usage_and_reset_credit_stdio_contracts_are_typed() {
    let usage = ProviderStdioRequest {
        method: ProviderStdioMethod::Usage,
        input: json!({ "api_key": "secret" }),
    };
    assert_eq!(serde_json::to_value(usage).unwrap()["method"], "usage");

    let reset_credit = ProviderStdioRequest {
        method: ProviderStdioMethod::ResetCredit,
        input: serde_json::to_value(ProviderResetCreditRuntimeInput {
            provider_config: json!({ "api_key": "secret" }),
            operation: ProviderResetCreditOperation::Consume {
                idempotency_key: "attempt-123".to_string(),
            },
        })
        .unwrap(),
    };
    assert_eq!(
        serde_json::to_value(reset_credit).unwrap(),
        json!({
            "method": "reset_credit",
            "input": {
                "provider_config": { "api_key": "secret" },
                "operation": {
                    "type": "consume",
                    "idempotency_key": "attempt-123"
                }
            }
        })
    );

    let usage_result = ProviderUsageWindowsResult {
        windows: vec![ProviderUsageWindow {
            limit_window_seconds: 18_000,
            used_percent: 42.0,
            reset_at: Some("2026-08-20T10:00:00Z".to_string()),
        }],
        queried_at: "2026-08-20T05:00:00Z".to_string(),
    };
    let usage_payload = serde_json::to_value(usage_result).unwrap();
    assert_eq!(usage_payload["windows"][0]["limit_window_seconds"], 18_000);
    assert_eq!(usage_payload["windows"][0]["used_percent"], 42.0);

    let count = ProviderResetCreditResult::Count { available_count: 2 };
    assert_eq!(
        serde_json::to_value(count).unwrap(),
        json!({ "type": "count", "available_count": 2 })
    );
    assert_eq!(
        serde_json::to_value(ProviderResetCreditResult::Consumed).unwrap(),
        json!({ "type": "consumed" })
    );
}

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
fn provider_balance_result_serializes_deepseek_shape() {
    let result = ProviderBalanceResult {
        is_available: true,
        balance_infos: vec![ProviderBalanceInfo {
            currency: "CNY".to_string(),
            total_balance: "110.00".to_string(),
            granted_balance: Some("10.00".to_string()),
            topped_up_balance: Some("100.00".to_string()),
        }],
        provider_metadata: json!({ "provider": "deepseek" }),
    };

    let payload = serde_json::to_value(result).unwrap();

    assert_eq!(payload["is_available"], true);
    assert_eq!(payload["balance_infos"][0]["currency"], "CNY");
    assert_eq!(payload["balance_infos"][0]["total_balance"], "110.00");
    assert_eq!(payload["balance_infos"][0]["granted_balance"], "10.00");
    assert_eq!(payload["balance_infos"][0]["topped_up_balance"], "100.00");
    assert_eq!(payload["provider_metadata"]["provider"], "deepseek");
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

mod invocation;
