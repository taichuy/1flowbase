use control_plane::application_public_api::compat::anthropic::{
    map_messages_request, translate_messages_request, AnthropicCompatError,
};
use control_plane::application_public_api::{
    mapping::ApplicationApiMappingConfig,
    native::NativeInputMapper,
    protocol_translation::{TranslationDecisionKind, TranslationSafeRepresentation},
};
use plugin_framework::provider_contract::NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY;
use serde_json::{json, Value};

fn base_request() -> Value {
    json!({
        "model": "claude-compatible-custom",
        "max_tokens": 512,
        "messages": [
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ]
    })
}

fn assert_unsupported_feature(request: Value, path: &str) {
    let error = translate_messages_request(request).unwrap_err();

    assert_anthropic_unsupported_feature(error.clone());
    assert!(error
        .report
        .has_decision(path, TranslationDecisionKind::Unsupported));
}

fn assert_anthropic_unsupported_feature(error: AnthropicCompatError) {
    assert_eq!(error.error_type, "unsupported_feature");
    assert!(error.message.contains("is not supported by this endpoint"));
}

#[test]
fn context_management_is_preserved_only_in_the_anthropic_protocol_context_residual() {
    let mut request = base_request();
    request["context_management"] = json!({
        "edits": [{"type": "clear_thinking_20251015"}]
    });

    let translated = translate_messages_request(request)
        .expect("context management is an optional context optimization");

    assert!(translated
        .report
        .has_decision("$.context_management", TranslationDecisionKind::Exact));
    let envelope = translated
        .request
        .client_protocol_envelope
        .expect("Anthropic context management should have one protocol residual");
    assert_eq!(
        envelope.body["context_management"]["edits"][0]["type"],
        "clear_thinking_20251015"
    );
    for typed in ["model", "messages", "max_tokens"] {
        assert!(!envelope.body.contains_key(typed), "{typed}");
    }
}

#[test]
fn system_maps_to_native_system_context() {
    let mut request = base_request();
    request["system"] = json!("Use the support playbook.");

    let native = map_messages_request(request).unwrap();

    assert_eq!(
        native.system_text().as_deref(),
        Some("Use the support playbook.")
    );
    assert_eq!(
        native.history,
        vec![
            json!({"role": "user", "content": "Earlier question"}),
            json!({"role": "assistant", "content": "Earlier answer"})
        ]
    );
}

#[test]
fn system_message_role_maps_to_native_system_history() {
    let mut request = base_request();
    request["messages"] = json!([
        {"role": "system", "content": "Use Claude Code tools carefully."},
        {"role": "user", "content": "Continue"}
    ]);

    let native = map_messages_request(request).expect("system messages have a Native owner");

    assert_eq!(native.history[0]["role"], "system");
    assert_eq!(
        native.history[0]["content"],
        "Use Claude Code tools carefully."
    );
}

#[test]
fn ac_002_anthropic_system_blocks_and_end_user_reference_become_native_truth() {
    let mut request = base_request();
    request["system"] = json!([
        {
            "type": "text",
            "text": "Use Claude Code project instructions.",
            "cache_control": { "type": "ephemeral", "ttl": "1h" }
        },
        {
            "type": "text",
            "text": "Preserve repository safety rules."
        }
    ]);
    request["metadata"] = json!({
        "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
    });

    let native = map_messages_request(request).unwrap();
    let payload = serde_json::to_value(&native).unwrap();
    let mapped =
        NativeInputMapper::map(&native, &ApplicationApiMappingConfig::default_native()).unwrap();

    assert_eq!(
        payload["system"],
        json!([
            {
                "type": "text",
                "text": "Use Claude Code project instructions.",
                "cache_control": { "type": "ephemeral", "ttl": "1h" }
            },
            {
                "type": "text",
                "text": "Preserve repository safety rules."
            }
        ])
    );
    assert_eq!(
        payload["request_context"]["end_user_reference"],
        json!("user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f")
    );
    assert!(payload["metadata"].get("user_id").is_none());
    assert_eq!(
        mapped.node_input_payload[NATIVE_MODEL_REQUEST_CONTEXT_PAYLOAD_KEY]["end_user_reference"],
        json!("user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f")
    );
    assert_eq!(
        mapped.node_input_payload["node-start"]["system"],
        payload["system"]
    );
}

#[test]
fn last_user_text_maps_to_native_query() {
    let native = map_messages_request(base_request()).unwrap();

    assert_eq!(native.query, "Final question");
}

#[test]
fn ac_003_anthropic_max_tokens_maps_to_native_max_output_tokens() {
    let native = map_messages_request(base_request()).unwrap();

    assert_eq!(
        serde_json::to_value(&native).unwrap()["execution"]["model_parameters"]
            ["max_output_tokens"],
        json!(512)
    );
}

#[test]
fn ac_005_anthropic_adaptive_thinking_maps_to_native_reasoning() {
    let mut request = base_request();
    request["thinking"] = json!({"type": "adaptive", "display": "omitted"});

    let translated =
        translate_messages_request(request).expect("adaptive thinking should map to Native");
    let execution =
        serde_json::to_value(translated.request.execution).expect("execution should serialize");

    assert_eq!(
        execution["model_parameters"]["reasoning"]["mode"],
        json!("adaptive")
    );
    assert!(translated
        .report
        .has_decision("$.thinking.display", TranslationDecisionKind::Dropped));
}

#[test]
fn wp_d2a_anthropic_adaptive_max_reasoning_has_one_typed_owner() {
    let mut request = base_request();
    request["thinking"] = json!({"type": "adaptive"});
    request["output_config"] = json!({"effort": "max"});

    let native = map_messages_request(request).expect("output effort should map to Native");
    let execution = serde_json::to_value(native.execution).expect("execution should serialize");

    assert_eq!(
        execution["model_parameters"]["reasoning"]["mode"],
        json!("adaptive")
    );
    assert_eq!(
        execution["model_parameters"]["reasoning"]["effort"],
        json!("max")
    );
    assert!(native.client_protocol_envelope.is_none());
}

#[test]
fn prior_messages_map_to_native_history() {
    let native = map_messages_request(base_request()).unwrap();

    assert_eq!(
        native.history,
        vec![
            json!({"role": "user", "content": "Earlier question"}),
            json!({"role": "assistant", "content": "Earlier answer"})
        ]
    );
}

#[test]
fn stream_true_maps_to_native_streaming_response_mode() {
    let mut request = base_request();
    request["stream"] = json!(true);

    let native = map_messages_request(request).unwrap();

    assert_eq!(native.response_mode.as_deref(), Some("streaming"));
}

#[test]
fn metadata_expand_id_maps_to_native_conversation_user() {
    let mut request = base_request();
    request["metadata"] = json!({
        "expand_id": "external-user-123"
    });

    let native = map_messages_request(request).unwrap();

    assert_eq!(
        native.conversation.get("user"),
        Some(&json!("external-user-123"))
    );
}

#[test]
fn metadata_user_id_json_maps_session_to_native_conversation() {
    let mut request = base_request();
    request["metadata"] = json!({
        "user_id": "{\"device_id\":\"device-123\",\"account_uuid\":\"\",\"session_id\":\"session-456\"}"
    });

    let native = map_messages_request(request).unwrap();

    assert_eq!(native.conversation.get("user"), Some(&json!("device-123")));
    assert_eq!(native.conversation.get("id"), Some(&json!("session-456")));
}

#[test]
fn metadata_plain_user_id_session_suffix_maps_session_to_native_conversation() {
    let mut request = base_request();
    request["metadata"] = json!({
        "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
    });

    let native = map_messages_request(request).unwrap();

    assert_eq!(
        native.conversation.get("user"),
        Some(&json!(
            "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        ))
    );
    assert_eq!(
        native.conversation.get("id"),
        Some(&json!("3e7058c2-3120-4222-bb14-c99ec85e1c0f"))
    );
}

#[test]
fn metadata_session_id_maps_to_native_conversation_id() {
    let mut request = base_request();
    request["metadata"] = json!({
        "expand_id": "external-user-123",
        "session_id": "header-session-789"
    });

    let native = map_messages_request(request).unwrap();

    assert_eq!(
        native.conversation.get("user"),
        Some(&json!("external-user-123"))
    );
    assert_eq!(
        native.conversation.get("id"),
        Some(&json!("header-session-789"))
    );
}

#[test]
fn model_maps_exactly_without_validation() {
    let mut request = base_request();
    request["model"] = json!("unregistered/anthropic:model.with/slashes");

    let native = map_messages_request(request).unwrap();

    assert_eq!(
        native.model.as_deref(),
        Some("unregistered/anthropic:model.with/slashes")
    );
}

#[test]
fn wp_d2a_one_m_model_suffix_maps_to_typed_requested_context_only() {
    let mut request = base_request();
    request["model"] = json!("claude-opus-4-8[1M]");

    let native = map_messages_request(request).unwrap();

    assert_eq!(native.model.as_deref(), Some("claude-opus-4-8"));
    assert_eq!(
        serde_json::to_value(&native).unwrap()["execution"]["model_parameters"]
            ["requested_context_window"],
        json!(1_000_000)
    );
    assert!(native.client_protocol_envelope.is_none());
}

#[test]
fn ac_001_anthropic_tools_and_choice_map_to_native_inputs() {
    let mut request = base_request();
    request["tools"] = json!([
        {
            "name": "lookup_order",
            "description": "Find an order",
            "input_schema": {"type": "object"}
        }
    ]);
    request["tool_choice"] = json!({
        "type": "tool",
        "name": "lookup_order"
    });

    let native = map_messages_request(request).expect("Anthropic tools should map to Native");

    assert_eq!(native.inputs.as_value()["tools"][0]["name"], "lookup_order");
    assert_eq!(
        native.inputs.as_value()["tools"][0]["source"],
        "anthropic_compatible"
    );
    assert_eq!(
        native.inputs.as_value()["tool_choice"],
        json!({"name": "lookup_order"})
    );
}

#[test]
fn ac_002_anthropic_tool_blocks_map_to_native_history() {
    let mut request = base_request();
    request["messages"] = json!([
        {"role": "user", "content": "Find order"},
        {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_123",
                    "name": "lookup_order",
                    "input": {"order_id": "order_123"}
                }
            ]
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "toolu_123",
                    "content": "Order found",
                    "cache_control": {"type": "ephemeral", "ttl": "5m"}
                }
            ]
        }
    ]);

    let native = map_messages_request(request).expect("tool history should map to Native");

    assert_eq!(native.history[1]["role"], "assistant");
    assert_eq!(native.history[1]["tool_calls"][0]["id"], "toolu_123");
    assert_eq!(
        native.history[1]["tool_calls"][0]["arguments"]["order_id"],
        "order_123"
    );
    assert_eq!(native.query, "Order found");
    assert_eq!(native.inputs.as_value(), json!({}));
}

#[test]
fn last_user_multimodal_content_maps_query_text_and_preserves_media_blocks() {
    let native = map_messages_request(json!({
        "model": "claude-compatible-custom",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe this image"},
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }
                ]
            }
        ]
    }))
    .unwrap();

    assert_eq!(native.query, "Describe this image");
    assert_eq!(native.history.len(), 1);
    assert_eq!(native.history[0]["role"], json!("user"));
    assert_eq!(native.history[0]["content"], json!(""));
    assert_eq!(
        native.history[0]["content_blocks"][0]["type"],
        json!("image")
    );
    assert_eq!(
        native.history[0]["content_blocks"][0]["source"]["media_type"],
        json!("image/png")
    );
}

#[test]
fn d2_ac_001_anthropic_text_block_unknown_field_is_rejected_with_its_own_receipt() {
    let error = translate_messages_request(json!({
        "model": "claude-compatible-custom",
        "messages": [{
            "role": "user",
            "content": [{
                "type": "text",
                "text": "hello",
                "unexpected": "must not reach canonical history"
            }]
        }]
    }))
    .expect_err("unknown Anthropic text-block fields must be rejected");

    assert!(error.report.has_decision(
        "$.messages[0].content[0].<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn d2_ac_001_anthropic_required_and_nested_fields_have_one_safe_rejection_receipt() {
    let sentinel = "D2-ANTHROPIC-RAW-METADATA-MUST-NOT-REACH-RECEIPT";
    let cases = [
        (
            translate_messages_request(json!({
                "messages": [{"role": "user", "content": "hello"}]
            }))
            .expect_err("missing Anthropic model must be decided before rejection"),
            "$.model",
        ),
        (
            translate_messages_request(json!({
                "model": "claude-compatible"
            }))
            .expect_err("missing Anthropic messages must be decided before rejection"),
            "$.messages",
        ),
        (
            translate_messages_request(json!({
                "model": "claude-compatible",
                "messages": [{"role": "user", "content": "hello"}],
                "metadata": {"raw_provider_body": sentinel}
            }))
            .expect_err("nested Anthropic metadata must not be copied into the Native request"),
            "$.metadata.<unknown>[0]",
        ),
    ];

    for (error, source_path) in cases {
        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(
            decisions.len(),
            1,
            "{source_path} must have exactly one TranslationDecision"
        );
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
        assert!(
            !serde_json::to_string(&error.report)
                .expect("receipt should serialize")
                .contains(sentinel),
            "receipt must not retain the raw nested sentinel"
        );
    }
}

#[test]
fn d2_ac_001_anthropic_required_malformed_values_remain_present_in_the_receipt() {
    let malformed_cases = [
        (
            translate_messages_request(json!({
                "model": false,
                "messages": [{"role": "user", "content": "hello"}]
            }))
            .expect_err("a malformed Anthropic model must be rejected"),
            "$.model",
        ),
        (
            translate_messages_request(json!({
                "model": "claude-compatible",
                "messages": {}
            }))
            .expect_err("a malformed Anthropic messages value must be rejected"),
            "$.messages",
        ),
    ];
    for (error, source_path) in malformed_cases {
        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
        assert_eq!(
            decisions[0].effective_value,
            TranslationSafeRepresentation::Present,
            "{source_path} is malformed but present, not missing"
        );
    }

    let missing_model = translate_messages_request(json!({
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .expect_err("a missing Anthropic model must be rejected");
    let decision = missing_model
        .report
        .decisions
        .iter()
        .find(|decision| decision.source_path == "$.model")
        .expect("the missing model needs a receipt");
    assert_eq!(
        decision.effective_value,
        TranslationSafeRepresentation::Absent
    );
}

#[test]
fn d2_ac_001_anthropic_invalid_message_shapes_have_field_receipts() {
    let cases = [
        (
            translate_messages_request(json!({
                "model": "claude-compatible",
                "messages": [false]
            }))
            .expect_err("a non-object Anthropic message must be rejected at its item path"),
            "$.messages[0]",
        ),
        (
            translate_messages_request(json!({
                "model": "claude-compatible",
                "messages": [{"role": false, "content": "hello"}]
            }))
            .expect_err("a non-text Anthropic role must be rejected at its field path"),
            "$.messages[0].role",
        ),
        (
            translate_messages_request(json!({
                "model": "claude-compatible",
                "messages": [{"role": "user", "content": [false]}]
            }))
            .expect_err("a non-object Anthropic content block must be rejected at its item path"),
            "$.messages[0].content[0]",
        ),
    ];

    for (error, source_path) in cases {
        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one decision");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }
}

#[test]
fn d2_ac_001_anthropic_media_source_unknown_field_is_rejected_before_canonical_history() {
    let error = translate_messages_request(json!({
        "model": "claude-compatible-custom",
        "messages": [{
            "role": "user",
            "content": [{
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "aW1hZ2U=",
                    "unexpected": "raw compat payload"
                }
            }]
        }]
    }))
    .expect_err("unknown Anthropic media source fields must be rejected");

    assert!(error.report.has_decision(
        "$.messages[0].content[0].source.<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn d2_ac_001_anthropic_cache_control_unknown_field_is_rejected_with_its_own_receipt() {
    let error = translate_messages_request(json!({
        "model": "claude-compatible-custom",
        "system": [{
            "type": "text",
            "text": "Use the support playbook.",
            "cache_control": {
                "type": "ephemeral",
                "unexpected": "raw compat payload"
            }
        }],
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .expect_err("unknown cache-control fields must be rejected");

    assert!(error.report.has_decision(
        "$.system[0].cache_control.<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn anthropic_message_cache_control_is_dropped_while_content_is_retained() {
    let cases = [
        json!({
            "model": "claude-compatible",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": "hello",
                    "cache_control": {"type": "ephemeral", "ttl": "5m"}
                }]
            }]
        }),
        json!({
            "model": "claude-compatible",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "aW1hZ2U="
                    },
                    "cache_control": {"type": "ephemeral", "ttl": "5m"}
                }]
            }]
        }),
        json!({
            "model": "claude-compatible",
            "messages": [{
                "role": "user",
                "content": [{
                    "type": "document",
                    "source": {
                        "type": "base64",
                        "media_type": "application/pdf",
                        "data": "ZG9jdW1lbnQ="
                    },
                    "cache_control": {"type": "ephemeral", "ttl": "5m"}
                }]
            }]
        }),
    ];

    for request in cases {
        let translated = translate_messages_request(request)
            .expect("message cache control is an optional transport hint");

        for suffix in ["cache_control", "cache_control.type", "cache_control.ttl"] {
            let source_path = format!("$.messages[0].content[0].{suffix}");
            let decisions = translated
                .report
                .decisions
                .iter()
                .filter(|decision| decision.source_path == source_path)
                .collect::<Vec<_>>();
            assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
            assert_eq!(decisions[0].kind, TranslationDecisionKind::Dropped);
        }
        assert!(
            translated
                .report
                .decisions
                .iter()
                .any(|decision| decision.kind == TranslationDecisionKind::Dropped),
            "cache hints must be explicitly receipted as dropped"
        );
    }
}

#[test]
fn d2_ac_001_anthropic_content_and_source_type_receipts_preserve_wire_presence() {
    let cases = [
        (
            json!({
                "model": "claude-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{"text": "hello"}]
                }]
            }),
            "$.messages[0].content[0].type",
            TranslationSafeRepresentation::Absent,
        ),
        (
            json!({
                "model": "claude-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{"type": false, "text": "hello"}]
                }]
            }),
            "$.messages[0].content[0].type",
            TranslationSafeRepresentation::Present,
        ),
        (
            json!({
                "model": "claude-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "image",
                        "source": {
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }]
                }]
            }),
            "$.messages[0].content[0].source.type",
            TranslationSafeRepresentation::Absent,
        ),
        (
            json!({
                "model": "claude-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "image",
                        "source": {
                            "type": false,
                            "media_type": "image/png",
                            "data": "aW1hZ2U="
                        }
                    }]
                }]
            }),
            "$.messages[0].content[0].source.type",
            TranslationSafeRepresentation::Present,
        ),
    ];

    for (request, source_path, effective_value) in cases {
        let error = translate_messages_request(request)
            .expect_err("invalid Anthropic nested type fields must be rejected");
        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
        assert_eq!(decisions[0].effective_value, effective_value);
    }
}

#[test]
fn d2_ac_001_anthropic_metadata_normalization_is_receipted_without_changing_conversation() {
    let identity = "  {\"account_uuid\":\" account-42 \",\"session_id\":\" 3e7058c2-3120-4222-bb14-c99ec85e1c0f \"}  ";
    let identity_translated = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        "metadata": {"user_id": identity}
    }))
    .expect("a Claude Code identity string should retain the current canonical derivation");

    assert_eq!(
        identity_translated
            .request
            .request_context
            .end_user_reference
            .as_deref(),
        Some(identity.trim())
    );
    assert_eq!(
        identity_translated.request.conversation["user"],
        json!("account-42")
    );
    assert_eq!(
        identity_translated.request.conversation["id"],
        json!("3e7058c2-3120-4222-bb14-c99ec85e1c0f")
    );
    assert!(identity_translated
        .report
        .has_decision("$.metadata.user_id", TranslationDecisionKind::Normalized));

    let trimmed_translated = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        "metadata": {
            "user_id": "  source-user  ",
            "expand_id": "  external-user  ",
            "session_id": "  session-42  "
        }
    }))
    .expect("trimmed Anthropic metadata should retain the current canonical mapping");

    assert_eq!(
        trimmed_translated
            .request
            .request_context
            .end_user_reference
            .as_deref(),
        Some("source-user")
    );
    assert_eq!(
        trimmed_translated.request.conversation["user"],
        json!("external-user")
    );
    assert_eq!(
        trimmed_translated.request.conversation["id"],
        json!("session-42")
    );
    for source_path in [
        "$.metadata.user_id",
        "$.metadata.expand_id",
        "$.metadata.session_id",
    ] {
        assert!(
            trimmed_translated
                .report
                .has_decision(source_path, TranslationDecisionKind::Normalized),
            "{source_path} must record its normalization"
        );
    }
}

#[test]
fn d2_ac_001_anthropic_marker_rejection_replaces_preliminary_decision_for_the_same_path() {
    let error = translate_messages_request(json!({
        "model": "claude-compatible-custom",
        "messages": [{
            "role": "user",
            "content": "Your task is to create a detailed summary of the conversation so far"
        }]
    }))
    .expect_err("Claude Code control markers have no canonical owner");

    let decisions = error
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.messages[0].content")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "a source path has one final decision");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Unsupported);
}

#[test]
fn mixed_tool_result_and_text_maps_visible_text_to_native_query() {
    let request = json!({
        "model": "claude-compatible-custom",
        "messages": [
            {"role": "user", "content": "uploads/agent-flow-preview-debug.png 描述一下这幅图说什么？"},
            {
                "role": "assistant",
                "content": [
                    {
                        "type": "tool_use",
                        "id": "toolu_read",
                        "name": "Read",
                        "input": {"file_path": "uploads/agent-flow-preview-debug.png"}
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_read",
                        "content": "<tool_use_error>old tool payload</tool_use_error>\nold image output"
                    },
                    {"type": "text", "text": "帮我找找这个代码位置"}
                ]
            }
        ]
    });

    let translated = translate_messages_request(request)
        .expect("mixed tool result and visible text has a Native representation");
    assert_eq!(translated.request.query, "帮我找找这个代码位置");
    assert_eq!(translated.request.history[1]["role"], "assistant");
    assert_eq!(
        translated.request.history[1]["tool_calls"][0]["id"],
        "toolu_read"
    );
}

#[test]
fn ac_006_thinking_history_maps_to_native_reasoning_content_blocks() {
    let request = json!({
        "model": "claude-compatible-custom",
        "messages": [
            {"role": "user", "content": "hi ?"},
            {
                "role": "assistant",
                "content": [
                    {"type": "thinking", "thinking": "internal reasoning", "signature": ""}
                ]
            },
            {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Hello!"}
                ]
            },
            {"role": "user", "content": "next question"}
        ]
    });

    let translated = translate_messages_request(request)
        .expect("assistant thinking history has a Native reasoning owner");

    assert_eq!(
        translated.request.history[1]["content_blocks"][0],
        json!({
            "type": "reasoning",
            "text": "internal reasoning",
            "signature": ""
        })
    );
    assert!(translated.report.has_decision(
        "$.messages[1].content[0].type",
        TranslationDecisionKind::Normalized,
    ));
}

#[test]
fn d2_ac_007_claude_code_compact_summary_marker_is_unsupported() {
    let request = json!({
        "model": "claude-compatible-custom",
        "metadata": {
            "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "messages": [
            {"role": "user", "content": "hi ?"},
            {
                "role": "user",
                "content": "CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.\n\nYour task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions.\n\nIMPORTANT: Do NOT use any tools. You MUST respond with ONLY the <summary>...</summary> block as your text output."
            }
        ]
    });

    assert_unsupported_feature(request, "$.messages[1].content");
}

#[test]
fn d2_ac_007_claude_code_title_marker_is_unsupported() {
    let request = json!({
        "model": "claude-compatible-custom",
        "system": "x-anthropic-billing-header: cc_version=2.1.141.831; cc_entrypoint=cli; cch=a143a;\n\nYou are Claude Code, Anthropic's official CLI for Claude.\n\nGenerate a concise, sentence-case title (3-7 words) that captures the main topic or goal of this coding session. Return JSON with a single \"title\" field.",
        "metadata": {
            "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "messages": [
            {"role": "user", "content": "uploads/image-1.png 帮我看看这导航栏代码是在哪来的？"}
        ]
    });

    assert_unsupported_feature(request, "$.system");
}

#[test]
fn d2_ac_007_claude_code_away_summary_marker_is_unsupported() {
    let request = json!({
        "model": "claude-compatible-custom",
        "metadata": {
            "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "messages": [
            {
                "role": "user",
                "content": "The user stepped away and is coming back. Write exactly 1-3 short sentences. Start by stating the high-level task — what they are building or debugging, not implementation details. Next: the concrete next step. Skip status reports and commit recaps."
            }
        ]
    });

    assert_unsupported_feature(request, "$.messages[0].content");
}

#[test]
fn d2_ac_007_claude_code_compact_resume_marker_is_unsupported() {
    let request = json!({
        "model": "claude-compatible-custom",
        "metadata": {
            "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "messages": [
            {
                "role": "user",
                "content": "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n- user asked where uploads/image-1.png is implemented\n\nContinue the conversation from where it left off without asking the user any further questions. Resume directly — do not acknowledge the summary, do not recap what was happening, do not preface with \"I'll continue\" or similar. Pick up the last task as if the break never happened."
            }
        ]
    });

    assert_unsupported_feature(request, "$.messages[0].content");
}

#[test]
fn d2_ac_007_claude_code_compact_resume_history_marker_is_unsupported() {
    let request = json!({
        "model": "claude-compatible-custom",
        "metadata": {
            "user_id": "user_31fb5a_account__session_3e7058c2-3120-4222-bb14-c99ec85e1c0f"
        },
        "messages": [
            {
                "role": "user",
                "content": "This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n- user said hi\n\nIf you need specific details from before compaction (like exact code snippets, error messages, or content you generated), read the full transcript at: C:\\Users\\Lw\\.claude\\projects\\repo\\session.jsonl\nPlease continue the conversation from where we left off without asking the user any further questions."
            },
            {"role": "assistant", "content": "已恢复上下文。"},
            {"role": "user", "content": "那你帮我拉一下最新代码"}
        ]
    });

    assert_unsupported_feature(request, "$.messages[0].content");
}

#[test]
fn computer_use_returns_unsupported_feature() {
    let mut request = base_request();
    request["messages"] = json!([
        {
            "role": "assistant",
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_computer",
                    "name": "computer",
                    "input": {"action": "screenshot"}
                }
            ]
        },
        {"role": "user", "content": "What is on screen?"}
    ]);

    assert_unsupported_feature(request, "$.messages[0].content[0].type");
}

#[test]
fn d2_f1_anthropic_nested_failures_reject_system_and_messages_containers() {
    let system_error = translate_messages_request(json!({
        "model": "claude-compatible",
        "system": [{"type": "text", "text": false}],
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .expect_err("malformed Anthropic system text must reject the system container");
    for source_path in ["$.system", "$.system[0].text"] {
        let decisions = system_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }
    let text = system_error
        .report
        .decisions
        .iter()
        .find(|decision| decision.source_path == "$.system[0].text")
        .expect("malformed text decision exists");
    assert_eq!(text.effective_value, TranslationSafeRepresentation::Present);

    let message_error = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{"role": "user", "content": false}]
    }))
    .expect_err("malformed Anthropic content must reject the messages container");
    for source_path in ["$.messages", "$.messages[0].content"] {
        let decisions = message_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }
}

#[test]
fn d2_f1_anthropic_unknown_defined_keys_are_anonymous_and_preserved_as_residuals() {
    let alpha = "D2-F1-ANTHROPIC-UNKNOWN-KEY-ALPHA";
    let beta = "D2-F1-ANTHROPIC-UNKNOWN-KEY-BETA";
    let translated = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        alpha: true,
        beta: false
    }))
    .expect("safe unknown Anthropic roots should remain protocol-authentic residuals");
    let unknown_paths = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path.starts_with("$.<unknown>"))
        .map(|decision| decision.source_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(unknown_paths, ["$.<unknown>[0]", "$.<unknown>[1]"]);
    assert!(translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path.starts_with("$.<unknown>"))
        .all(|decision| decision.kind == TranslationDecisionKind::Exact));
    let serialized = serde_json::to_string(&translated.report).expect("receipt serializes");
    assert!(!serialized.contains(alpha));
    assert!(!serialized.contains(beta));
    let envelope = translated
        .request
        .client_protocol_envelope
        .expect("unknown Anthropic roots should create one residual envelope");
    assert_eq!(envelope.body[alpha], true);
    assert_eq!(envelope.body[beta], false);
}

#[test]
fn d2_f1_anthropic_defined_container_receipts_remain_unique_on_nested_rejection() {
    let message_error = translate_messages_request(json!({
        "model": "claude-compatible",
        "messages": [{
            "role": "user",
            "content": [{
                "type": "image",
                "source": {"type": "base64", "media_type": false, "data": "aW1hZ2U="}
            }]
        }]
    }))
    .expect_err("a malformed Anthropic media field retains all defined container receipts");
    for (source_path, kind) in [
        ("$.messages", TranslationDecisionKind::Rejected),
        ("$.messages[0]", TranslationDecisionKind::Normalized),
        ("$.messages[0].content", TranslationDecisionKind::Rejected),
        (
            "$.messages[0].content[0]",
            TranslationDecisionKind::Normalized,
        ),
        (
            "$.messages[0].content[0].source",
            TranslationDecisionKind::Normalized,
        ),
        (
            "$.messages[0].content[0].source.media_type",
            TranslationDecisionKind::Rejected,
        ),
    ] {
        let decisions = message_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, kind);
    }

    let system_error = translate_messages_request(json!({
        "model": "claude-compatible",
        "system": [{
            "type": "text",
            "text": "Use the runbook.",
            "cache_control": {"type": false}
        }],
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .expect_err("a malformed system cache retains all defined system container receipts");
    for (source_path, kind) in [
        ("$.system", TranslationDecisionKind::Rejected),
        ("$.system[0]", TranslationDecisionKind::Normalized),
        ("$.system[0].cache_control", TranslationDecisionKind::Exact),
        (
            "$.system[0].cache_control.type",
            TranslationDecisionKind::Rejected,
        ),
    ] {
        let decisions = system_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, kind);
    }
}
