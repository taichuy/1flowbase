use super::*;

#[test]
fn anthropic_waiting_callback_maps_to_tool_use_block() {
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
async fn anthropic_waiting_internal_llm_tool_callback_streams_text_without_tool_use() {
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

    let response = completed_compatible_stream(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("visible internal LLM output"), "{body}");
    assert!(body.contains("\"type\":\"text_delta\""), "{body}");
    assert!(body.contains("\"stop_reason\":\"end_turn\""), "{body}");
    assert!(!body.contains("\"type\":\"tool_use\""), "{body}");
    assert!(!body.contains("required_action_not_supported"), "{body}");
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

    let response = completed_compatible_stream(events);
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
async fn anthropic_waiting_callback_streams_tool_input_json_delta() {
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
    let response = completed_compatible_stream(events);
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
