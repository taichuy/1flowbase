use super::*;

// #2028 AC-007/008: a native function result remains a sealed delta even
// when this turn has no custom tool declaration to trigger native admission.
#[test]
fn issue_2028_native_function_result_inherits_provider_continuation() {
    let body = json!({"model":"fixture","previous_response_id":"resp_public","input":[{"type":"function_call_output","call_id":"call_original","output":"random-result"}]});
    let previous = OpenAiPreviousResponseContext {
        history: None,
        provider_continuation: Some(
            crate::ports::ProviderContinuation::new(
                "resp_upstream",
                crate::ports::ProviderTransportAffinity::new(
                    "provider",
                    "openai",
                    "openai_responses",
                    "fixture",
                ),
            )
            .unwrap(),
        ),
        response_id: "resp_public".into(),
        external_user: None,
        external_conversation_id: None,
        answer: None,
    };
    let mut translated = translate_response_request_with_context_and_previous(
        body.clone(),
        OpenAiResponsesRequestContext::responses(),
        Some(previous),
    )
    .expect("native function result is valid without replayed call");
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("continuation delta must stay native");
    assert_eq!(payload.wire_body(), &body);
}

// #2032 AC-001/002/008/010: real Responses translation must retain only a
// separate log identity; input history and native transport remain unchanged.
#[test]
fn issue_2032_responses_identity_is_separate_from_model_history() {
    let body = json!({"model":"1flowbase","client_metadata":{"thread_id":"thread-A","turn_id":"turn-B","x-codex-turn-metadata":"{\"thread_id\":\"thread-A\",\"turn_id\":\"turn-B\",\"request_kind\":\"turn\"}"},"input":[{"role":"user","content":"same question"}]});
    let translated = translate_response_request(body.clone()).unwrap();
    let context = translated
        .request
        .metadata
        .application_run_log_context()
        .expect("durable log identity must survive translation");
    assert_eq!(context.thread_id.as_deref(), Some("thread-A"));
    assert_eq!(context.turn_id.as_deref(), Some("turn-B"));
    let mut without_identity = body;
    without_identity
        .as_object_mut()
        .unwrap()
        .remove("client_metadata");
    let baseline = translate_response_request(without_identity).unwrap();
    assert_eq!(translated.request.history, baseline.request.history);
    assert_eq!(
        translated.request.conversation,
        baseline.request.conversation
    );
    assert_eq!(translated.request.query, baseline.request.query);
    assert_eq!(
        translated.request.metadata.as_value(),
        baseline.request.metadata.as_value()
    );
}

#[test]
fn issue_2032_log_identity_rejects_conflicts_and_preserves_fork_provenance() {
    use super::super::log_context::{
        capture_application_run_log_context, reconcile_client_log_headers,
    };
    let body = serde_json::json!({"client_metadata":{
        "thread_id":"child", "turn_id":"turn-child",
        "x-codex-turn-metadata":serde_json::json!({"thread_id":"child","turn_id":"turn-child","forked_from_thread_id":"parent","request_kind":"turn"}).to_string()
    },"input":[{"role":"user","content":"same question"}]});
    let mut context = capture_application_run_log_context(&body, None);
    let snapshot = serde_json::to_value(&context).unwrap();
    assert_eq!(snapshot["forked_from_thread_id"], "parent");
    assert!(snapshot["identity_sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "client_metadata"));
    let headers =
        std::collections::BTreeMap::from([("thread-id".into(), vec!["different".into()])]);
    reconcile_client_log_headers(&mut context, &headers);
    assert_eq!(context.identity_status, "conflicting_identity");
    assert!(context.thread_id.is_none());
    let invalid = capture_application_run_log_context(
        &serde_json::json!({"client_metadata":{"x-codex-turn-metadata":"[]"}}),
        None,
    );
    assert_eq!(invalid.identity_status, "invalid_identity");
}

// #2034 AC-002/003/004/005: the mapping layer reconciles every Codex projection
// (body flat keys, x-codex-turn-metadata, ingress headers) into one protocol
// neutral declaration and writes the protocol name itself.
#[test]
fn issue_2034_mapping_layer_reconciles_header_only_identity_and_subagent() {
    use super::super::log_context::reconcile_client_log_headers;
    let turn_metadata = serde_json::json!({"thread_id":"thread-H","turn_id":"turn-H","request_kind":"turn","subagent_kind":"review"});
    let context = OpenAiResponsesRequestContext::responses()
        .with_captured_codex_turn_metadata(turn_metadata.clone());
    let translated = translate_response_request_with_context(
        json!({"model":"1flowbase","input":[{"role":"user","content":"header only"}]}),
        context,
    )
    .unwrap();
    let mut log_context = translated
        .request
        .metadata
        .application_run_log_context()
        .cloned()
        .expect("header declared identity is captured without body client_metadata");
    assert_eq!(log_context.protocol.as_deref(), Some("openai_responses"));
    assert_eq!(log_context.thread_id.as_deref(), Some("thread-H"));
    assert_eq!(log_context.turn_id.as_deref(), Some("turn-H"));
    assert_eq!(log_context.identity_status, "identified");
    assert_eq!(log_context.subagent_kind.as_deref(), Some("review"));
    assert_eq!(log_context.call_kind.as_deref(), Some("generate"));
    assert!(log_context
        .identity_sources
        .iter()
        .any(|source| source == "header.x-codex-turn-metadata"));
    // The envelope session-id is the route's key-scoped reconnect identity, not
    // the client's session; it must never be treated as a conflicting declaration.
    let headers = std::collections::BTreeMap::from([
        ("thread-id".to_string(), vec!["thread-H".to_string()]),
        ("session-id".to_string(), vec!["9f2c3a".repeat(10)]),
        ("x-openai-subagent".to_string(), vec!["review".to_string()]),
    ]);
    reconcile_client_log_headers(&mut log_context, &headers);
    assert_eq!(log_context.identity_status, "identified");
    assert_eq!(log_context.session_id, None);
    assert!(!log_context
        .identity_sources
        .iter()
        .any(|source| source == "header.session-id"));
    let conflicting = std::collections::BTreeMap::from([(
        "x-openai-subagent".to_string(),
        vec!["compact".to_string()],
    )]);
    reconcile_client_log_headers(&mut log_context, &conflicting);
    assert_eq!(log_context.identity_status, "conflicting_identity");
    assert!(log_context.thread_id.is_none());

    // A WebSocket handshake replays the prewarm header for every turn: the
    // thread must still agree, but the body owns the turn-scoped declarations.
    let handshake = OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(
        json!({"thread_id":"thread-H","session_id":"session-H","request_kind":"prewarm"}),
    );
    let turn = translate_response_request_with_context(
        json!({"model":"1flowbase","client_metadata":{"thread_id":"thread-H","turn_id":"turn-1",
            "x-codex-turn-metadata":json!({"thread_id":"thread-H","turn_id":"turn-1","request_kind":"turn","root_turn_id":"turn-1"}).to_string()},
            "input":[{"role":"user","content":"real turn"}]}),
        handshake.clone(),
    )
    .unwrap();
    let context = turn.request.metadata.application_run_log_context().unwrap();
    assert_eq!(context.identity_status, "identified");
    assert_eq!(context.request_kind.as_deref(), Some("turn"));
    assert_eq!(context.turn_id.as_deref(), Some("turn-1"));
    let other_thread = translate_response_request_with_context(
        json!({"model":"1flowbase","client_metadata":{"thread_id":"thread-X","turn_id":"turn-1"},
            "input":[{"role":"user","content":"real turn"}]}),
        handshake,
    )
    .unwrap();
    assert_eq!(
        other_thread
            .request
            .metadata
            .application_run_log_context()
            .unwrap()
            .identity_status,
        "conflicting_identity"
    );
}

// #2034 AC-001/004: call_kind is owned by the AI Native operation; request_kind
// is retained as the client's own value and never decides grouping by itself.
#[test]
fn issue_2034_call_kind_derives_from_native_operation_not_request_kind() {
    let compaction = translate_response_request_with_context(
        json!({"model":"1flowbase","input":[{"type":"message","role":"user","content":"compact"},{"type":"compaction_trigger"}]}),
        OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(json!({
            "thread_id":"thread-C","turn_id":"turn-C","request_kind":"compaction",
            "compaction":{"implementation":"responses_compaction_v2"}
        })),
    )
    .unwrap();
    let context = compaction
        .request
        .metadata
        .application_run_log_context()
        .unwrap();
    assert_eq!(context.call_kind.as_deref(), Some("compact"));
    assert_eq!(context.request_kind.as_deref(), Some("compaction"));
    assert_eq!(context.turn_id.as_deref(), Some("turn-C"));
    // Codex's default local-summary compaction executes as a generate profile;
    // the log still classifies it as a compaction of the same turn.
    let local_summary = translate_response_request_with_context(
        json!({"model":"1flowbase","input":[{"role":"user","content":"summarize history"}]}),
        OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(json!({
            "thread_id":"thread-C","turn_id":"turn-C","request_kind":"compaction",
            "compaction":{"implementation":"responses"}
        })),
    )
    .unwrap();
    let context = local_summary
        .request
        .metadata
        .application_run_log_context()
        .unwrap();
    assert_eq!(context.call_kind.as_deref(), Some("compact"));
    assert_eq!(context.turn_id.as_deref(), Some("turn-C"));
    let memory = translate_response_request_with_context(
        json!({"model":"1flowbase","input":[{"role":"user","content":"remember"}]}),
        OpenAiResponsesRequestContext::responses()
            .with_captured_codex_turn_metadata(json!({"request_kind":"memory","turn_id":"turn-M"})),
    )
    .unwrap();
    let context = memory
        .request
        .metadata
        .application_run_log_context()
        .unwrap();
    assert_eq!(context.identity_status, "missing_identity");
    assert_eq!(context.request_kind.as_deref(), Some("memory"));
    assert_eq!(context.call_kind.as_deref(), Some("generate"));
}

#[test]
fn issue_2039_v2_compaction_keeps_opaque_codex_history_out_of_native_state() {
    let request = json!({
        "model": "1flowbase",
        "input": [
            {"type":"message","role":"user","content":"inspect"},
            {"type":"reasoning","id":"rs_1","encrypted_content":"opaque"},
            {"type":"custom_tool_call","id":"ct_1","call_id":"call_1","name":"exec","input":"{}"},
            {"type":"custom_tool_call_output","call_id":"call_1","output":"done"},
            {"type":"compaction_trigger"}
        ]
    });
    let mut translated = translate_response_request_with_context(
        request.clone(),
        OpenAiResponsesRequestContext::responses(),
    )
    .expect("Codex V2 compaction history should be accepted as provider-opaque input");

    assert_eq!(
        *translated.request.execution.execution_operation(),
        domain::AiNativeOperation::Compact(domain::AiNativeCompactProfile::ResponsesCompactionV2)
    );
    assert!(translated.request.query.is_empty());
    assert!(translated.request.history.is_empty());
    assert!(translated.request.system.is_empty());
    assert_eq!(
        translated
            .request
            .metadata
            .take_provider_transport_payload()
            .unwrap()
            .wire_body(),
        &request
    );
}

#[test]
fn issue_2049_local_summary_keeps_opaque_codex_history_in_provider_transport() {
    let request = json!({
        "model": "1flowbase",
        "input": [
            {"type":"message","role":"user","content":"inspect"},
            {"type":"reasoning","id":"rs_1","encrypted_content":"opaque-reasoning"},
            {"type":"custom_tool_call","id":"ct_1","call_id":"call_1","name":"exec","input":"{}"},
            {"type":"custom_tool_call_output","call_id":"call_1","output":"private-output"},
            {"type":"message","role":"assistant","phase":"commentary","content":"work complete"},
            {"type":"message","role":"user","content":"summarize history"}
        ]
    });
    let mut translated = translate_response_request_with_context(
        request.clone(),
        OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(json!({
            "thread_id":"thread-local-summary",
            "turn_id":"turn-local-summary",
            "request_kind":"compaction",
            "compaction":{"implementation":"responses"}
        })),
    )
    .expect("Codex local-summary history should use provider-opaque transport");

    assert_eq!(
        *translated.request.execution.execution_operation(),
        domain::AiNativeOperation::Generate(domain::AiNativeGenerateProfile::LocalSummary)
    );
    assert!(translated.request.query.is_empty());
    assert!(translated.request.history.is_empty());
    assert!(translated.request.system.is_empty());
    assert_eq!(
        translated
            .request
            .metadata
            .application_run_log_context()
            .and_then(|context| context.call_kind.as_deref()),
        Some("compact")
    );
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("provider-opaque local summary must retain its exact wire body");
    assert_eq!(payload.wire_body(), &request);
    let debug = format!("{payload:?}");
    assert!(!debug.contains("opaque-reasoning"));
    assert!(!debug.contains("private-output"));
}

#[test]
fn issue_2039_v2_compaction_rejects_invalid_trigger_shapes() {
    let cases = [
        (
            json!([{"type":"message","role":"user","content":"missing"}]),
            "compaction",
        ),
        (
            json!([
                {"type":"compaction_trigger"},
                {"type":"compaction_trigger"}
            ]),
            "input",
        ),
        (
            json!([
                {"type":"compaction_trigger"},
                {"type":"message","role":"user","content":"not tail"}
            ]),
            "input",
        ),
        (
            json!([{"type":"compaction_trigger","unexpected":true}]),
            "input",
        ),
        (json!([{"type":""}, {"type":"compaction_trigger"}]), "input"),
        (
            json!(["not-an-object", {"type":"compaction_trigger"}]),
            "input",
        ),
    ];
    for (input, expected_param) in cases {
        let error = translate_response_request_with_context(
            json!({"model":"1flowbase","input":input}),
            OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(json!({
                "request_kind":"compaction",
                "compaction":{"implementation":"responses_compaction_v2"}
            })),
        )
        .expect_err("invalid V2 compaction trigger shape must fail closed");
        assert_eq!(error.code, "invalid_request");
        assert_eq!(error.param.as_deref(), Some(expected_param));
    }
}

#[test]
fn responses_compaction_preserves_input_above_the_former_item_limit() {
    let mut input = vec![json!({"type":"reasoning"}); 4_096];
    input.push(json!({"type":"compaction_trigger"}));
    let request = json!({"model":"1flowbase","input":input});
    let mut translated = translate_response_request_with_context(
        request.clone(),
        OpenAiResponsesRequestContext::responses(),
    )
    .expect("valid compaction history must not have a gateway-specific item ceiling");
    assert_eq!(
        *translated.request.execution.execution_operation(),
        domain::AiNativeOperation::Compact(domain::AiNativeCompactProfile::ResponsesCompactionV2)
    );
    assert!(translated.request.query.is_empty());
    assert!(translated.request.history.is_empty());
    assert_eq!(
        translated
            .request
            .metadata
            .take_provider_transport_payload()
            .unwrap()
            .wire_body(),
        &request
    );
}

// #2034 AC-002 diagnostic: the real HTTP ingress envelope must reconcile with
// the body declarations without inventing a conflict.
#[test]
fn issue_2034_ingress_envelope_headers_reconcile_with_body_identity() {
    use super::super::log_context::reconcile_client_log_headers;
    use crate::application_public_api::client_protocol_envelope::{
        capture_client_protocol_envelope, ClientProtocolIngressPolicy,
    };
    let thread = "probe-thread-1";
    let meta = json!({"thread_id":thread,"turn_id":"probe-turn-1","request_kind":"turn","session_id":thread}).to_string();
    let mut envelope = capture_client_protocol_envelope(
        ClientProtocolIngressPolicy::OpenAiResponses,
        [
            ("authorization", "Bearer secret"),
            ("content-type", "application/json"),
            ("thread-id", thread),
            ("session-id", thread),
            ("x-codex-turn-metadata", meta.as_str()),
        ],
    )
    .expect("headers captured");
    // Mirror bind_responses_session_context: the route replaces session-id
    // with a scoped digest before the run service reconciles the log context.
    envelope
        .headers
        .insert("session-id".into(), vec!["a".repeat(64)]);
    let translated = translate_response_request_with_context(
        json!({"model":"1flowbase","client_metadata":{"thread_id":thread,"turn_id":"probe-turn-1","session_id":thread,"x-codex-turn-metadata":meta},
            "input":[{"role":"user","content":"OK"}]}),
        OpenAiResponsesRequestContext::responses()
            .with_captured_codex_turn_metadata(serde_json::from_str(&meta).unwrap()),
    )
    .unwrap();
    let mut context = translated
        .request
        .metadata
        .application_run_log_context()
        .cloned()
        .unwrap();
    reconcile_client_log_headers(&mut context, &envelope.headers);
    assert_eq!(context.identity_status, "identified", "{context:?}");
    assert_eq!(context.session_id.as_deref(), Some(thread));
}

#[test]
fn issue_2046_responses_index_does_not_copy_opaque_content() {
    let input = json!([
        {"role":"user","content":"private-marker"},
        {"type":"custom_tool_call","call_id":"call_1","name":"exec","input":"{}"},
        {"type":"custom_tool_call_output","call_id":"call_1","output":"private-output"},
        {"role":"assistant","phase":"commentary","content":"private-agent-message"},
        {"type":"future_context_boundary","future":"private-extension"}
    ]);
    let index = ResponsesInputIndex::build(&input).expect("opaque items are indexable");
    assert_eq!(index.item_count(), 5);
    assert_eq!(index.current_tool_output_positions(), vec![2]);
    assert!(index.has_assistant_message_after_tool_outputs());
    assert_eq!(index.outputs_by_call_id()["call_1"], vec![2]);
    let mut continuing = input.as_array().unwrap().clone();
    continuing
        .push(json!({"type":"custom_tool_call_output","call_id":"call_2","output":"new result"}));
    assert!(!ResponsesInputIndex::build(&json!(continuing))
        .unwrap()
        .has_assistant_message_after_tool_outputs());
    let debug = format!("{index:?}");
    for private_value in [
        "private-marker",
        "private-output",
        "private-agent-message",
        "private-extension",
    ] {
        assert!(!debug.contains(private_value));
    }

    assert_eq!(
        ResponsesInputIndex::build(&json!(["invalid"])),
        Err(ResponsesInputIndexError::ItemKind { index: 0 })
    );
    assert_eq!(
        ResponsesInputIndex::build(&json!([{"type":42}])),
        Err(ResponsesInputIndexError::ItemType { index: 0 })
    );
}

#[test]
fn responses_envelope_preserves_large_input_and_validates_tail_items() {
    let mut input = vec![json!({"type":"message","role":"user","content":"history"}); 4_097];
    input.push(json!({"type":"function_call","call_id":"tail","name":"exec","arguments":"{}"}));
    input.push(json!({"type":"function_call_output","call_id":"tail","output":"tail-result"}));
    let request = json!({"model":"1flowbase","input":input});
    let envelope = OpenAiResponsesEnvelope::capture(request.clone())
        .expect("valid Responses history must not have a gateway-specific item ceiling");
    assert_eq!(envelope.raw_body(), &request);
    assert_eq!(envelope.provider_transport_payload().wire_body(), &request);
    let index = ResponsesRequestIndex::build(&request).unwrap();
    assert_eq!(index.input().item_count(), 4_099);
    assert_eq!(index.input().current_tool_output_positions(), vec![4_098]);
    assert_eq!(index.input().outputs_by_call_id()["tail"], vec![4_098]);

    let mut invalid = request;
    invalid["input"][4_098]["type"] = json!(42);
    assert_eq!(
        ResponsesRequestIndex::build(&invalid),
        Err(ResponsesInputIndexError::ItemType { index: 4_098 })
    );
}

#[test]
fn issue_2046_request_index_owns_the_continuation_identity() {
    let request = json!({
        "model":"fixture",
        "previous_response_id":"resp_round",
        "input":[{"type":"future_item","opaque":"private-body"}]
    });
    let index = ResponsesRequestIndex::build(&request).expect("request is indexable");
    assert_eq!(index.previous_response_id(), Some("resp_round"));
    assert_eq!(index.input().item_count(), 1);
    assert!(!format!("{index:?}").contains("private-body"));
    assert_eq!(
        ResponsesRequestIndex::build(&json!({"input":[],"previous_response_id":42})),
        Err(ResponsesInputIndexError::PreviousResponseId)
    );

    let envelope = OpenAiResponsesEnvelope::capture(request.clone()).expect("envelope captured");
    assert_eq!(envelope.previous_response_id(), Some("resp_round"));
    assert_eq!(envelope.raw_body(), &request);
    assert_eq!(envelope.provider_transport_payload().wire_body(), &request);
    assert!(!format!("{envelope:?}").contains("private-body"));

    let mut translated = translate_response_envelope_with_context_and_previous(
        envelope,
        OpenAiResponsesRequestContext::responses(),
        Some(OpenAiPreviousResponseContext {
            history: None,
            provider_continuation: Some(
                crate::ports::ProviderContinuation::new(
                    "resp_round",
                    crate::ports::ProviderTransportAffinity::new(
                        "instance", "provider", "model", "route",
                    ),
                )
                .unwrap(),
            ),
            response_id: "resp_round".into(),
            external_user: None,
            external_conversation_id: None,
            answer: None,
        }),
    )
    .expect("the same envelope should reach native translation");
    assert_eq!(
        translated
            .request
            .metadata
            .take_provider_transport_payload()
            .unwrap()
            .wire_body(),
        &request
    );
}

#[test]
fn openai_text_translation_preserves_nul_and_literal_escape_in_each_content_shape() {
    let text = "before\0after\\u0000";
    for content in [json!(text), json!([{ "type": "input_text", "text": text }])] {
        let mapped = super::super::response_input::openai_content(&content).unwrap();
        assert_eq!(mapped.text, text);
    }
    let mapped = super::super::response_input::openai_content(&json!([
        { "type": "text", "text": text },
        { "type": "image_url", "image_url": "https://example.com/image.png" }
    ]))
    .unwrap();
    assert_eq!(mapped.text, text);
    assert_eq!(mapped.content_blocks.unwrap()[0]["text"], json!(text));
}

#[test]
fn semantic_cursor_extends_compact_predecessor_without_transcript_materialization() {
    let first = json!({"model":"fixture", "input":[{"role":"user","content":"first"}]});
    let prefix = super::super::history::completed_history(&first, None, &[])
        .unwrap()
        .unwrap();
    let previous = OpenAiPreviousResponseContext {
        history: Some(serde_json::from_value(prefix.clone()).unwrap()),
        provider_continuation: None,
        response_id: "resp_first".into(),
        external_user: None,
        external_conversation_id: None,
        answer: Some("prior answer".into()),
    };
    let delta = json!({"model":"fixture", "previous_response_id":"resp_first", "input":[{"role":"user","content":"second"}]});
    let translated = translate_response_request_with_context_and_previous(
        delta.clone(),
        OpenAiResponsesRequestContext::responses(),
        Some(previous.clone()),
    )
    .unwrap();
    let expected = super::super::history::completed_history(&delta, Some(&prefix), &[])
        .unwrap()
        .unwrap();
    assert_eq!(translated.request.query, "second");
    assert_eq!(
        translated.request.metadata.responses_input_history(),
        Some(&expected)
    );
    assert_eq!(expected["version"], 3);
    assert_eq!(expected["item_count"], 2);
    assert_eq!(expected["digest"].as_str().unwrap().len(), 64);
    let mut invalid = previous;
    invalid.history.as_mut().unwrap().version = 2;
    assert!(translate_response_request_with_context_and_previous(
        delta,
        OpenAiResponsesRequestContext::responses(),
        Some(invalid),
    )
    .is_err());
}
