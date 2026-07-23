use control_plane::application_public_api::compat::openai::{
    map_chat_completion_request, map_response_request, translate_chat_completion_request,
    translate_response_request, translate_response_request_with_context, OpenAiCompatError,
    OpenAiResponsesEndpoint, OpenAiResponsesRequestContext,
};
use control_plane::application_public_api::native::{
    CompactionProfile, CompactionResultRequirement, NativeExecutionOperation,
    RemoteCompactionProfile,
};
use control_plane::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationSafeRepresentation,
};
use control_plane::application_public_api::run_service::GenerateExecutionProfile;
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
    let intent = request
        .execution
        .execution_operation()
        .compaction_intent()
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
fn d2_ac_001_responses_store_is_unsupported_not_dropped() {
    let error = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "hello",
        "store": true
    }))
    .expect_err("server-side response storage has no current Native owner");

    assert_openai_unsupported_feature(error.clone(), "store");
    let decisions = error
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path == "$.store")
        .collect::<Vec<_>>();
    assert_eq!(decisions.len(), 1, "store needs one final receipt");
    assert_eq!(decisions[0].kind, TranslationDecisionKind::Unsupported);
    assert!(
        !error
            .report
            .decisions
            .iter()
            .any(|decision| decision.kind == TranslationDecisionKind::Dropped),
        "a capability-dependent ingress field must not be silently dropped"
    );
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
fn d2_ac_001_chat_unknown_field_is_rejected_with_a_safe_receipt() {
    let mut request = base_request();
    request["unmapped_top_level_option"] = json!(true);

    let error = translate_chat_completion_request(request)
        .expect_err("unknown Chat field must not be silently dropped");

    assert!(error
        .report
        .has_decision("$.<unknown>[0]", TranslationDecisionKind::Rejected));
}

#[test]
fn last_user_text_maps_to_native_query() {
    let native = map_chat_completion_request(base_request()).unwrap();

    assert_eq!(native.query, "Final question");
}

#[test]
fn last_user_image_url_maps_to_native_content_blocks() {
    let native = map_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Describe image"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://example.com/cat.png"}
                    }
                ]
            }
        ]
    }))
    .unwrap();

    assert_eq!(native.query, "Describe image");
    assert_eq!(native.history.len(), 1);
    assert_eq!(native.history[0]["role"], json!("user"));
    assert_eq!(native.history[0]["content"], json!("Describe image"));
    assert_eq!(
        native.history[0]["content_blocks"],
        json!([
            {"type": "text", "text": "Describe image"},
            {
                "type": "image_url",
                "image_url": {"url": "https://example.com/cat.png"}
            }
        ])
    );
}

#[test]
fn prior_system_message_maps_to_native_system_context() {
    let native = map_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [
            {"role": "system", "content": "Use the support playbook."},
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ]
    }))
    .unwrap();

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
fn stream_true_maps_to_native_streaming_response_mode() {
    let mut request = base_request();
    request["stream"] = json!(true);

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(native.response_mode.as_deref(), Some("streaming"));
}

#[test]
fn user_maps_to_native_conversation_user() {
    let mut request = base_request();
    request["user"] = json!("external-user-123");

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.conversation.get("user"),
        Some(&json!("external-user-123"))
    );
}

#[test]
fn trace_id_metadata_maps_to_native_metadata() {
    let mut request = base_request();
    request["metadata"] = json!({
        "trace_id": "trace-123"
    });

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.metadata.as_value(),
        json!({ "trace_id": "trace-123" })
    );
}

#[test]
fn responses_instructions_map_to_native_system_context() {
    let native = map_response_request(
        json!({
            "model": "gpt-compatible",
            "instructions": "Use the support playbook.",
            "input": "Final question"
        }),
        None,
    )
    .unwrap();

    assert_eq!(native.query, "Final question");
    assert_eq!(
        native.system_text().as_deref(),
        Some("Use the support playbook.")
    );
    assert!(native.history.is_empty());
}

#[test]
fn responses_input_image_maps_to_native_content_blocks() {
    let native = map_response_request(
        json!({
            "model": "gpt-compatible",
            "input": [
                {
                    "role": "user",
                    "content": [
                        {"type": "input_text", "text": "Describe image"},
                        {
                            "type": "input_image",
                            "image_url": "data:image/png;base64,aW1hZ2U="
                        }
                    ]
                }
            ]
        }),
        None,
    )
    .unwrap();

    assert_eq!(native.query, "Describe image");
    assert_eq!(native.history.len(), 1);
    assert_eq!(
        native.history[0]["content_blocks"][0]["type"],
        json!("text")
    );
    assert_eq!(
        native.history[0]["content_blocks"][1],
        json!({
            "type": "image_url",
            "image_url": {"url": "data:image/png;base64,aW1hZ2U="}
        })
    );
}

#[test]
fn model_maps_exactly_without_validation() {
    let mut request = base_request();
    request["model"] = json!("unregistered/provider:model.with/slashes");

    let native = map_chat_completion_request(request).unwrap();

    assert_eq!(
        native.model.as_deref(),
        Some("unregistered/provider:model.with/slashes")
    );
}

#[test]
fn d2_ac_007_chat_legacy_function_call_is_unsupported_with_a_translation_receipt() {
    let mut request = base_request();
    request["function_call"] = json!({"name": "lookup_order"});

    assert_unsupported_feature(request, "function_call");
}

#[test]
fn d2_ac_007_responses_nested_unsupported_content_has_a_field_receipt() {
    let error = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_file", "file_id": "file_123"}]
        }]
    }))
    .expect_err("input_file has no D2 canonical owner");

    assert_openai_unsupported_feature(error.clone(), "input");
    assert!(error.report.has_decision(
        "$.input[0].content[0].type",
        TranslationDecisionKind::Unsupported
    ));
}

#[test]
fn d2_ac_001_chat_content_part_unknown_field_is_rejected_with_its_own_receipt() {
    let error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{
            "role": "user",
            "content": [{
                "type": "text",
                "text": "hello",
                "unexpected": "must not reach canonical history"
            }]
        }]
    }))
    .expect_err("an unknown content-part field has no canonical owner");

    assert!(error.report.has_decision(
        "$.messages[0].content[0].<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn d2_ac_001_responses_content_part_unknown_field_is_rejected_with_its_own_receipt() {
    let error = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "hello",
                "unexpected": "must not reach canonical history"
            }]
        }]
    }))
    .expect_err("an unknown Responses content-part field has no canonical owner");

    assert!(error.report.has_decision(
        "$.input[0].content[0].<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn d2_ac_001_openai_image_source_unknown_field_is_rejected_before_canonical_history() {
    let error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{
            "role": "user",
            "content": [{
                "type": "image_url",
                "image_url": {
                    "url": "https://example.com/cat.png",
                    "unexpected": "raw compat payload"
                }
            }]
        }]
    }))
    .expect_err("untyped image source fields must not reach canonical history");

    assert!(error.report.has_decision(
        "$.messages[0].content[0].image_url.<unknown>[0]",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn audio_output_returns_unsupported_feature() {
    let mut request = base_request();
    request["audio"] = json!({
        "voice": "alloy",
        "format": "mp3"
    });

    assert_unsupported_feature(request, "audio");
}

#[test]
fn multimodal_generation_returns_unsupported_feature() {
    let mut request = base_request();
    request["modalities"] = json!(["text", "audio"]);

    assert_unsupported_feature(request, "modalities");
}

#[test]
fn d2_f1_openai_nested_failures_reject_the_messages_and_input_containers() {
    let chat_error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{"role": "user", "content": false}]
    }))
    .expect_err("malformed Chat content must reject its parent messages container");
    for source_path in ["$.messages", "$.messages[0].content"] {
        let decisions = chat_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one receipt");
        assert_eq!(decisions[0].kind, TranslationDecisionKind::Rejected);
    }

    let responses_error = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [{"type": "message", "role": "user", "content": false}]
    }))
    .expect_err("malformed Responses content must reject its input container");
    for source_path in ["$.input", "$.input[0].content"] {
        let decisions = responses_error
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
fn d2_f1_openai_unknown_defined_keys_are_anonymous_and_complete() {
    let alpha = "D2-F1-OPENAI-UNKNOWN-KEY-ALPHA";
    let beta = "D2-F1-OPENAI-UNKNOWN-KEY-BETA";
    let error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        alpha: true,
        beta: false
    }))
    .expect_err("unknown Chat keys must not become receipt content");
    let unknown_paths = error
        .report
        .decisions
        .iter()
        .filter(|decision| decision.source_path.starts_with("$.<unknown>"))
        .map(|decision| decision.source_path.as_str())
        .collect::<Vec<_>>();
    assert_eq!(unknown_paths, ["$.<unknown>[0]", "$.<unknown>[1]"]);
    let serialized = serde_json::to_string(&error.report).expect("receipt serializes");
    assert!(!serialized.contains(alpha));
    assert!(!serialized.contains(beta));
}

#[test]
fn d2_f1_openai_defined_container_receipts_remain_unique_on_nested_rejection() {
    let chat_error = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{
            "role": "user",
            "content": [{"type": "image_url", "image_url": {"url": false}}]
        }]
    }))
    .expect_err("a malformed Chat image URL rejects its leaf without losing container receipts");
    for (source_path, kind) in [
        ("$.messages", TranslationDecisionKind::Rejected),
        ("$.messages[0]", TranslationDecisionKind::Normalized),
        ("$.messages[0].content", TranslationDecisionKind::Normalized),
        (
            "$.messages[0].content[0]",
            TranslationDecisionKind::Normalized,
        ),
        (
            "$.messages[0].content[0].image_url",
            TranslationDecisionKind::Normalized,
        ),
        (
            "$.messages[0].content[0].image_url.url",
            TranslationDecisionKind::Rejected,
        ),
    ] {
        let decisions = chat_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, kind);
    }

    let responses_error = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_image", "image_url": {"url": false}}]
        }]
    }))
    .expect_err(
        "a malformed Responses image URL rejects its leaf without losing container receipts",
    );
    for (source_path, kind) in [
        ("$.input", TranslationDecisionKind::Rejected),
        ("$.input[0]", TranslationDecisionKind::Normalized),
        ("$.input[0].content", TranslationDecisionKind::Normalized),
        ("$.input[0].content[0]", TranslationDecisionKind::Normalized),
        (
            "$.input[0].content[0].image_url",
            TranslationDecisionKind::Normalized,
        ),
        (
            "$.input[0].content[0].image_url.url",
            TranslationDecisionKind::Rejected,
        ),
    ] {
        let decisions = responses_error
            .report
            .decisions
            .iter()
            .filter(|decision| decision.source_path == source_path)
            .collect::<Vec<_>>();
        assert_eq!(decisions.len(), 1, "{source_path} needs one final receipt");
        assert_eq!(decisions[0].kind, kind);
    }
}

#[test]
fn k1_codex_compaction_profiles_select_closed_operations_and_result_requirements() {
    let local = translate_response_request_with_context(
        responses_request(json!([{"role": "user", "content": "earlier turn"}])),
        OpenAiResponsesRequestContext::new(OpenAiResponsesEndpoint::Responses)
            .with_captured_codex_turn_metadata(codex_compaction_metadata("responses")),
    )
    .expect("Codex local compaction metadata should be explicit enough to classify");
    assert_eq!(
        local.request.execution.execution_operation(),
        &NativeExecutionOperation::Generate(GenerateExecutionProfile::LocalSummary)
    );
    assert_compaction_intent(
        &local.request,
        CompactionProfile::LocalSummary,
        CompactionResultRequirement::Generate,
    );

    let legacy = translate_response_request_with_context(
        responses_request(json!([{"role": "user", "content": "retained user turn"}])),
        OpenAiResponsesRequestContext::new(OpenAiResponsesEndpoint::ResponsesCompact)
            .with_captured_codex_turn_metadata(codex_compaction_metadata("responses_compact")),
    )
    .expect("Codex legacy compact endpoint should select its dedicated profile");
    assert_eq!(
        legacy.request.execution.execution_operation(),
        &NativeExecutionOperation::Compact(RemoteCompactionProfile::ResponsesCompact)
    );
    assert_compaction_intent(
        &legacy.request,
        CompactionProfile::ResponsesCompact,
        CompactionResultRequirement::ResponseItems,
    );

    let v2 = translate_response_request_with_context(
        responses_request(json!([
            {"role": "user", "content": "retained user turn"},
            {"type": "compaction_trigger"}
        ])),
        OpenAiResponsesRequestContext::new(OpenAiResponsesEndpoint::Responses)
            .with_captured_codex_turn_metadata(codex_compaction_metadata(
                "responses_compaction_v2",
            )),
    )
    .expect("Codex V2 trigger and metadata should select the opaque V2 profile");
    assert_eq!(
        v2.request.execution.execution_operation(),
        &NativeExecutionOperation::Compact(RemoteCompactionProfile::ResponsesCompactionV2)
    );
    assert_compaction_intent(
        &v2.request,
        CompactionProfile::ResponsesCompactionV2,
        CompactionResultRequirement::CompletedOpaqueCompactionItem,
    );
}

#[test]
fn k1_ordinary_summary_and_uncaptured_client_compaction_stay_generate() {
    let ordinary_summary = translate_response_request(responses_request(json!(
        "Summarize the previous discussion for a teammate."
    )))
    .expect("ordinary summarization text remains a Generate request");
    assert_eq!(
        ordinary_summary.request.execution.execution_operation(),
        &NativeExecutionOperation::Generate(GenerateExecutionProfile::Standard)
    );

    let regular_metadata = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "Summarize the previous discussion for a teammate.",
        "metadata": {"trace_id": "client-local-summary"}
    }))
    .expect("regular OpenAI metadata is not captured Codex compaction evidence");
    assert_eq!(
        regular_metadata.request.execution.execution_operation(),
        &NativeExecutionOperation::Generate(GenerateExecutionProfile::Standard)
    );
}

#[test]
fn k1_unknown_or_malformed_captured_codex_profile_is_never_guessed() {
    let unknown = translate_response_request_with_context(
        responses_request(json!([{"role": "user", "content": "retained user turn"}])),
        OpenAiResponsesRequestContext::responses()
            .with_captured_codex_turn_metadata(codex_compaction_metadata("future_compaction")),
    )
    .expect_err("an unknown Codex compaction profile must be typed unsupported");
    assert_eq!(unknown.code, "unsupported_compaction_profile");
    assert!(unknown.report.has_decision(
        "$.ingress.x-codex-turn-metadata.compaction.implementation",
        TranslationDecisionKind::Unsupported
    ));

    let malformed = translate_response_request_with_context(
        responses_request(json!([{"role": "user", "content": "retained user turn"}])),
        OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(json!({
            "request_kind": "compaction",
            "compaction": {"implementation": false}
        })),
    )
    .expect_err("malformed captured Codex metadata must not become a compaction fallback");
    assert_eq!(malformed.code, "invalid_request");
    assert!(malformed.report.has_decision(
        "$.ingress.x-codex-turn-metadata",
        TranslationDecisionKind::Rejected
    ));
}

#[test]
fn k1_compaction_metadata_stays_behind_the_authenticated_ingress_boundary() {
    let error = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "Summarize the previous discussion for a teammate.",
        "metadata": {
            "trace_id": "client-supplied",
            "compaction": codex_compaction_metadata("responses")
        }
    }))
    .expect_err("ordinary body metadata must never become captured Codex evidence");
    assert_eq!(error.code, "invalid_request");
    assert!(error
        .report
        .has_decision("$.metadata.<unknown>[0]", TranslationDecisionKind::Rejected));
    assert!(!error
        .report
        .has_decision("$.ingress.endpoint", TranslationDecisionKind::Exact));
}

#[test]
fn k1_remote_profiles_do_not_fallback_to_local_summary_on_mismatched_evidence() {
    let error = translate_response_request_with_context(
        responses_request(json!([{"role": "user", "content": "retained user turn"}])),
        OpenAiResponsesRequestContext::responses()
            .with_captured_codex_turn_metadata(codex_compaction_metadata("responses_compact")),
    )
    .expect_err("legacy remote metadata on the normal endpoint must not downgrade to local");
    assert_eq!(error.code, "invalid_request");
    assert!(error.report.has_decision(
        "$.ingress.compaction_evidence",
        TranslationDecisionKind::Rejected
    ));
}
