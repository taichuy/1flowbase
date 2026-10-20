use super::*;
use std::collections::BTreeMap;

use axum::body::Bytes;
use control_plane::application_public_api::callback_tool_ids::decode_openai_callback_tool_call_id;
use control_plane::application_public_api::compat::openai::{
    translate_response_request_with_context_and_previous, OpenAiResponsesRequestContext,
};
use control_plane::application_public_api::native::{
    NativeRequiredAction, NativeRunStatus, NativeUsage,
};
use control_plane::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationSafeRepresentation,
};
use control_plane::ports::{ProviderProtocolContextSlotId, ProviderTransportStore};
use plugin_framework::provider_contract::ProtocolContextEnvelope;
use storage_ephemeral::MemoryProviderTransportStore;
use time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

fn blocking_run(status: NativeRunStatus) -> NativeRunResult {
    NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("must-not-successify".to_string()),
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn chat_usage_details_unary_preserves_totals_zero_and_unknown() {
    for (cache_read, cache_hit, reasoning, expected_details) in [
        (Some(4096), Some(3000), Some(80), Some(4096)),
        (Some(0), Some(4096), Some(0), Some(0)),
        (None, Some(4096), None, Some(4096)),
        (None, None, None, None),
    ] {
        let mut run = blocking_run(NativeRunStatus::Succeeded);
        run.usage = Some(NativeUsage {
            prompt_tokens: Some(4107),
            completion_tokens: Some(120),
            total_tokens: Some(4227),
            cache_read_tokens: cache_read,
            input_cache_hit_tokens: cache_hit,
            reasoning_tokens: reasoning,
            ..NativeUsage::default()
        });
        let response = serde_json::to_value(
            to_openai_response(run, "deepseek-flash".into(), "test".into()).unwrap(),
        )
        .unwrap();
        let usage = &response["usage"];
        assert_eq!(usage["prompt_tokens"], 4107);
        assert_eq!(usage["completion_tokens"], 120);
        assert_eq!(usage["total_tokens"], 4227);
        match expected_details {
            Some(cached) => assert_eq!(usage["prompt_tokens_details"]["cached_tokens"], cached),
            None => assert!(usage.get("prompt_tokens_details").is_none()),
        }
        match reasoning {
            Some(tokens) => assert_eq!(
                usage["completion_tokens_details"]["reasoning_tokens"],
                tokens
            ),
            None => assert!(usage.get("completion_tokens_details").is_none()),
        }
    }
    assert_eq!(
        serde_json::to_value(openai_usage(None)).unwrap(),
        json!({"prompt_tokens":0,"completion_tokens":0,"total_tokens":0})
    );
}

#[test]
fn responses_usage_details_unary_preserves_totals_and_default() {
    let mut run = blocking_run(NativeRunStatus::Succeeded);
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(33024),
        completion_tokens: Some(120),
        total_tokens: Some(33144),
        input_cache_hit_tokens: Some(3000),
        cache_read_tokens: Some(3328),
        reasoning_tokens: Some(80),
        ..NativeUsage::default()
    });
    let response = serde_json::to_value(
        to_openai_responses_response(run, "provider/model".into(), None)
            .expect("completed response should project"),
    )
    .expect("Responses JSON should serialize");
    assert_eq!(
        response["usage"],
        json!({
            "input_tokens": 33024,
            "input_tokens_details": { "cached_tokens": 3328 },
            "output_tokens": 120,
            "output_tokens_details": { "reasoning_tokens": 80 },
            "total_tokens": 33144
        })
    );

    let absent = serde_json::to_value(
        to_openai_responses_response(
            blocking_run(NativeRunStatus::Succeeded),
            "provider/model".into(),
            None,
        )
        .expect("completed response without usage should project"),
    )
    .expect("default Responses JSON should serialize");
    assert_eq!(
        absent["usage"],
        json!({
            "input_tokens": 0,
            "output_tokens": 0,
            "total_tokens": 0
        })
    );

    let hit_only = NativeUsage {
        input_cache_hit_tokens: Some(17),
        ..NativeUsage::default()
    };
    let projected = serde_json::to_value(openai_responses_usage(Some(&hit_only)))
        .expect("fallback usage should serialize");
    assert_eq!(projected["input_tokens_details"]["cached_tokens"], 17);
}

#[test]
fn issue_2048_cancelled_previous_response_status_is_not_continuable() {
    let error = ensure_previous_response_is_usable(&blocking_run(NativeRunStatus::Cancelled))
        .expect_err("a superseded response must fail before capsule lookup");
    let response = error.into_response();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(ensure_previous_response_is_usable(&blocking_run(NativeRunStatus::Succeeded)).is_ok());
}

#[test]
fn websocket_operation_uses_current_frame_metadata_instead_of_stale_handshake() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-codex-turn-metadata",
        json!({"request_kind":"turn"}).to_string().parse().unwrap(),
    );
    let local_summary = json!({
        "request_kind":"compaction", "compaction":{"implementation":"responses"}
    });
    let body = json!({
        "model":"1flowbase", "input":"summarize",
        "client_metadata":{"x-codex-turn-metadata":local_summary.to_string()}
    });
    let context = compact::responses_request_context(
        &headers,
        OpenAiResponsesEndpoint::Responses,
        &body,
        OpenAiResponseDelivery::TypedEvents,
    )
    .unwrap();
    let envelope = OpenAiResponsesEnvelope::capture(body).unwrap();
    assert!(classify_response_envelope_operation(&envelope, context)
        .unwrap()
        .is_local_summary());

    let ordinary = json!({
        "model":"1flowbase", "input":"please summarize",
        "client_metadata":{"x-codex-turn-metadata":json!({"request_kind":"turn"}).to_string()}
    });
    let context = compact::responses_request_context(
        &headers,
        OpenAiResponsesEndpoint::Responses,
        &ordinary,
        OpenAiResponseDelivery::TypedEvents,
    )
    .unwrap();
    let envelope = OpenAiResponsesEnvelope::capture(ordinary).unwrap();
    assert!(!classify_response_envelope_operation(&envelope, context)
        .unwrap()
        .is_local_summary());
}

#[test]
fn http_conflicting_codex_header_and_body_metadata_is_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-codex-turn-metadata",
        json!({"request_kind":"turn"}).to_string().parse().unwrap(),
    );
    let body = json!({
        "client_metadata":{"x-codex-turn-metadata":json!({
            "request_kind":"compaction", "compaction":{"implementation":"responses"}
        }).to_string()}
    });
    assert!(compact::responses_request_context(
        &headers,
        OpenAiResponsesEndpoint::Responses,
        &body,
        OpenAiResponseDelivery::Http,
    )
    .is_err());
}

#[test]
fn issue_2048_low_budget_local_summary_keeps_one_opaque_responses_ingress_plan() {
    let wire_body = json!({
        "model":"1flowbase",
        "input":[
            {"type":"message","role":"user","content":"inspect"},
            {"type":"custom_tool_call","call_id":"call-1","name":"exec","input":"{}"},
            {"type":"custom_tool_call_output","call_id":"call-1","output":"private"},
            {"type":"message","role":"user","content":"summarize"}
        ]
    });
    let mut translated = control_plane::application_public_api::compat::openai::translate_response_request_with_context(
        wire_body.clone(),
        OpenAiResponsesRequestContext::responses().with_captured_codex_turn_metadata(json!({
            "thread_id":"thread-low-budget",
            "turn_id":"turn-low-budget",
            "request_kind":"compaction",
            "compaction":{"implementation":"responses"}
        })),
    )
    .expect("the same bounded Local Summary request used by HTTP/SSE/WebSocket should translate");

    assert_eq!(
        *translated.request.execution.execution_operation(),
        AiNativeOperation::Generate(domain::AiNativeGenerateProfile::LocalSummary)
    );
    assert!(translated.request.query.is_empty());
    assert!(translated.request.history.is_empty());
    assert!(translated.request.system.is_empty());
    let payload = translated
        .request
        .metadata
        .take_provider_transport_payload()
        .expect("Local Summary ingress must carry provider-opaque transport");
    assert_eq!(payload.wire_body(), &wire_body);
    assert_eq!(
        compat_sse::ResponsesProjectionMode::from_transport(Some(&payload)),
        compat_sse::ResponsesProjectionMode::TransparentProviderResponses
    );
}

#[test]
fn issue_2052_encoded_callback_selects_restored_provider_transport() {
    let callback_task_id = Uuid::now_v7();
    let external_call_id = encode_openai_callback_tool_call_id(callback_task_id, "call_provider");
    let notice = json!({"type":"function_call_output","name":"send_message_to_thread","namespace":"codex_app","output":"Continue","extension":{"source":"another-thread"}});
    let envelope = OpenAiResponsesEnvelope::capture(json!({
        "model": "gpt-test",
        "store": false,
        "input": [
            {"type": "function_call", "call_id": external_call_id, "name": "exec", "arguments": "{}"},
            {"type": "function_call_output", "call_id": external_call_id, "output": "done"},
            notice
        ]
    }))
    .expect("Responses callback envelope should be valid");

    let transport = responses_callback_provider_transport(&envelope, false, true)
        .expect("encoded callback should prepare transport")
        .expect("encoded callback must not fall back to semantic rendering");

    assert_eq!(transport.wire_body()["store"], false);
    assert_eq!(transport.wire_body()["input"][2], notice);
    assert_eq!(
        transport.wire_body()["input"][0]["call_id"],
        "call_provider"
    );
    assert_eq!(
        transport.wire_body()["input"][1]["call_id"],
        "call_provider"
    );
    assert!(
        responses_resume_requires_previous_response_match(false),
        "encoded correlation must still validate previous_response_id even with provider transport"
    );
    assert_eq!(
        compat_sse::ResponsesProjectionMode::from_transport(Some(&transport)),
        compat_sse::ResponsesProjectionMode::TransparentProviderResponses,
        "transport representation remains independent from callback correlation"
    );
    assert!(
        !responses_resume_requires_previous_response_match(true),
        "native-state correlation already owns the callback-to-run relationship"
    );
}

#[test]
fn issue_2062_native_callback_restores_stale_gateway_ids_before_provider_transport() {
    let stale_callback_task_id = Uuid::now_v7();
    let provider_call_id = "call_sFtx4FZ7vOajvx2J4rzlnd5H";
    let stale_external_call_id =
        encode_openai_callback_tool_call_id(stale_callback_task_id, provider_call_id);
    assert_eq!(provider_call_id.len(), 29);
    assert_eq!(stale_external_call_id.len(), 71);

    let envelope = OpenAiResponsesEnvelope::capture(json!({
        "model": "gpt-test",
        "store": false,
        "input": [
            {"type": "function_call", "call_id": stale_external_call_id, "name": "exec", "arguments": "{}"},
            {"type": "function_call_output", "call_id": stale_external_call_id, "output": "old result"},
            {"type": "custom_tool_call", "call_id": "call_current", "name": "exec", "input": "pwd"},
            {"type": "custom_tool_call_output", "call_id": "call_current", "output": "workspace"}
        ]
    }))
    .expect("mixed Responses callback envelope should be valid");

    let transport = responses_callback_provider_transport(&envelope, true, false)
        .expect("native callback should prepare transport")
        .expect("native callback must retain provider transport");

    assert_eq!(
        transport.wire_body()["input"][0]["call_id"],
        provider_call_id
    );
    assert_eq!(
        transport.wire_body()["input"][1]["call_id"],
        provider_call_id
    );
    assert_eq!(transport.wire_body()["input"][2]["call_id"], "call_current");
    assert_eq!(transport.wire_body()["input"][3]["call_id"], "call_current");
}

#[tokio::test]
async fn wp_d1c_compatible_ingress_stages_raw_protocol_context_outside_the_run_payload() {
    const CANARY: &str = "WP-D1C-INGRESS-RAW-CANARY";
    let store = MemoryProviderTransportStore::new(Duration::minutes(5));
    let mut run = blocking_run(NativeRunStatus::Queued);
    run.id = Uuid::now_v7();
    let protocol_context = ProtocolContextEnvelope {
        source_protocol: "openai_chat".to_string(),
        query: BTreeMap::new(),
        headers: BTreeMap::from([("openai-organization".to_string(), vec![CANARY.to_string()])]),
        body: BTreeMap::new(),
        ..ProtocolContextEnvelope::default()
    };

    native::stage_client_protocol_context(&store, &run, Some(protocol_context.clone()))
        .await
        .expect("compatible ingress context should stage");

    let stored = store
        .get_protocol_context(ProviderProtocolContextSlotId::for_original_flow_run(run.id))
        .await
        .unwrap()
        .expect("original flow context should stay available inside its TTL");
    assert_eq!(
        stored.into_value(),
        serde_json::to_value(protocol_context).unwrap()
    );
    assert!(!run.node_input_payload.to_string().contains(CANARY));
}

#[test]
fn d2_ac_001_openai_malformed_json_uses_the_endpoint_protocol_and_safe_receipt() {
    let sentinel = "D2-OPENAI-MALFORMED-JSON-MUST-NOT-REACH-RECEIPT";
    for protocol in [
        TranslationProtocol::OpenAiChat,
        TranslationProtocol::OpenAiResponses,
    ] {
        let error =
            parse_openai_json_body(Bytes::from(format!("{{\"raw\":\"{sentinel}\"")), protocol)
                .expect_err("malformed OpenAI JSON must be rejected by the adapter boundary");
        let OpenAiRouteError::Compat(error) = error else {
            panic!("malformed JSON must remain an OpenAI adapter error");
        };

        assert_eq!(error.report.protocol, protocol);
        assert_eq!(error.report.decisions.len(), 1);
        let decision = &error.report.decisions[0];
        assert_eq!(decision.source_path, "$.body");
        assert_eq!(decision.kind, TranslationDecisionKind::Rejected);
        assert_eq!(
            decision.effective_value,
            TranslationSafeRepresentation::Present
        );
        assert!(
            !serde_json::to_string(&error.report)
                .expect("receipt should serialize")
                .contains(sentinel),
            "malformed JSON must not be retained in the receipt"
        );
    }
}

#[test]
fn openai_chat_ingress_builds_one_safe_query_header_and_body_protocol_context() {
    let mut headers = HeaderMap::new();
    headers.append("openai-organization", "org-one".parse().unwrap());
    headers.append("openai-organization", "org-two".parse().unwrap());
    headers.insert("authorization", "Bearer platform-key".parse().unwrap());
    headers.insert("cookie", "session=secret".parse().unwrap());
    headers.insert("host", "api.example.test".parse().unwrap());
    headers.insert("connection", "keep-alive, x-hop-secret".parse().unwrap());
    headers.insert("x-hop-secret", "hop-secret".parse().unwrap());
    let translated = translate_chat_completion_request(json!({
        "model": "gpt-compatible",
        "messages": [{"role": "user", "content": "hello"}],
        "future_chat_option": {"shape": "opaque"},
        "authorization": "body-secret",
        "__native_transport": {"must_not_cross": true}
    }))
    .expect("safe Chat residual fixture should translate");

    let envelope = openai_protocol_context_from_ingress(
        ClientProtocolIngressPolicy::OpenAiChat,
        Some("preview=one&preview=two&x_api_key=query-secret&__native_transport=internal"),
        &headers,
        translated.request.client_protocol_envelope,
    )
    .expect("safe Chat ingress context should produce one envelope");

    assert_eq!(envelope.source_protocol, "openai_chat");
    assert_eq!(
        envelope.headers["openai-organization"],
        vec!["org-one", "org-two"]
    );
    assert_eq!(envelope.query["preview"], vec!["one", "two"]);
    assert_eq!(envelope.body["future_chat_option"]["shape"], "opaque");
    for stripped in [
        "authorization",
        "cookie",
        "host",
        "connection",
        "x-hop-secret",
    ] {
        assert!(!envelope.headers.contains_key(stripped), "{stripped}");
    }
    for stripped in ["x_api_key", "__native_transport"] {
        assert!(!envelope.query.contains_key(stripped), "{stripped}");
    }
    for stripped in ["model", "messages", "authorization", "__native_transport"] {
        assert!(!envelope.body.contains_key(stripped), "{stripped}");
    }
}

#[test]
fn openai_responses_ingress_subtracts_typed_codex_header_and_body_semantics() {
    let mut headers = HeaderMap::new();
    headers.insert("openai-project", "project-one".parse().unwrap());
    headers.insert(
        "x-codex-turn-metadata",
        r#"{"compaction":{"implementation":"responses_compact"}}"#
            .parse()
            .unwrap(),
    );
    headers.insert("x-api-key", "platform-key".parse().unwrap());
    let translated = translate_response_request_with_context_and_previous(
        json!({
            "model": "gpt-compatible",
            "input": "hello",
            "stream": true,
            "future_responses_option": {"shape": "opaque"}
        }),
        OpenAiResponsesRequestContext::responses(),
        None,
    )
    .expect("safe Responses residual fixture should translate");

    let envelope = openai_protocol_context_from_ingress(
        ClientProtocolIngressPolicy::OpenAiResponses,
        Some("preview=one&preview=two"),
        &headers,
        translated.request.client_protocol_envelope,
    )
    .expect("safe Responses ingress context should produce one envelope");

    assert_eq!(envelope.source_protocol, "openai_responses");
    assert_eq!(envelope.headers["openai-project"], vec!["project-one"]);
    assert!(!envelope.headers.contains_key("x-codex-turn-metadata"));
    assert!(!envelope.headers.contains_key("x-api-key"));
    assert_eq!(envelope.query["preview"], vec!["one", "two"]);
    assert_eq!(envelope.body["future_responses_option"]["shape"], "opaque");
    for typed in ["model", "input", "stream"] {
        assert!(!envelope.body.contains_key(typed), "{typed}");
    }
}

#[test]
fn issue_2039_compact_translation_retains_the_exact_opaque_provider_body() {
    let wire_body = json!({
        "model":"gpt-compatible",
        "input":[
            {"type":"message","role":"user","content":"inspect"},
            {"type":"reasoning","encrypted_content":"opaque"},
            {"type":"custom_tool_call","call_id":"call_1","name":"exec","input":"{}"},
            {"type":"custom_tool_call_output","call_id":"call_1","output":"done"},
            {"type":"compaction_trigger"}
        ]
    });
    let translated = translate_response_request_with_context_and_previous(
        wire_body.clone(),
        OpenAiResponsesRequestContext::responses(),
        None,
    )
    .expect("V2 compaction should translate");
    let mut request = translated.request;

    let payload = request
        .metadata
        .take_provider_transport_payload()
        .expect("Compact provider transport must be present");
    assert_eq!(payload.wire_body(), &wire_body);
    assert!(!payload.digest().is_empty());
}

#[test]
fn openai_response_projects_native_tool_calls() {
    let callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({ "callback_task_id": callback_task_id, "callback_kind": "llm_tool_calls" }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_123",
                "name": "lookup_order",
                "arguments": {"order_id": "order_123"}
            }
        ])),
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(
        to_openai_response(
            run,
            "provider/model".into(),
            "chatcmpl-test-tool-call".to_string(),
        )
        .expect("waiting external tool call should project"),
    )
    .expect("openai response serializes");

    assert_eq!(payload["id"], json!("chatcmpl-test-tool-call"));
    assert_eq!(payload["choices"][0]["finish_reason"], json!("tool_calls"));
    assert_eq!(
        payload["choices"][0]["message"]["tool_calls"][0]["function"]["name"],
        json!("lookup_order")
    );
    assert_eq!(
        payload["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
        json!("{\"order_id\":\"order_123\"}")
    );
}

#[test]
fn openai_response_filters_internal_visible_llm_tool_calls() {
    let callback_task_id = Uuid::from_u128(0xabababababababababababababababab);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("visible internal LLM output".to_string()),
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({
                "callback_task_id": callback_task_id,
                "callback_kind": "llm_tool_calls"
            }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_internal",
                "type": "visible_internal_llm_tool",
                "name": "inspect_visible_context",
                "arguments": {"query": "visible"}
            }
        ])),
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let chat_payload = serde_json::to_value(
        to_openai_response(
            run.clone(),
            "provider/model".into(),
            "chatcmpl-internal".to_string(),
        )
        .expect("succeeded run should project"),
    )
    .expect("openai chat response serializes");
    let responses_payload = serde_json::to_value(
        to_openai_responses_response(run, "provider/model".into(), None)
            .expect("waiting external tool call should project"),
    )
    .expect("openai responses object serializes");

    assert_eq!(chat_payload["choices"][0]["finish_reason"], json!("stop"));
    assert_eq!(
        chat_payload["choices"][0]["message"]["content"],
        json!("visible internal LLM output")
    );
    assert!(chat_payload["choices"][0]["message"]["tool_calls"].is_null());
    assert_eq!(
        responses_payload["output_text"],
        json!("visible internal LLM output")
    );
    assert_eq!(responses_payload["output"][0]["type"], json!("message"));
    assert!(responses_payload["output"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["type"] != json!("function_call")));
}

#[test]
fn openai_response_encodes_callback_task_id_into_tool_call_ids() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("need tool".to_string()),
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({ "callback_task_id": callback_task_id }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_123",
                "name": "lookup_order",
                "arguments": {"order_id": "order_123"}
            }
        ])),
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(
        to_openai_response(
            run,
            "provider/model".into(),
            "chatcmpl-test-callback".to_string(),
        )
        .expect("succeeded run should project"),
    )
    .expect("openai response serializes");

    let tool_call_id = payload["choices"][0]["message"]["tool_calls"][0]["id"]
        .as_str()
        .expect("tool call id should be a string");
    assert!(tool_call_id.starts_with(
        control_plane::application_public_api::callback_tool_ids::OPENAI_CALLBACK_TOOL_CALL_PREFIX
    ));
    assert_eq!(
        decode_openai_callback_tool_call_id(tool_call_id),
        Some((callback_task_id, "call_123".to_string()))
    );
}

#[test]
fn openai_responses_response_preserves_provider_tool_call_id() {
    let callback_task_id = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: Some("".to_string()),
        answer_segments: None,
        required_action: Some(NativeRequiredAction {
            action_type: "submit_tool_outputs".to_string(),
            payload: json!({ "callback_task_id": callback_task_id, "callback_kind": "llm_tool_calls" }),
        }),
        tool_calls: Some(json!([
            {
                "id": "call_inventory",
                "name": "lookup_inventory",
                "arguments": {"sku": "sku_123"}
            }
        ])),
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(
        to_openai_responses_response(run, "provider/model".into(), Some("resp_previous".into()))
            .expect("waiting external tool call should project"),
    )
    .expect("responses object serializes");

    assert_eq!(payload["status"], json!("completed"));
    assert_eq!(payload["output_text"], json!(""));
    assert_eq!(payload["output"][0]["type"], json!("function_call"));
    assert_eq!(payload["output"][0]["name"], json!("lookup_inventory"));
    assert_eq!(
        payload["output"][0]["arguments"],
        json!("{\"sku\":\"sku_123\"}")
    );
    let call_id = payload["output"][0]["call_id"]
        .as_str()
        .expect("call_id should be present");
    assert_eq!(call_id, "call_inventory");
}

#[test]
fn d2_ac_004_openai_blocking_terminal_status_matrix() {
    let chat_incomplete = serde_json::to_value(
        to_openai_response(
            blocking_run(NativeRunStatus::Incomplete),
            "provider/model".into(),
            "chatcmpl-incomplete".to_string(),
        )
        .expect("incomplete Chat run should project"),
    )
    .expect("Chat response serializes");
    assert_eq!(
        chat_incomplete["choices"][0]["finish_reason"],
        json!("length")
    );
    assert_eq!(
        chat_incomplete["choices"][0]["message"]["content"],
        json!("must-not-successify")
    );

    let responses_incomplete = serde_json::to_value(
        to_openai_responses_response(
            blocking_run(NativeRunStatus::Incomplete),
            "provider/model".into(),
            None,
        )
        .expect("incomplete Responses run should project"),
    )
    .expect("Responses response serializes");
    assert_eq!(responses_incomplete["status"], json!("incomplete"));
    assert_eq!(
        responses_incomplete["incomplete_details"]["reason"],
        json!("max_output_tokens")
    );
    assert_ne!(responses_incomplete["status"], json!("completed"));

    for status in [NativeRunStatus::Failed, NativeRunStatus::Cancelled] {
        assert!(matches!(
            to_openai_response(
                blocking_run(status),
                "provider/model".into(),
                "chatcmpl-terminal-error".to_string(),
            ),
            Err(OpenAiRouteError::Native(_))
        ));
        assert!(matches!(
            to_openai_responses_response(blocking_run(status), "provider/model".into(), None,),
            Err(OpenAiRouteError::Native(_))
        ));
    }

    assert!(matches!(
        to_openai_response(
            blocking_run(NativeRunStatus::Waiting),
            "provider/model".into(),
            "chatcmpl-waiting".to_string(),
        ),
        Err(OpenAiRouteError::RequiredAction)
    ));
    assert!(matches!(
        to_openai_responses_response(
            blocking_run(NativeRunStatus::Waiting),
            "provider/model".into(),
            None,
        ),
        Err(OpenAiRouteError::RequiredAction)
    ));
}
mod blocking_projection;
