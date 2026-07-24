use std::collections::{BTreeMap, BTreeSet};

use plugin_framework::{
    installation::PluginTaskStatus,
    provider_contract::{
        semantic_required_capabilities, ClientProtocolEnvelope, ModelDiscoveryMode,
        NativeModelRequestContext, NativePromptBlock, NativePromptCacheControl,
        NativePromptCacheControlType, ProviderBalanceInfo, ProviderBalanceResult,
        ProviderCompactError, ProviderCompactProfile, ProviderCompactResult,
        ProviderCountTokensError, ProviderCountTokensInput, ProviderCountTokensResult,
        ProviderInvocationCapability, ProviderInvocationInput, ProviderInvocationResult,
        ProviderMessage, ProviderMessageRole, ProviderRuntimeError, ProviderRuntimeErrorKind,
        ProviderRuntimeLine, ProviderStdioMethod, ProviderStdioRequest, ProviderStdioResponse,
        ProviderStreamEvent, ProviderToolCall, ProviderUsage, ProviderWireOperation,
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
fn provider_invocation_input_preserves_tool_message_metadata() {
    let input = ProviderInvocationInput {
        previous_response_id: Some("resp_previous".to_string()),
        messages: vec![
            ProviderMessage {
                role: ProviderMessageRole::Assistant,
                content: String::new(),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: Some(json!([
                    {
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "lookup_order",
                            "arguments": "{\"order_id\":\"A-1\"}"
                        }
                    }
                ])),
                content_blocks: None,
            },
            ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: "{\"status\":\"shipped\"}".to_string(),
                name: None,
                tool_call_id: Some("call-1".to_string()),
                is_error: Some(true),
                tool_calls: None,
                content_blocks: None,
            },
        ],
        tools: vec![json!({
            "type": "function",
            "function": { "name": "lookup_order" }
        })],
        ..ProviderInvocationInput::default()
    };

    let payload = serde_json::to_value(input).unwrap();

    assert_eq!(payload["tools"][0]["function"]["name"], "lookup_order");
    assert_eq!(payload["previous_response_id"], "resp_previous");
    assert_eq!(payload["messages"][0]["tool_calls"][0]["id"], "call-1");
    assert_eq!(payload["messages"][1]["role"], "tool");
    assert_eq!(payload["messages"][1]["tool_call_id"], "call-1");
    assert_eq!(payload["messages"][1]["is_error"], true);
}

#[test]
fn provider_invocation_input_serializes_client_protocol_envelope() {
    let input = ProviderInvocationInput {
        client_protocol_envelope: Some(ClientProtocolEnvelope {
            source_protocol: "anthropic_messages".to_string(),
            policy: "anthropic_messages_v1".to_string(),
            headers: BTreeMap::from([
                ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ("anthropic-beta".to_string(), "prompt-caching".to_string()),
            ]),
        }),
        ..ProviderInvocationInput::default()
    };

    let payload = serde_json::to_value(input).unwrap();
    let decoded: ProviderInvocationInput = serde_json::from_value(payload.clone()).unwrap();

    assert_eq!(
        payload["client_protocol_envelope"]["source_protocol"],
        "anthropic_messages"
    );
    assert_eq!(
        payload["client_protocol_envelope"]["headers"]["anthropic-version"],
        "2023-06-01"
    );
    assert_eq!(
        decoded
            .client_protocol_envelope
            .as_ref()
            .unwrap()
            .headers
            .get("anthropic-beta")
            .map(String::as_str),
        Some("prompt-caching")
    );
}

#[test]
fn d4_ac_002_native_responses_passthrough_has_one_manifest_capability_name() {
    let capability = ProviderInvocationCapability::ResponsesNativePassthrough;

    assert_eq!(
        capability.manifest_capability_name(),
        "responses.native_passthrough"
    );
    assert_eq!(
        serde_json::to_value(capability).unwrap(),
        json!("responses.native_passthrough")
    );
}

#[test]
fn ac_002_current_provider_fails_closed_when_required_capability_is_missing() {
    let input = ProviderInvocationInput {
        system: vec![NativePromptBlock::Text {
            text: "Cache this block".to_string(),
            cache_control: Some(NativePromptCacheControl {
                cache_type: NativePromptCacheControlType::Ephemeral,
                ttl: None,
            }),
        }],
        request_context: NativeModelRequestContext {
            end_user_reference: Some("external-user-123".to_string()),
        },
        ..ProviderInvocationInput::default()
    };

    let error = input
        .to_current_provider_wire_value(&["system_prompt_blocks".to_string()])
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("system_prompt_cache_control"));
    assert!(message.contains("end_user_reference"));
}

#[test]
fn ac_003_semantic_requirements_expose_canonical_manifest_capability_names() {
    let required = semantic_required_capabilities(
        &[NativePromptBlock::Text {
            text: "Cache this block".to_string(),
            cache_control: Some(NativePromptCacheControl {
                cache_type: NativePromptCacheControlType::Ephemeral,
                ttl: None,
            }),
        }],
        &NativeModelRequestContext {
            end_user_reference: Some("external-user-123".to_string()),
        },
    );

    assert_eq!(
        required,
        BTreeSet::from([
            ProviderInvocationCapability::SystemPromptBlocks,
            ProviderInvocationCapability::SystemPromptCacheControl,
            ProviderInvocationCapability::EndUserReference,
        ])
    );
    assert_eq!(
        required
            .iter()
            .map(|capability| capability.manifest_capability_name())
            .collect::<Vec<_>>(),
        vec![
            "system_prompt_blocks",
            "system_prompt_cache_control",
            "end_user_reference",
        ]
    );
}

#[test]
fn c1_count_tokens_wire_is_tagged_and_requires_a_declared_capability() {
    let input = ProviderCountTokensInput {
        operation: ProviderWireOperation::CountTokens,
        contract_version: Default::default(),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "anthropic".to_string(),
        protocol: "anthropic_messages".to_string(),
        model: "claude-fixture".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "count this exact prompt".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderCountTokensInput::default()
    };

    let wire = input
        .to_current_provider_wire_value(&["count_tokens".to_string()])
        .expect("declared CountTokens capability should serialize the current typed operation");
    assert_eq!(wire["operation"], json!("count_tokens"));
    assert_eq!(wire["contract_version"], json!("1flowbase.provider/v2"));
    assert_eq!(
        wire["messages"][0]["content"],
        json!("count this exact prompt")
    );
    assert!(wire.get("tools").is_none());

    let result = serde_json::to_value(ProviderCountTokensResult {
        operation: ProviderWireOperation::CountTokens,
        input_tokens: 37,
    })
    .expect("typed CountTokens result should serialize");
    assert_eq!(
        result,
        json!({ "operation": "count_tokens", "input_tokens": 37 })
    );

    assert!(matches!(
        input.to_current_provider_wire_value(&[]),
        Err(ProviderCountTokensError::Unsupported { capabilities })
            if capabilities == vec!["count_tokens"]
    ));
}

#[test]
fn k2_compact_wire_derives_the_selected_profile_capability_and_closed_result_shape() {
    let input = ProviderInvocationInput {
        operation: ProviderWireOperation::Compact,
        profile: Some(ProviderCompactProfile::ResponsesCompactionV2),
        provider_instance_id: "provider-1".to_string(),
        provider_code: "openai".to_string(),
        protocol: "openai_responses".to_string(),
        model: "gpt-5.4-mini".to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "retain this turn".to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..ProviderInvocationInput::default()
    };

    let wire = input
        .to_current_provider_compact_wire_value(&["compact.responses_compaction_v2".to_string()])
        .expect("the selected Compact profile should require its exact manifest row");
    assert_eq!(wire["operation"], json!("compact"));
    assert_eq!(wire["profile"], json!("responses_compaction_v2"));
    assert_eq!(
        wire["required_capabilities"],
        json!(["compact.responses_compaction_v2"])
    );

    let result = serde_json::to_value(ProviderCompactResult::CompletedOpaqueCompactionItem {
        operation: ProviderWireOperation::Compact,
        profile: ProviderCompactProfile::ResponsesCompactionV2,
        response_id: Some("resp_compact".to_string()),
        compaction_item: json!({
            "type": "compaction",
            "encrypted_content": "opaque-byte-canary"
        }),
        encrypted_content: "opaque-byte-canary".to_string(),
    })
    .expect("the V2 Compact result should have one closed typed representation");
    assert_eq!(
        result,
        json!({
            "result_type": "completed_opaque_compaction_item",
            "operation": "compact",
            "profile": "responses_compaction_v2",
            "response_id": "resp_compact",
            "compaction_item": {
                "type": "compaction",
                "encrypted_content": "opaque-byte-canary"
            },
            "encrypted_content": "opaque-byte-canary"
        })
    );

    assert!(matches!(
        input.to_current_provider_compact_wire_value(&[]),
        Err(ProviderCompactError::Unsupported {
            profile: ProviderCompactProfile::ResponsesCompactionV2,
            capabilities,
        }) if capabilities == vec!["compact.responses_compaction_v2"]
    ));
}

#[test]
fn ac_002_current_generate_input_requires_explicit_contract_version() {
    let error = serde_json::from_value::<ProviderInvocationInput>(json!({
        "provider_instance_id": "provider-1",
        "provider_code": "anthropic",
        "protocol": "anthropic_messages",
        "model": "claude-fable-5",
        "messages": [{ "role": "user", "content": "hello" }],
        "system": []
    }))
    .unwrap_err();

    assert!(error.to_string().contains("contract_version"));
}

#[test]
fn ac_002_current_generate_input_strictly_rejects_legacy_and_unknown_fields() {
    let legacy_contract = serde_json::from_value::<ProviderInvocationInput>(json!({
        "contract_version": "1flowbase.provider/v1",
        "provider_instance_id": "provider-1",
        "provider_code": "legacy",
        "protocol": "openai_compatible",
        "model": "legacy-model",
        "messages": [],
        "system": "legacy system projection"
    }))
    .unwrap_err();
    assert!(legacy_contract
        .to_string()
        .contains("1flowbase.provider/v1"));

    let unknown_field = serde_json::from_value::<ProviderInvocationInput>(json!({
        "contract_version": "1flowbase.provider/v2",
        "provider_instance_id": "provider-1",
        "provider_code": "current",
        "protocol": "openai_compatible",
        "model": "current-model",
        "messages": [],
        "system": [],
        "raw_body": "must-not-be-accepted"
    }))
    .unwrap_err();
    assert!(unknown_field.to_string().contains("raw_body"));
}

#[test]
fn ac_005_wire_audit_is_bounded_and_excludes_raw_or_sensitive_values() {
    const SECRET_CANARY: &str = "secret-canary-do-not-audit";
    const PROMPT_CANARY: &str = "prompt-canary-do-not-audit";
    const RAW_CANARY: &str = "raw-body-canary-do-not-audit";

    let input = ProviderInvocationInput {
        provider_config: json!({
            "api_key": SECRET_CANARY,
            "raw_body": RAW_CANARY,
        }),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: PROMPT_CANARY.to_string(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        system: vec![NativePromptBlock::text(PROMPT_CANARY)],
        trace_context: BTreeMap::from([("secret-header".to_string(), SECRET_CANARY.to_string())]),
        run_context: BTreeMap::from([("raw_body".to_string(), json!(RAW_CANARY))]),
        ..ProviderInvocationInput::default()
    };

    let audit = input.wire_audit();
    let payload = serde_json::to_value(&audit).unwrap();
    let encoded = serde_json::to_string(&audit).unwrap();

    assert_eq!(payload["operation"], "generate");
    assert_eq!(payload["message_count"], 1);
    assert_eq!(payload["system_block_count"], 1);
    assert_eq!(payload["trace_context_entry_count"], 1);
    assert_eq!(payload["run_context_entry_count"], 1);
    assert!(!payload["counts_capped"].as_bool().unwrap());
    assert!(encoded.len() <= 1024, "wire audit must remain bounded");
    assert!(!encoded.contains(SECRET_CANARY));
    assert!(!encoded.contains(PROMPT_CANARY));
    assert!(!encoded.contains(RAW_CANARY));
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("raw_body"));
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
