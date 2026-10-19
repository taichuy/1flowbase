use super::*;

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

