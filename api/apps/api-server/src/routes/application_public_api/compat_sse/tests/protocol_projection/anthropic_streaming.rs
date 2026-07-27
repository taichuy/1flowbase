use super::*;

#[derive(Debug)]
struct DecodedAnthropicEvent {
    name: String,
    data: Value,
}

fn decode_anthropic_sse(body: &str) -> Vec<DecodedAnthropicEvent> {
    body.split("\n\n")
        .filter_map(|wire_event| {
            let mut name = None;
            let mut data_lines = Vec::new();
            for line in wire_event.lines() {
                let line = line.strip_suffix('\r').unwrap_or(line);
                if let Some(value) = line.strip_prefix("event: ") {
                    name = Some(value.to_string());
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data_lines.push(value);
                }
            }
            let name = name?;
            let data = serde_json::from_str(&data_lines.join("\n"))
                .expect("Anthropic SSE data should decode as JSON");
            Some(DecodedAnthropicEvent { name, data })
        })
        .collect()
}

#[tokio::test]
async fn issue_1474_anthropic_sse_error_preserves_native_message_exactly() {
    let mut run = native_run();
    run.status = NativeRunStatus::Failed;
    run.error = Some(NativeError {
        code: "provider_upstream_error".to_string(),
        message: PROVIDER_UPSTREAM_ERROR_BODY.to_string(),
        details: json!({}),
    });
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_failed(run.id, json!({})),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Anthropic error SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("Anthropic error SSE should be UTF-8");
    let decoded = decode_anthropic_sse(&body);

    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].name, "error");
    assert_eq!(
        decoded[0].data["error"]["message"],
        PROVIDER_UPSTREAM_ERROR_BODY
    );
}

#[test]
fn anthropic_tool_use_wire_helper_encodes_callback_block() {
    let callback_task_id = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
    let blocks = anthropic_tool_use_blocks_from_waiting_payload(&json!({
        "callback_kind": "llm_tool_calls",
        "callback_task_id": callback_task_id,
        "tool_calls": [
            {
                "id": "toolu_weather",
                "name": "lookup_weather",
                "arguments": {"city": "Hangzhou"}
            }
        ]
    }))
    .expect("LLM callback should map to Anthropic tool_use blocks");

    assert_eq!(blocks[0]["type"], json!("tool_use"));
    assert_eq!(blocks[0]["name"], json!("lookup_weather"));
    assert_eq!(blocks[0]["input"]["city"], json!("Hangzhou"));
    assert!(blocks[0]["id"]
        .as_str()
        .expect("tool_use id should be encoded")
        .contains("toolu_weather"));
}

#[tokio::test]
async fn anthropic_waiting_internal_llm_tool_callback_is_explicitly_unsupported() {
    let mut run = native_run();
    let callback_task_id = Uuid::from_u128(0x56565656565656565656565656565656);
    run.status = NativeRunStatus::Waiting;
    run.answer = Some("visible internal LLM output".to_string());
    run.tool_calls = Some(json!([
        {
            "id": "toolu_internal",
            "metadata": {
                "visibility": "internal",
                "origin": "visible_internal_llm_tool"
            },
            "name": "inspect_visible_context",
            "arguments": { "query": "visible" }
        }
    ]));

    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    );
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            RuntimeEventPayload {
                event_type: "waiting_callback".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({
                    "type": "waiting_callback",
                    "run_id": run.id,
                    "status": "waiting_callback",
                    "callback_task_id": callback_task_id,
                    "callback_kind": "llm_tool_calls",
                    "tool_calls": run.tool_calls.clone().unwrap()
                }),
            },
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: error"), "{body}");
    assert!(body.contains("required_action_not_supported"), "{body}");
    assert!(!body.contains("visible internal LLM output"), "{body}");
    assert!(!body.contains("\"type\":\"tool_use\""), "{body}");
    assert!(!body.contains("event: message_stop"), "{body}");
}

#[tokio::test]
async fn anthropic_text_stream_follows_claude_messages_event_order() {
    let run = native_run();
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    );
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "hello ClaudeCode".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            3,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "hello ClaudeCode" })),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    let message_start = body
        .find("event: message_start")
        .unwrap_or_else(|| panic!("Anthropic stream should start with message_start: {body}"));
    let block_start = body
        .find("event: content_block_start")
        .unwrap_or_else(|| panic!("Anthropic stream should open a content block: {body}"));
    let text_delta = body
        .find("\"type\":\"text_delta\"")
        .unwrap_or_else(|| panic!("Anthropic stream should emit text_delta: {body}"));
    let block_stop = body
        .find("event: content_block_stop")
        .unwrap_or_else(|| panic!("Anthropic stream should close the content block: {body}"));
    let message_delta = body
        .find("event: message_delta")
        .unwrap_or_else(|| panic!("Anthropic stream should emit message_delta: {body}"));
    let message_stop = body
        .find("event: message_stop")
        .unwrap_or_else(|| panic!("Anthropic stream should stop with message_stop: {body}"));
    assert!(
        message_start < block_start
            && block_start < text_delta
            && text_delta < block_stop
            && block_stop < message_delta
            && message_delta < message_stop,
        "Anthropic event order should match Claude Messages streaming: {body}"
    );
    assert!(body.contains("hello ClaudeCode"), "{body}");
    assert!(body.contains("\"stop_reason\":\"end_turn\""), "{body}");
}

#[tokio::test]
async fn d1_ac_001_002_anthropic_decoder_preserves_canonical_text_segments_exactly() {
    let run = native_run();
    let expected_segments = vec![
        "A",
        " ",
        " ",
        "\n",
        "\n",
        "`",
        "`",
        "---",
        "---",
        "中文🙂",
        "\r\n",
        "",
        "Z",
    ];
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    );
    for (index, segment) in expected_segments.iter().enumerate() {
        events.extend(mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 2,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    (*segment).to_string(),
                    index,
                    Some("node-llm"),
                    None,
                    Some("text"),
                ),
            ),
        ));
    }
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            expected_segments.len() as i64 + 2,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "terminal payload must not reconstruct text" }),
            ),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let decoded = decode_anthropic_sse(&body);
    let decoded_segments = decoded
        .iter()
        .filter(|event| event.data["delta"]["type"] == json!("text_delta"))
        .map(|event| {
            event.data["delta"]["text"]
                .as_str()
                .expect("text_delta should contain text")
        })
        .collect::<Vec<_>>();

    assert_eq!(decoded_segments, expected_segments);
    assert_eq!(decoded_segments.concat(), expected_segments.concat());
    assert!(!body.contains("terminal payload must not reconstruct text"));
}

#[tokio::test]
async fn d1_ac_003_anthropic_terminal_is_absorbing_and_protocol_order_is_legal() {
    let mut run = native_run();
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(7),
        completion_tokens: Some(11),
        total_tokens: Some(18),
        ..NativeUsage::default()
    });
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    );
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "same".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            3,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "same".to_string(),
                1,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            4,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            5,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            6,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "late".to_string(),
                2,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let decoded = decode_anthropic_sse(&body);
    let names = decoded
        .iter()
        .map(|event| event.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "message_start",
            "content_block_start",
            "content_block_delta",
            "content_block_delta",
            "content_block_stop",
            "message_delta",
            "message_stop",
        ]
    );
    assert!(decoded
        .iter()
        .all(|event| event.data["type"].as_str() == Some(event.name.as_str())));
    assert_eq!(
        decoded[0].data["message"]["id"],
        json!(format!("msg_{}", run.id))
    );
    assert_eq!(decoded[0].data["message"]["usage"]["input_tokens"], 7);
    assert_eq!(decoded[1].data["index"], 0);
    assert_eq!(decoded[2].data["index"], 0);
    assert_eq!(decoded[3].data["index"], 0);
    assert_eq!(decoded[4].data["index"], 0);
    assert_eq!(decoded[5].data["delta"]["stop_reason"], "end_turn");
    assert_eq!(decoded[5].data["usage"]["output_tokens"], 11);
    assert_eq!(
        decoded
            .iter()
            .filter(|event| event.name == "message_stop")
            .count(),
        1
    );
    assert_eq!(
        decoded
            .iter()
            .filter(|event| event.data["delta"]["type"] == json!("text_delta"))
            .map(|event| event.data["delta"]["text"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["same", "same"]
    );
    assert!(!body.contains("late"));
}

#[tokio::test]
async fn d2_ac_004_anthropic_incomplete_uses_canonical_terminal_not_provider_finish() {
    let mut run = native_run();
    run.status = NativeRunStatus::Incomplete;
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    );
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            RuntimeEventPayload {
                event_type: "finish".to_string(),
                source: RuntimeEventSource::Provider,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({
                    "type": "finish",
                    "reason": "stop"
                }),
            },
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            3,
            debug_stream_events::flow_incomplete(run.id, json!({ "answer": "" })),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"stop_reason\":\"max_tokens\""), "{body}");
    assert!(!body.contains("\"stop_reason\":\"end_turn\""), "{body}");
}

#[tokio::test]
async fn anthropic_tool_use_wire_helper_serializes_input_json() {
    let callback_task_id = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
    let mut mapper = AnthropicStreamMapper::new("1flowbase".to_string());
    let events = mapper
        .anthropic_tool_use_events(
            &json!({
                "callback_kind": "llm_tool_calls",
                "callback_task_id": callback_task_id,
                "tool_calls": [
                    {
                        "id": "toolu_bash",
                        "name": "Bash",
                        "arguments": {
                            "command": "pwd && ls -la",
                            "description": "List files"
                        }
                    }
                ]
            }),
            None,
        )
        .expect("LLM callback should map to Anthropic tool_use stream events");
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("\"input\":{}"), "{body}");
    assert!(body.contains("\"type\":\"input_json_delta\""), "{body}");
    assert!(
        body.contains("\\\"command\\\":\\\"pwd && ls -la\\\""),
        "{body}"
    );
}
