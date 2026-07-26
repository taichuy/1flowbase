use super::*;

#[tokio::test]
async fn openai_responses_waiting_callback_projects_client_function_call() {
    let run = native_run();
    let callback_task_id = Uuid::from_u128(0xcccccccccccccccccccccccccccccccc);
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
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
                    "tool_calls": [
                        {
                            "id": "call_inventory",
                            "name": "lookup_inventory",
                            "arguments": {"sku": "sku_123"}
                        }
                    ]
                }),
            },
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    let added_index = body
        .find("event: response.output_item.added")
        .unwrap_or_else(|| panic!("Responses function_call must emit output_item.added: {body}"));
    let done_index = body
        .find("event: response.output_item.done")
        .unwrap_or_else(|| panic!("Responses function_call must emit output_item.done: {body}"));
    let completed_index = body
        .find("event: response.completed")
        .unwrap_or_else(|| panic!("Responses function_call must complete the response: {body}"));
    assert!(
        added_index < done_index && done_index < completed_index,
        "{body}"
    );
    assert!(body.contains("\"type\":\"function_call\""), "{body}");
    assert!(body.contains("\"name\":\"lookup_inventory\""), "{body}");
    assert!(
        body.contains("\"arguments\":\"{\\\"sku\\\":\\\"sku_123\\\"}\""),
        "{body}"
    );
    assert!(!body.contains("required_action_not_supported"), "{body}");
}

#[tokio::test]
async fn openai_responses_waiting_internal_llm_tool_callback_is_explicitly_unsupported() {
    let mut run = native_run();
    let callback_task_id = Uuid::from_u128(0x34343434343434343434343434343434);
    run.status = NativeRunStatus::Waiting;
    run.answer = Some("visible internal LLM output".to_string());
    run.tool_calls = Some(json!([
        {
            "id": "call_internal",
            "visibility": "internal",
            "origin": "visible_internal_llm_tool",
            "name": "inspect_visible_context",
            "arguments": { "query": "visible" }
        }
    ]));

    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
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

    assert!(body.contains("event: response.failed"), "{body}");
    assert!(body.contains("required_action_not_supported"), "{body}");
    assert!(!body.contains("\"type\":\"function_call\""), "{body}");
    assert!(!body.contains("event: response.completed"), "{body}");
}
