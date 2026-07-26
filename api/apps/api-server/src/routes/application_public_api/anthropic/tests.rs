use super::*;
use axum::body::Bytes;
use control_plane::application_public_api::callback_tool_ids::decode_anthropic_callback_tool_use_id;
use control_plane::application_public_api::native::{NativeRequiredAction, NativeRunStatus};
use control_plane::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationSafeRepresentation,
};
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
fn d2_ac_001_anthropic_malformed_json_has_one_safe_adapter_receipt() {
    let sentinel = "D2-ANTHROPIC-MALFORMED-JSON-MUST-NOT-REACH-RECEIPT";
    let error = parse_anthropic_json_body(Bytes::from(format!("{{\"raw\":\"{sentinel}\"")))
        .expect_err("malformed Anthropic JSON must be rejected by the adapter boundary");
    let AnthropicRouteError::Compat(error) = error else {
        panic!("malformed JSON must remain an Anthropic adapter error");
    };

    assert_eq!(
        error.report.protocol,
        TranslationProtocol::AnthropicMessages
    );
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

#[test]
fn anthropic_response_projects_native_tool_calls() {
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: Some(json!([
            {
                "id": "toolu_123",
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
        to_anthropic_response(run, "provider/model".into()).expect("succeeded run should project"),
    )
    .expect("anthropic response serializes");

    assert_eq!(payload["stop_reason"], json!("tool_use"));
    assert_eq!(payload["content"][0]["type"], json!("tool_use"));
    assert_eq!(payload["content"][0]["name"], json!("lookup_order"));
    assert_eq!(
        payload["content"][0]["input"]["order_id"],
        json!("order_123")
    );
}

#[test]
fn anthropic_resume_rejects_mixed_callback_groups() {
    let first_callback = Uuid::from_u128(0x11111111111111111111111111111111);
    let latest_callback = Uuid::from_u128(0x22222222222222222222222222222222);
    let request = json!({
        "messages": [
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": encode_anthropic_callback_tool_use_id(latest_callback, "toolu_latest"),
                    "name": "lookup_latest",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": encode_anthropic_callback_tool_use_id(first_callback, "toolu_first"),
                    "content": "FIRST"
                },
                {
                    "type": "tool_result",
                    "tool_use_id": encode_anthropic_callback_tool_use_id(latest_callback, "toolu_latest"),
                    "content": "LATEST",
                    "is_error": true
                }
                ]
            }
        ]
    });

    assert!(correlate_anthropic_callback(&request).is_err());
}

#[test]
fn ac_001_anthropic_tool_result_mixed_with_new_text_starts_a_new_run() {
    let callback_task_id = Uuid::from_u128(0x33333333333333333333333333333333);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_read");
    let request = json!({
        "messages": [
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": tool_use_id,
                    "name": "Read",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": "old result"
                    },
                    {
                        "type": "text",
                        "text": "please answer the new question"
                    }
                ]
            }
        ]
    });

    let resume = correlate_anthropic_callback(&request).expect("request should parse");

    assert!(
        resume.is_none(),
        "mixed new user text must create a new run"
    );
}

#[test]
fn ac_002_anthropic_orphan_tool_result_is_invalid() {
    let callback_task_id = Uuid::from_u128(0x44444444444444444444444444444444);
    let request = json!({
        "messages": [{
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_stale"),
                "content": "stale result"
            }]
        }]
    });

    assert!(correlate_anthropic_callback(&request).is_err());
}

#[test]
fn anthropic_response_filters_internal_visible_llm_tool_calls() {
    let callback_task_id = Uuid::from_u128(0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd);
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
                "id": "toolu_internal",
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

    let payload = serde_json::to_value(
        to_anthropic_response(run, "provider/model".into()).expect("succeeded run should project"),
    )
    .expect("anthropic response serializes");

    assert_eq!(payload["stop_reason"], json!("end_turn"));
    assert_eq!(payload["content"][0]["type"], json!("text"));
    assert_eq!(
        payload["content"][0]["text"],
        json!("visible internal LLM output")
    );
    assert!(payload["content"]
        .as_array()
        .unwrap()
        .iter()
        .all(|block| block["type"] != json!("tool_use")));
}

#[test]
fn anthropic_response_preserves_canonical_answer_with_marker_like_text() {
    let run = NativeRunResult {
            id: Uuid::nil(),
            application_id: Uuid::nil(),
            api_key_id: Uuid::nil(),
            publication_version_id: Uuid::nil(),
            status: NativeRunStatus::Succeeded,
            node_input_payload: json!({}),
            metadata: json!({}),
            answer: Some(
                "<think>private reasoning</think>raw draft<tool_call>{}</tool_call>\n\n---\n\n下面是美化后内容\n\nVisible answer"
                    .to_string(),
            ),
            answer_segments: None,
            required_action: None,
            tool_calls: None,
            usage: None,
            error: None,
            operation_terminal: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        };

    let payload = serde_json::to_value(
        to_anthropic_response(run, "provider/model".into()).expect("succeeded run should project"),
    )
    .expect("anthropic response serializes");

    assert_eq!(
        payload["content"][0]["text"],
        json!("<think>private reasoning</think>raw draft<tool_call>{}</tool_call>\n\n---\n\n下面是美化后内容\n\nVisible answer")
    );
}

#[test]
fn claude_code_session_header_fills_missing_metadata_session_id() {
    let mut request = json!({
        "model": "1flowbase",
        "messages": [{"role": "user", "content": "hi"}]
    });
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-claude-code-session-id",
        "header-session-123".parse().unwrap(),
    );

    merge_claude_code_session_header(&mut request, &headers);

    assert_eq!(
        request["metadata"]["session_id"],
        json!("header-session-123")
    );
}

#[test]
fn anthropic_ingress_captures_client_protocol_envelope_from_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
    headers.insert("anthropic-beta", "prompt-caching".parse().unwrap());
    headers.insert(
        "x-claude-code-session-id",
        "header-session-123".parse().unwrap(),
    );
    headers.insert("authorization", "Bearer platform-key".parse().unwrap());
    headers.insert("content-length", "42".parse().unwrap());

    let envelope = anthropic_client_protocol_envelope_from_headers(&headers)
        .expect("anthropic headers should produce client protocol envelope");

    assert_eq!(envelope.source_protocol, "anthropic_messages");
    assert_eq!(
        envelope
            .headers
            .get("anthropic-version")
            .map(String::as_str),
        Some("2023-06-01")
    );
    assert_eq!(
        envelope
            .headers
            .get("x-claude-code-session-id")
            .map(String::as_str),
        Some("header-session-123")
    );
    assert!(!envelope.headers.contains_key("authorization"));
    assert!(!envelope.headers.contains_key("content-length"));
}

#[test]
fn anthropic_response_encodes_callback_task_id_into_tool_use_ids() {
    let callback_task_id = Uuid::from_u128(0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Succeeded,
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
                "id": "toolu_123",
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
        to_anthropic_response(run, "provider/model".into()).expect("succeeded run should project"),
    )
    .expect("anthropic response serializes");

    let tool_use_id = payload["content"][0]["id"]
        .as_str()
        .expect("tool_use id should be encoded");
    assert_eq!(
        decode_anthropic_callback_tool_use_id(tool_use_id),
        Some((callback_task_id, "toolu_123".to_string()))
    );
}

#[test]
fn d2_ac_004_anthropic_blocking_terminal_status_matrix() {
    let incomplete = serde_json::to_value(
        to_anthropic_response(
            blocking_run(NativeRunStatus::Incomplete),
            "provider/model".into(),
        )
        .expect("incomplete Anthropic run should project"),
    )
    .expect("Anthropic response serializes");
    assert_eq!(incomplete["stop_reason"], json!("max_tokens"));
    assert_eq!(
        incomplete["content"][0]["text"],
        json!("must-not-successify")
    );

    for status in [NativeRunStatus::Failed, NativeRunStatus::Cancelled] {
        assert!(matches!(
            to_anthropic_response(blocking_run(status), "provider/model".into()),
            Err(AnthropicRouteError::Native(_))
        ));
    }
    let callback_task_id = Uuid::from_u128(0xdddddddddddddddddddddddddddddddd);
    let mut waiting = blocking_run(NativeRunStatus::Waiting);
    waiting.required_action = Some(NativeRequiredAction {
        action_type: "submit_tool_outputs".to_string(),
        payload: json!({
            "callback_task_id": callback_task_id,
            "callback_kind": "llm_tool_calls"
        }),
    });
    waiting.tool_calls = Some(json!([{
        "id": "toolu_lookup",
        "name": "lookup",
        "arguments": {"query": "order"}
    }]));
    let waiting = serde_json::to_value(
        to_anthropic_response(waiting, "provider/model".into())
            .expect("AC-003 waiting tool callbacks should project"),
    )
    .expect("Anthropic response serializes");
    assert_eq!(waiting["stop_reason"], json!("tool_use"));
    assert!(waiting["content"].as_array().is_some_and(|blocks| blocks
        .iter()
        .any(|block| block["type"] == json!("tool_use"))));
}

#[test]
fn d2_ac_007_anthropic_prompt_marker_control_is_explicitly_unsupported() {
    let marker_error = control_plane::application_public_api::compat::anthropic::translate_messages_request(
        json!({
            "model": "1flowbase",
            "system": "Generate a concise, sentence-case title. Return JSON with a single \"title\" field",
            "messages": [{"role": "user", "content": "continue"}]
        }),
    )
    .expect_err("prompt-marker control has no D2 canonical owner");
    assert!(marker_error.report.has_decision(
        "$.system",
        control_plane::application_public_api::protocol_translation::TranslationDecisionKind::Unsupported,
    ));
}
