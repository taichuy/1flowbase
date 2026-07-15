use super::*;
use control_plane::application_public_api::native::{NativeRequiredAction, NativeRunStatus};
use time::OffsetDateTime;
use uuid::Uuid;

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
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(to_anthropic_response(run, "provider/model".into()))
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
fn anthropic_response_filters_internal_visible_llm_tool_calls() {
    let callback_task_id = Uuid::from_u128(0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd);
    let run = NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Waiting,
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
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(to_anthropic_response(run, "provider/model".into()))
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
fn anthropic_response_projects_only_visible_assistant_text() {
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
            created_at: OffsetDateTime::UNIX_EPOCH,
        };

    let payload = serde_json::to_value(to_anthropic_response(run, "provider/model".into()))
        .expect("anthropic response serializes");

    assert_eq!(payload["content"][0]["text"], json!("Visible answer"));
}

#[test]
fn anthropic_count_tokens_estimates_messages_and_tools() {
    let without_tools = anthropic_count_input_tokens(&json!({
        "model": "1flowbase",
        "messages": [{"role": "user", "content": "hello"}]
    }));
    let with_tools = anthropic_count_input_tokens(&json!({
        "model": "1flowbase",
        "messages": [{"role": "user", "content": "hello"}],
        "tools": [{
            "name": "lookup_order",
            "description": "Find an order",
            "input_schema": {"type": "object"}
        }]
    }));

    assert!(without_tools > 0);
    assert!(with_tools > without_tools);
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
                "id": "toolu_123",
                "name": "lookup_order",
                "arguments": {"order_id": "order_123"}
            }
        ])),
        usage: None,
        error: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    };

    let payload = serde_json::to_value(to_anthropic_response(run, "provider/model".into()))
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
fn anthropic_tool_resume_request_decodes_tool_result_blocks() {
    let callback_task_id = Uuid::from_u128(0xffffffffffffffffffffffffffffffff);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_123");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": tool_use_id,
                    "name": "lookup_order",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": [{"type": "text", "text": "{\"order\":\"ready\"}"}]
                    }
                ]
            }
        ]
    }))
    .expect("tool_result should parse")
    .expect("encoded tool_result should resume callback");

    assert_eq!(resume.callback_task_id, callback_task_id);
    assert_eq!(resume.tool_results[0]["tool_call_id"], json!("toolu_123"));
    assert_eq!(
        resume.tool_results[0]["content"],
        json!("{\"order\":\"ready\"}")
    );
}

#[test]
fn anthropic_tool_resume_request_accepts_hidden_system_reminder_text() {
    let callback_task_id = Uuid::from_u128(0xffffffffffffffffffffffffffffffff);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_123");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": tool_use_id,
                    "name": "Grep",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": "Found 3 files"
                    },
                    {
                        "type": "text",
                        "text": "<system-reminder>Claude Code internal reminder</system-reminder>"
                    }
                ]
            }
        ]
    }))
    .expect("tool_result with hidden reminder should parse")
    .expect("encoded tool_result should resume callback");

    assert_eq!(resume.callback_task_id, callback_task_id);
    assert_eq!(resume.tool_results[0]["tool_call_id"], json!("toolu_123"));
    assert_eq!(resume.tool_results[0]["content"], json!("Found 3 files"));
}

#[test]
fn anthropic_tool_resume_request_decodes_latest_message_only_tool_result() {
    let callback_task_id = Uuid::from_u128(0xffffffffffffffffffffffffffffffff);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_123");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": "Found 3 files"
                    }
                ]
            }
        ]
    }))
    .expect("latest-message-only tool_result should parse")
    .expect("encoded tool_result should resume callback");

    assert_eq!(resume.callback_task_id, callback_task_id);
    assert_eq!(resume.tool_results[0]["tool_call_id"], json!("toolu_123"));
    assert_eq!(resume.tool_results[0]["content"], json!("Found 3 files"));
}

#[test]
fn anthropic_tool_resume_request_uses_latest_callback_from_latest_message_only_results() {
    let previous_callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let current_callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let previous_tool_use_id =
        encode_anthropic_callback_tool_use_id(previous_callback_task_id, "toolu_previous");
    let current_tool_use_id =
        encode_anthropic_callback_tool_use_id(current_callback_task_id, "toolu_current");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": previous_tool_use_id,
                        "content": "old result replayed"
                    },
                    {
                        "type": "tool_result",
                        "tool_use_id": current_tool_use_id,
                        "content": "new result"
                    }
                ]
            }
        ]
    }))
    .expect("latest-message-only tool_result replay should parse")
    .expect("latest callback should be resumed");

    assert_eq!(resume.callback_task_id, current_callback_task_id);
    assert_eq!(resume.tool_results.as_array().unwrap().len(), 1);
    assert_eq!(
        resume.tool_results[0]["tool_call_id"],
        json!("toolu_current")
    );
    assert_eq!(resume.tool_results[0]["content"], json!("new result"));
}

#[test]
fn anthropic_tool_resume_request_rejects_orphan_trailing_tool_result() {
    let error = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "toolu_123",
                        "content": "stale result"
                    }
                ]
            }
        ]
    }))
    .expect_err("orphan tool_result should not create a run");

    match error {
        AnthropicRouteError::Compat(error) => {
            assert_eq!(error.error_type, "tool_result_only_orphan");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn anthropic_tool_resume_request_rejects_tool_result_without_callback_encoding() {
    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_read",
                    "name": "Read",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_read",
                    "content": "plain Anthropic tool result"
                }]
            }
        ]
    }));

    let error = resume.expect_err("unencoded tool_result should not create a run");
    match error {
        AnthropicRouteError::Compat(error) => {
            assert_eq!(error.error_type, "tool_result_only_orphan");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn anthropic_tool_resume_request_preserves_media_tool_result_content() {
    let callback_task_id = Uuid::from_u128(0x99999999999999999999999999999999);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_image");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
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
                        "content": [
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
            }
        ]
    }))
    .expect("tool_result should parse")
    .expect("encoded tool_result should resume callback");

    assert_eq!(resume.callback_task_id, callback_task_id);
    assert_eq!(resume.tool_results[0]["tool_call_id"], json!("toolu_image"));
    assert_eq!(resume.tool_results[0]["content"][0]["type"], json!("image"));
    assert_eq!(
        resume.tool_results[0]["content"][0]["source"]["media_type"],
        json!("image/png")
    );
}

#[test]
fn anthropic_tool_resume_request_uses_latest_trailing_tool_result_message() {
    let previous_callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let current_callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let previous_tool_use_id =
        encode_anthropic_callback_tool_use_id(previous_callback_task_id, "toolu_previous");
    let current_tool_use_id =
        encode_anthropic_callback_tool_use_id(current_callback_task_id, "toolu_current");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "first"},
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": previous_tool_use_id,
                    "name": "lookup_previous",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": previous_tool_use_id,
                    "content": "old result"
                }]
            },
            {"role": "assistant", "content": "old answer"},
            {"role": "user", "content": "next"},
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": current_tool_use_id,
                    "name": "lookup_current",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": current_tool_use_id,
                    "content": "new result"
                }]
            }
        ]
    }))
    .expect("resume request should parse")
    .expect("trailing tool_result should resume callback");

    assert_eq!(resume.callback_task_id, current_callback_task_id);
    assert_eq!(resume.tool_results.as_array().unwrap().len(), 1);
    assert_eq!(
        resume.tool_results[0]["tool_call_id"],
        json!("toolu_current")
    );
    assert_eq!(resume.tool_results[0]["content"], json!("new result"));
}

#[test]
fn anthropic_tool_resume_request_ignores_historical_tool_results_before_latest_user_text() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_previous");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "first"},
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": tool_use_id,
                    "name": "lookup_previous",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": "old result"
                }]
            },
            {"role": "assistant", "content": "old answer"},
            {"role": "user", "content": "next question"}
        ]
    }))
    .expect("historical tool_result should parse");

    assert!(resume.is_none());
}

#[test]
fn anthropic_tool_resume_request_ignores_tool_result_mixed_with_new_user_text() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let tool_use_id = encode_anthropic_callback_tool_use_id(callback_task_id, "toolu_previous");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
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
                        "content": [{
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": "aW1hZ2U="
                            }
                        }]
                    },
                    {
                        "type": "text",
                        "text": "帮我找找这个代码位置"
                    }
                ]
            }
        ]
    }))
    .expect("mixed tool_result and text should parse");

    assert!(resume.is_none());
}

#[test]
fn anthropic_tool_resume_request_uses_latest_callback_from_contiguous_tool_results() {
    let previous_callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let current_callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let previous_tool_use_id =
        encode_anthropic_callback_tool_use_id(previous_callback_task_id, "toolu_previous");
    let current_tool_use_id =
        encode_anthropic_callback_tool_use_id(current_callback_task_id, "toolu_current");

    let resume = anthropic_tool_resume_request(&json!({
        "model": "1flowbase",
        "messages": [
            {"role": "user", "content": "first"},
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": previous_tool_use_id,
                    "name": "lookup_previous",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": previous_tool_use_id,
                    "content": "old result"
                }]
            },
            {
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": current_tool_use_id,
                    "name": "lookup_current",
                    "input": {}
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": previous_tool_use_id,
                    "content": "old result replayed"
                }]
            },
            {
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": current_tool_use_id,
                    "content": "new result"
                }]
            }
        ]
    }))
    .expect("mixed trailing tool_result history should parse")
    .expect("latest callback should be resumed");

    assert_eq!(resume.callback_task_id, current_callback_task_id);
    assert_eq!(resume.tool_results.as_array().unwrap().len(), 1);
    assert_eq!(
        resume.tool_results[0]["tool_call_id"],
        json!("toolu_current")
    );
    assert_eq!(resume.tool_results[0]["content"], json!("new result"));
}
