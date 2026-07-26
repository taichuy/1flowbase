use super::*;

#[tokio::test]
async fn openai_responses_live_text_stream_wraps_deltas_in_output_items() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let mut events = Vec::new();
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    ));
    for (index, (event_type, text)) in [
        ("reasoning_delta", "thinking"),
        ("text_delta", "你好"),
        ("text_delta", "，OK"),
    ]
    .into_iter()
    .enumerate()
    {
        events.extend(mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 2,
                RuntimeEventPayload {
                    event_type: event_type.to_string(),
                    source: RuntimeEventSource::Runtime,
                    durability: RuntimeEventDurability::DurableRequired,
                    persist_required: true,
                    trace_visible: true,
                    payload: json!({
                        "type": event_type,
                        "event_type": event_type,
                        "node_id": "node-answer",
                        "text": text,
                        "presentation": {
                            "kind": "answer",
                            "answer_node_id": "node-answer",
                            "source_node_id": "node-llm",
                            "source_output_key": "text",
                            "segment_index": 0
                        }
                    }),
                },
            ),
        ));
    }
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            5,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "你好，OK" })),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    let reasoning_added = body
        .find("\"type\":\"reasoning\"")
        .unwrap_or_else(|| panic!("reasoning item should be added before its deltas: {body}"));
    let reasoning_delta = body
        .find("event: response.reasoning_text.delta")
        .unwrap_or_else(|| panic!("reasoning delta should stream: {body}"));
    let message_added = body
        .find("\"type\":\"message\"")
        .unwrap_or_else(|| panic!("message item should be added before text deltas: {body}"));
    let text_delta = body
        .find("event: response.output_text.delta")
        .unwrap_or_else(|| panic!("text delta should stream: {body}"));
    let completed = body
        .find("event: response.completed")
        .unwrap_or_else(|| panic!("stream should complete: {body}"));
    assert!(
        reasoning_added < reasoning_delta && reasoning_delta < message_added,
        "reasoning item lifecycle should precede the message item: {body}"
    );
    assert!(
        message_added < text_delta && text_delta < completed,
        "message item should open before text deltas and close before completed: {body}"
    );
    let item_done_count = body.matches("event: response.output_item.done").count();
    assert!(
        item_done_count >= 2,
        "both reasoning and message items should be closed, got {item_done_count}: {body}"
    );
    assert!(
        body.contains("\"text\":\"你好，OK\""),
        "message output_item.done should carry the accumulated text: {body}"
    );
}
