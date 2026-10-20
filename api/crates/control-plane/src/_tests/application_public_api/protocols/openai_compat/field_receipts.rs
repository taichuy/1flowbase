use super::*;

#[test]
fn wp_d2a_chat_and_responses_reasoning_have_one_typed_owner() {
    let chat = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        "reasoning_effort": "high"
    }))
    .expect("Chat reasoning effort should map to Native");
    let responses = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "hello",
        "reasoning": {"effort": "xhigh"}
    }))
    .expect("Responses reasoning effort should map to Native");

    for (translated, effort) in [(chat, "high"), (responses, "xhigh")] {
        let execution = serde_json::to_value(&translated.request.execution)
            .expect("typed execution should serialize");
        assert_eq!(
            execution["model_parameters"]["reasoning"]["mode"],
            json!("enabled")
        );
        assert_eq!(
            execution["model_parameters"]["reasoning"]["effort"],
            json!(effort)
        );
        assert!(
            translated.request.client_protocol_envelope.is_none(),
            "typed reasoning must not remain in an opaque protocol residual"
        );
    }
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
fn d4_ac_016_responses_input_file_remains_exact_in_native_transport() {
    let mut translated = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_file", "file_id": "file_123"}]
        }]
    }))
    .expect("input_file should remain in native Responses transport");

    assert!(translated
        .report
        .has_decision("$.input[0]", TranslationDecisionKind::Exact));
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("input_file should remain in ephemeral provider transport");
    assert_eq!(
        payload.wire_body()["input"][0]["content"][0]["type"],
        "input_file"
    );
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
fn d4_ac_016_responses_unknown_content_field_remains_exact_in_native_transport() {
    let mut translated = translate_response_request(json!({
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
    .expect("unknown Responses content fields should remain in native transport");

    assert!(translated
        .report
        .has_decision("$.input[0]", TranslationDecisionKind::Exact));
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("unknown content field should remain ephemeral");
    assert_eq!(
        payload.wire_body()["input"][0]["content"][0]["unexpected"],
        "must not reach canonical history"
    );
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
fn d2_f1_openai_unknown_defined_keys_are_anonymous_and_preserved_as_residuals() {
    let alpha = "D2-F1-OPENAI-UNKNOWN-KEY-ALPHA";
    let beta = "D2-F1-OPENAI-UNKNOWN-KEY-BETA";
    let translated = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        alpha: true,
        beta: false
    }))
    .expect("safe unknown Chat roots should remain protocol-authentic residuals");
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
        .expect("unknown Chat roots should create one residual envelope");
    assert_eq!(envelope.body[alpha], true);
    assert_eq!(envelope.body[beta], false);
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
        &AiNativeOperation::Generate(AiNativeGenerateProfile::LocalSummary)
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
        &AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompact)
    );
    assert_compaction_intent(
        &legacy.request,
        CompactionProfile::ResponsesCompact,
        CompactionResultRequirement::ResponseItems,
    );

    let v2 = translate_response_request_with_context(
        responses_request(json!([
            {"type": "message", "role": "user", "content": "retained user turn"},
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
        &AiNativeOperation::Compact(AiNativeCompactProfile::ResponsesCompactionV2)
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
        &AiNativeOperation::Generate(AiNativeGenerateProfile::Standard)
    );

    let regular_metadata = translate_response_request(json!({
        "model": "gpt-compatible",
        "input": "Summarize the previous discussion for a teammate.",
        "metadata": {"trace_id": "client-local-summary"}
    }))
    .expect("regular OpenAI metadata is not captured Codex compaction evidence");
    assert_eq!(
        regular_metadata.request.execution.execution_operation(),
        &AiNativeOperation::Generate(AiNativeGenerateProfile::Standard)
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

#[test]
fn increment_effort_openai_shared_mapping_preserves_extensions_and_rejects_format() {
    for effort in [json!("max"), json!("ultra"), json!("custom-v2")] {
        let responses = translate_response_request(
            json!({"model":"public-model","input":"hello","reasoning":{"effort":effort}}),
        )
        .unwrap();
        let chat = translate_chat_completion_request(json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning_effort":effort})).unwrap();
        for translated in [responses, chat] {
            let execution = serde_json::to_value(translated.request.execution).unwrap();
            assert_eq!(execution["model_parameters"]["reasoning"]["effort"], effort);
        }
    }
    for effort in [
        json!(""),
        json!(" max"),
        json!("max "),
        json!(7),
        json!(null),
        json!({}),
        json!("x".repeat(129)),
        json!("a\nb"),
    ] {
        assert!(translate_response_request(
            json!({"model":"public-model","input":"hello","reasoning":{"effort":effort}})
        )
        .is_err());
        assert!(translate_chat_completion_request(json!({"model":"public-model","messages":[{"role":"user","content":"hello"}],"reasoning_effort":effort})).is_err());
    }
}
