use control_plane::application_public_api::compat::openai::{
    map_chat_completion_request, map_response_request, translate_chat_completion_request,
    translate_response_request, translate_response_request_with_context, OpenAiCompatError,
    OpenAiResponsesEndpoint, OpenAiResponsesRequestContext,
};
use control_plane::application_public_api::native::{
    compaction_intent, CompactionProfile, CompactionResultRequirement,
};
use control_plane::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationSafeRepresentation,
};
use domain::{AiNativeCompactProfile, AiNativeGenerateProfile, AiNativeOperation};
use serde_json::{json, Value};

fn base_request() -> Value {
    json!({
        "model": "provider/custom-model",
        "messages": [
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ]
    })
}

fn codex_compaction_metadata(implementation: &str) -> Value {
    json!({
        "request_kind": "compaction",
        "compaction": {
            "trigger": "manual",
            "reason": "user_requested",
            "implementation": implementation,
            "phase": "standalone_turn",
            "strategy": "memento"
        }
    })
}

fn responses_request(input: Value) -> Value {
    json!({
        "model": "gpt-compatible",
        "input": input
    })
}

fn assert_compaction_intent(
    request: &control_plane::application_public_api::native::NativeRunRequest,
    profile: CompactionProfile,
    result_requirement: CompactionResultRequirement,
) {
    let intent = compaction_intent(*request.execution.execution_operation())
        .expect("Codex compaction evidence must select a compaction intent");
    assert_eq!(intent.profile(), profile);
    assert_eq!(intent.result_requirement(), result_requirement);
}

fn assert_unsupported_feature(request: Value, param: &str) {
    let error = translate_chat_completion_request(request).unwrap_err();

    assert_openai_unsupported_feature(error.clone(), param);
    assert!(error
        .report
        .has_decision(&format!("$.{param}"), TranslationDecisionKind::Unsupported));
}

fn assert_openai_unsupported_feature(error: OpenAiCompatError, param: &str) {
    assert_eq!(error.error_type, "invalid_request_error");
    assert_eq!(error.code, "unsupported_feature");
    assert_eq!(error.param.as_deref(), Some(param));
    assert_eq!(
        error.message,
        format!("{param} is not supported by this endpoint")
    );
}

#[test]
fn d2_ac_001_chat_translation_receipt_decides_each_safe_field_without_prompt_copy() {
    let sentinel_prompt = "D2-SENTINEL-PROMPT-MUST-NOT-REACH-RECEIPT";
    let translated = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{"role": "user", "content": sentinel_prompt}],
        "max_completion_tokens": 512,
        "stream": false
    }))
    .expect("supported Chat fields should translate");

    assert_eq!(translated.request.query, sentinel_prompt);
    assert!(translated
        .report
        .has_decision("$.model", TranslationDecisionKind::Exact));
    assert!(translated
        .report
        .has_decision("$.messages[0].content", TranslationDecisionKind::Normalized));
    assert!(translated.report.has_decision(
        "$.max_completion_tokens",
        TranslationDecisionKind::Normalized
    ));
    assert!(translated
        .report
        .has_decision("$.stream", TranslationDecisionKind::Normalized));
    assert!(!serde_json::to_string(&translated.report)
        .expect("receipt should serialize")
        .contains(sentinel_prompt));
}

#[test]
fn d2_ac_001_openai_required_and_nested_fields_have_one_safe_rejection_receipt() {
    let sentinel = "D2-OPENAI-RAW-METADATA-MUST-NOT-REACH-RECEIPT";
    let cases = [
        (
            translate_chat_completion_request(json!({
                "messages": [{"role": "user", "content": "hello"}]
            }))
            .expect_err("missing Chat model must be decided before rejection"),
            "$.model",
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible"
            }))
            .expect_err("missing Chat messages must be decided before rejection"),
            "$.messages",
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible"
            }))
            .expect_err("missing Responses input must be decided before rejection"),
            "$.input",
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [{"role": "user", "content": "hello"}],
                "user": {"raw": sentinel}
            }))
            .expect_err("non-text OpenAI user must not be silently discarded"),
            "$.user",
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": "hello",
                "metadata": {"raw_provider_body": sentinel}
            }))
            .expect_err("nested OpenAI metadata must not be copied into the Native request"),
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
fn d2_ac_001_openai_required_malformed_values_remain_present_in_the_receipt() {
    let malformed_cases = [
        (
            translate_response_request(json!({ "model": false, "input": "hello" }))
                .expect_err("a malformed OpenAI model must be rejected"),
            "$.model",
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": {}
            }))
            .expect_err("a malformed OpenAI messages value must be rejected"),
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

    let missing_model = translate_response_request(json!({ "input": "hello" }))
        .expect_err("a missing OpenAI model must be rejected");
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
fn d2_ac_001_openai_invalid_message_shapes_have_field_receipts() {
    let cases = [
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [false]
            }))
            .expect_err("a non-object Chat message must be rejected at its item path"),
            "$.messages[0]",
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [{"role": false, "content": "hello"}]
            }))
            .expect_err("a non-text Chat role must be rejected at its field path"),
            "$.messages[0].role",
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [{"role": "user", "content": false}]
            }))
            .expect_err(
                "a non-text-or-array Chat content value must be rejected at its field path",
            ),
            "$.messages[0].content",
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": false
            }))
            .expect_err("a non-text-or-array Responses input must be rejected at its path"),
            "$.input",
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": [false]
            }))
            .expect_err("a non-object Responses input item must be rejected at its item path"),
            "$.input[0]",
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": [{"type": false, "content": "hello"}]
            }))
            .expect_err("a non-text Responses item type must not silently default"),
            "$.input[0].type",
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": [{"role": "assistant", "content": "hello"}]
            }))
            .expect_err("Responses input without a user item must be rejected before mapping"),
            "$.input",
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
fn d2_ac_001_chat_no_user_rejects_after_each_valid_field_receipt() {
    let sentinel = "D2-OPENAI-CHAT-NO-USER-MUST-NOT-REACH-RECEIPT";
    let error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [
            {
                "role": "assistant",
                "content": [{"type": "text", "text": sentinel}]
            },
            {"role": "system", "content": "Use the support playbook."}
        ]
    }))
    .expect_err("a valid Chat transcript still requires a user turn");

    for source_path in [
        "$.messages[0].role",
        "$.messages[0].content",
        "$.messages[0].content[0].type",
        "$.messages[0].content[0].text",
        "$.messages[1].role",
        "$.messages[1].content",
    ] {
        assert_eq!(
            error
                .report
                .decisions
                .iter()
                .filter(|decision| decision.source_path == source_path)
                .count(),
            1,
            "{source_path} must be decided before the no-user rejection"
        );
    }
    assert!(error
        .report
        .has_decision("$.messages", TranslationDecisionKind::Rejected));
    assert!(
        !serde_json::to_string(&error.report)
            .expect("receipt should serialize")
            .contains(sentinel),
        "the receipt must not retain valid message text"
    );
}

#[test]
fn d2_ac_001_chat_nested_rejection_keeps_parent_field_receipts() {
    let error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{
            "role": "user",
            "content": [{"type": "text"}]
        }]
    }))
    .expect_err("a malformed text block must be rejected by the Chat adapter");

    for source_path in [
        "$.messages[0].role",
        "$.messages[0].content",
        "$.messages[0].content[0].type",
        "$.messages[0].content[0].text",
    ] {
        assert_eq!(
            error
                .report
                .decisions
                .iter()
                .filter(|decision| decision.source_path == source_path)
                .count(),
            1,
            "{source_path} must be decided before nested validation returns"
        );
    }
    assert!(error.report.has_decision(
        "$.messages[0].content[0].text",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn d2_ac_001_responses_default_role_reaches_the_same_canonical_user_mapping() {
    let translated = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [
            {"role": "assistant", "content": "earlier answer"},
            {"content": "hello"}
        ]
    }))
    .expect("a default Responses user role must not fail after validation");

    assert_eq!(translated.request.query, "hello");
    assert_eq!(
        translated.request.history,
        vec![json!({"role": "assistant", "content": "earlier answer"})]
    );
    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.input[1].role")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "default role needs one final receipt");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Defaulted);
}

#[test]
fn d2_ac_001_chat_system_media_is_unsupported_with_its_media_receipt() {
    let sentinel = "D2-OPENAI-SYSTEM-MEDIA-MUST-NOT-REACH-RECEIPT";
    for role in ["system", "developer"] {
        let error = translate_chat_completion_request(json!({
            "model": "gpt-compatible",
            "messages": [
                {
                    "role": role,
                    "content": [
                        {"type": "image_url", "image_url": sentinel}
                    ]
                },
                {"role": "user", "content": "hello"}
            ]
        }))
        .expect_err("system/developer media has no current canonical owner");

        let decisions = error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == "$.messages[0].content[0].type")
            .collect::<Vec<_>>();
        assert_eq!(
            decisions.len(),
            1,
            "{role} media type needs one final receipt"
        );
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Unsupported);
        assert!(
            !serde_json::to_string(&error.report)
                .expect("receipt should serialize")
                .contains(sentinel),
            "{role} media receipt must not retain the raw URL"
        );
    }
}

#[test]
fn d2_ac_001_responses_system_and_developer_media_stop_at_the_protocol_adapter() {
    let sentinel = "D2-OPENAI-RESPONSES-SYSTEM-MEDIA-MUST-NOT-REACH-HISTORY";
    for (role, media_type) in [
        ("system", "image_url"),
        ("system", "input_image"),
        ("developer", "image_url"),
        ("developer", "input_image"),
    ] {
        let error = translate_response_request(json!({
            "model": "gpt-compatible",
            "input": [
                {
                    "type": "message",
                    "role": role,
                    "content": [{"type": media_type, "image_url": sentinel}]
                },
                {"type": "message", "role": "user", "content": "hello"}
            ]
        }))
        .expect_err("Responses system/developer media has no Native canonical owner");

        for source_path in ["$.input[0].role", "$.input[0].content[0].type"] {
            assert_eq!(
                error
                    .report
                    .decisions
                    .iter()
                    .filter(|decision| decision.source_path == source_path)
                    .count(),
                1,
                "{role}/{media_type} {source_path} needs one final receipt"
            );
        }
        assert!(error.report.has_decision(
            "$.input[0].content[0].type",
            TranslationDecisionKind::Unsupported
        ));
        assert!(
            !serde_json::to_string(&error.report)
                .expect("receipt should serialize")
                .contains(sentinel),
            "{role}/{media_type} receipt must not retain media input"
        );
    }
}

#[test]
fn d2_ac_001_responses_system_and_developer_text_share_the_system_mapping() {
    let translated = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [
            {"type": "message", "role": "system", "content": "Use the support playbook."},
            {"type": "message", "role": "developer", "content": "Prefer concise answers."},
            {"type": "message", "role": "user", "content": "hello"}
        ]
    }))
    .expect("Responses system/developer text should translate without a mapper-only role");

    assert_eq!(
        translated.request.system_text().as_deref(),
        Some("Use the support playbook.\n\nPrefer concise answers.")
    );
    assert!(translated.request.history.is_empty());
}

#[test]
fn d4_ac_016_responses_store_is_exact_in_native_transport() {
    let mut translated = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "hello",
        "store": true
    }))
    .expect("server-side response storage should remain in native Responses transport");

    let decisions = translated
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.store")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "store needs one final receipt");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Exact);
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("store should remain in ephemeral provider transport");
    assert_eq!(payload.wire_body()["store"], true);
}

#[test]
fn d2_ac_001_openai_nested_receipts_preserve_missing_and_malformed_presence() {
    let cases = [
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{"text": "hello"}]
                }]
            })),
            "$.messages[0].content[0].type",
            TranslationSafeRepresentation::Absent,
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{"text": "hello"}]
                }]
            })),
            "$.input[0].content[0].type",
            TranslationSafeRepresentation::Absent,
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{"type": "text", "text": false}]
                }]
            })),
            "$.messages[0].content[0].text",
            TranslationSafeRepresentation::Present,
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": false}]
                }]
            })),
            "$.input[0].content[0].text",
            TranslationSafeRepresentation::Present,
        ),
        (
            translate_chat_completion_request(json!({
                "model": "gpt-compatible",
                "messages": [{
                    "role": "user",
                    "content": [{
                        "type": "image_url",
                        "image_url": {"url": false}
                    }]
                }]
            })),
            "$.messages[0].content[0].image_url.url",
            TranslationSafeRepresentation::Present,
        ),
        (
            translate_response_request(json!({
                "model": "gpt-compatible",
                "input": [{
                    "type": "message",
                    "role": "user",
                    "content": [{
                        "type": "input_image",
                        "image_url": {"url": false}
                    }]
                }]
            })),
            "$.input[0].content[0].image_url.url",
            TranslationSafeRepresentation::Present,
        ),
    ];

    for (result, source_path, effective_value) in cases {
        let error = result.expect_err("invalid OpenAI nested fields must be rejected");
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
fn d2_ac_001_openai_trimmed_user_is_recorded_as_normalized() {
    let cases = [
        translate_chat_completion_request(json!({
            "model": "gpt-compatible",
            "messages": [{"role": "user", "content": "hello"}],
            "user": "  external-user  "
        })),
        translate_response_request(json!({
            "model": "gpt-compatible",
            "input": "hello",
            "user": "  external-user  "
        })),
    ];

    for result in cases {
        let translated = result.expect("a trimmed OpenAI user should translate");
        assert_eq!(
            translated.request.conversation["user"],
            json!("external-user")
        );
        let decisions = translated
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == "$.user")
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "user needs one final receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Normalized);
    }
}

#[test]
fn d2_ac_001_chat_unknown_field_is_preserved_in_one_safe_protocol_residual() {
    let mut request = base_request();
    request["unmapped_top_level_option"] = json!(true);

    let translated = translate_chat_completion_request(request)
        .expect("safe unknown Chat roots should stay protocol-authentic");

    assert!(translated
        .report
        .has_decision("$.<unknown>[0]", TranslationDecisionKind::Exact));
    let envelope = translated
        .request
        .client_protocol_envelope
        .expect("unknown Chat root should create a protocol residual");
    assert_eq!(envelope.source_protocol, "openai_chat");
    assert_eq!(envelope.body["unmapped_top_level_option"], true);
    for typed in ["model", "messages", "max_tokens"] {
        assert!(!envelope.body.contains_key(typed), "{typed}");
    }
}

#[test]
fn openai_responses_unknown_root_is_residual_without_duplicating_typed_roots() {
    let translated = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "hello",
        "stream": true,
        "future_responses_extension": {"shape": "opaque"}
    }))
    .expect("safe unknown Responses root should stay protocol-authentic");

    let envelope = translated
        .request
        .client_protocol_envelope
        .expect("unknown Responses root should create a protocol residual");
    assert_eq!(envelope.source_protocol, "openai_responses");
    assert_eq!(
        envelope.body["future_responses_extension"]["shape"],
        "opaque"
    );
    for typed in ["model", "input", "stream"] {
        assert!(!envelope.body.contains_key(typed), "{typed}");
    }
}
#[path = "openai_compat/field_receipts.rs"]
mod field_receipts;
