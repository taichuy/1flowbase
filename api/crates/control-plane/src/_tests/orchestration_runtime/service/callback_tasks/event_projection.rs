use super::*;

#[test]
fn waiting_callback_event_references_large_request_without_copying_it() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let callback_task = domain::CallbackTaskRecord {
        id: Uuid::now_v7(),
        flow_run_id,
        node_run_id,
        callback_kind: "llm_tool_calls".to_string(),
        status: domain::CallbackTaskStatus::Pending,
        request_payload: json!({
            "history": "x".repeat(2 * 1024 * 1024),
            "tool_calls": [{
                "id": "call_weather",
                "name": "lookup_weather",
                "arguments": { "city": "Shanghai" }
            }]
        }),
        response_payload: None,
        external_ref_payload: None,
        created_at: time::OffsetDateTime::now_utc(),
        completed_at: None,
    };

    let event = crate::orchestration_runtime::debug_stream_events::waiting_callback_with_task(
        flow_run_id,
        node_run_id,
        "node-llm",
        &callback_task,
    );

    assert!(event.payload.get("request_payload").is_none());
    assert!(event.payload["required_action"]["payload"]
        .get("request_payload")
        .is_none());
    assert_eq!(
        event.payload["required_action"]["payload"]["callback_task_id"],
        json!(callback_task.id)
    );
    assert_eq!(
        event.payload["required_action"]["payload"]["tool_calls"][0]["id"],
        json!("call_weather")
    );
    assert!(
        serde_json::to_vec(&event.payload).unwrap().len()
            < crate::ports::EPHEMERAL_PAYLOAD_MAX_BYTES
    );
}

#[test]
fn assistant_tool_call_events_keep_the_live_trace_separate_from_callback_control() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let tool_call = json!({
        "id": "call_weather",
        "name": "lookup_weather",
        "arguments": { "city": "Shanghai" }
    });
    let call_usage = json!({
        "input_tokens": 1200,
        "output_tokens": 260,
        "total_tokens": 1460
    });

    let started = crate::orchestration_runtime::debug_stream_events::assistant_tool_call_started(
        flow_run_id,
        node_run_id,
        "node-llm",
        tool_call.clone(),
        call_usage.clone(),
    );
    let finished = crate::orchestration_runtime::debug_stream_events::assistant_tool_call_finished(
        flow_run_id,
        node_run_id,
        "node-llm",
        tool_call,
        call_usage.clone(),
        json!({
            "tool_call_id": "call_weather",
            "name": "lookup_weather",
            "content": { "temperature": 26 },
            "is_error": false
        }),
        42,
    );

    assert_eq!(started.event_type, "assistant_tool_call_started");
    assert_eq!(started.payload["tool_call"]["id"], "call_weather");
    assert_eq!(started.payload["call_usage"], call_usage);
    assert_eq!(started.payload["node_run_id"], json!(node_run_id));
    assert_eq!(finished.event_type, "assistant_tool_call_finished");
    assert_eq!(
        finished.payload["tool_result"]["content"]["temperature"],
        26
    );
    assert_eq!(finished.payload["duration_ms"], 42);
    assert_eq!(finished.payload["call_usage"]["total_tokens"], 1460);
    assert!(started.persist_required);
    assert!(finished.trace_visible);
}
