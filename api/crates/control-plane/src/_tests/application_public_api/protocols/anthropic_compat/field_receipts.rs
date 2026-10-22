use super::*;

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

    let sentinel = "ANTHROPIC-INVALID-CONTENT-MUST-NOT-REACH-RECEIPT";
    let content_cases = [
        (
            json!({"type": "server_tool_use", "name": "computer", "input": sentinel}),
            "unsupported_feature",
            "messages content block server_tool_use is not supported by this endpoint",
            TranslationDecisionKind::Unsupported,
            "server-managed tool content has no current canonical owner",
        ),
        (
            json!({"type": "computer_use", "input": sentinel}),
            "unsupported_feature",
            "messages content block computer_use is not supported by this endpoint",
            TranslationDecisionKind::Unsupported,
            "server-managed tool content has no current canonical owner",
        ),
        (
            json!({"type": sentinel, "text": sentinel}),
            "invalid_request",
            "unknown Anthropic content block type",
            TranslationDecisionKind::Rejected,
            "unknown Anthropic content block type",
        ),
        (
            json!({"type": "tool_use", "name": "computer", "input": false, sentinel: sentinel}),
            "unsupported_feature",
            "computer_use is not supported by this endpoint",
            TranslationDecisionKind::Unsupported,
            "computer tool use has no current Native owner",
        ),
    ];

    for (block, error_type, message, kind, reason) in content_cases {
        for trailing_blocks in [vec![], vec![json!(false), json!({"type": "computer_use"})]] {
            let mut content = vec![block.clone()];
            content.extend(trailing_blocks);
            let error = translate_messages_request(json!({
                "model": "claude-compatible",
                "messages": [{"role": "user", "content": content}]
            }))
            .expect_err("the first invalid content block must determine the translation error");

            assert_eq!(error.error_type, error_type);
            assert_eq!(error.message, message);
            assert_eq!(
                serde_json::to_value(&error.report).expect("receipt should serialize"),
                json!({
                    "protocol": "anthropic_messages",
                    "decisions": [
                        {
                            "source_path": "$.model",
                            "target_path": "$.model",
                            "kind": "normalized",
                            "effective_value": "present"
                        },
                        {
                            "source_path": "$.system",
                            "target_path": "$.system",
                            "kind": "defaulted",
                            "reason": "no system prompt",
                            "effective_value": "defaulted"
                        },
                        {
                            "source_path": "$.messages[0]",
                            "target_path": "$.query,$.history",
                            "kind": "normalized",
                            "effective_value": "redacted"
                        },
                        {
                            "source_path": "$.messages[0].role",
                            "target_path": "$.query,$.history",
                            "kind": "normalized",
                            "effective_value": "present"
                        },
                        {
                            "source_path": "$.messages[0].content[0]",
                            "target_path": "$.query,$.history",
                            "kind": "normalized",
                            "effective_value": "redacted"
                        },
                        {
                            "source_path": "$.messages[0].content[0].type",
                            "kind": kind,
                            "reason": reason,
                            "effective_value": "present"
                        },
                        {
                            "source_path": "$.messages[0].content",
                            "kind": "rejected",
                            "reason": "Anthropic message content contains invalid blocks",
                            "effective_value": "present"
                        },
                        {
                            "source_path": "$.messages",
                            "kind": "rejected",
                            "reason": "Anthropic messages contain an invalid message",
                            "effective_value": "present"
                        }
                    ]
                }),
                "content rejection receipts must stay ordered and redacted"
            );
        }
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
