use control_plane::{
    application_public_api::{
        compat::openai::response_id_from_run_id,
        native::{NativeError, NativeRunResult, NativeRunStatus, NativeUsage},
    },
    orchestration_runtime::debug_stream_events,
    ports::{
        RuntimeEventDurability, RuntimeEventEnvelope, RuntimeEventPayload, RuntimeEventSource,
    },
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

use super::super::projector::ResponsesWebSocketProjector;

const PROVIDER_UPSTREAM_ERROR_BODY: &str =
    " {\"future_error\":{\"shape\":\"unknown\"},\"message\":\"keep complete body\"}\n ";

fn native_run(id: u128) -> NativeRunResult {
    NativeRunResult {
        id: Uuid::from_u128(id),
        application_id: Uuid::from_u128(0x22222222222222222222222222222222),
        api_key_id: Uuid::from_u128(0x33333333333333333333333333333333),
        publication_version_id: Uuid::from_u128(0x44444444444444444444444444444444),
        status: NativeRunStatus::Running,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn decoded(frames: Vec<String>) -> Vec<Value> {
    frames
        .into_iter()
        .map(|frame| serde_json::from_str(&frame).expect("WS text must be canonical JSON"))
        .collect()
}

fn answer_text(run: &NativeRunResult, sequence: i64, delta: &str) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run.id,
        sequence,
        debug_stream_events::answer_text_delta(
            "answer",
            delta.to_string(),
            sequence as usize,
            Some("llm"),
            None,
            Some("text"),
        ),
    )
}

fn answer_reasoning(run: &NativeRunResult, sequence: i64, delta: &str) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run.id,
        sequence,
        debug_stream_events::answer_reasoning_delta(
            "answer",
            delta.to_string(),
            sequence as usize,
            Some("llm"),
            None,
            Some("reasoning"),
        ),
    )
}

#[test]
fn causal_barrier_projects_text_before_any_terminal_fact_exists() {
    let run = native_run(0x10101010101010101010101010101010);
    let mut projector = ResponsesWebSocketProjector::new("published-model".to_string(), None);

    let frames = decoded(
        projector
            .project(&run, answer_text(&run, 1, "partial"))
            .expect("a live Answer Presentation fact must project immediately"),
    );

    assert!(frames
        .iter()
        .any(|frame| frame["type"] == "response.output_text.delta" && frame["delta"] == "partial"));
    assert!(!frames.iter().any(|frame| {
        matches!(
            frame["type"].as_str(),
            Some("response.completed" | "response.failed" | "response.cancelled")
        )
    }));
    assert!(!projector.has_terminal());
}

fn waiting_tool(run: &NativeRunResult, sequence: i64) -> RuntimeEventEnvelope {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    RuntimeEventEnvelope::new(
        run.id,
        sequence,
        RuntimeEventPayload {
            event_type: "waiting_callback".to_string(),
            source: RuntimeEventSource::Runtime,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: true,
            payload: json!({
                "type": "waiting_callback",
                "callback_kind": "llm_tool_calls",
                "callback_task_id": callback_task_id,
                "required_action": {
                    "payload": {
                        "tool_calls": [{
                            "id": "call_weather",
                            "name": "weather",
                            "arguments": { "city": "杭州" }
                        }]
                    }
                }
            }),
        },
    )
}

#[test]
fn preserves_repeated_spaces_unicode_empty_deltas_usage_and_strict_order() {
    let mut run = native_run(0x11111111111111111111111111111111);
    let mut projector = ResponsesWebSocketProjector::new("published-model".to_string(), None);
    let mut frames = projector
        .project(
            &run,
            RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
        )
        .expect("flow start must project");
    for (sequence, delta) in ["two  spaces", "世界🙂", "", "two  spaces"]
        .into_iter()
        .enumerate()
    {
        frames.extend(
            projector
                .project(&run, answer_text(&run, sequence as i64 + 2, delta))
                .expect("ordered text fact must project"),
        );
    }
    run.status = NativeRunStatus::Succeeded;
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(7),
        completion_tokens: Some(11),
        total_tokens: Some(18),
        ..NativeUsage::default()
    });
    frames.extend(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    6,
                    debug_stream_events::flow_finished(run.id, json!({ "answer": "done" })),
                ),
            )
            .expect("terminal must project"),
    );

    let events = decoded(frames);
    for (expected, event) in events.iter().enumerate() {
        assert_eq!(event["sequence_number"], json!(expected));
    }
    let deltas = events
        .iter()
        .filter(|event| event["type"] == "response.output_text.delta")
        .map(|event| event["delta"].as_str().expect("delta must be text"))
        .collect::<Vec<_>>();
    assert_eq!(deltas, vec!["two  spaces", "世界🙂", "", "two  spaces"]);
    let terminal = events.last().expect("one terminal event");
    assert_eq!(terminal["type"], "response.completed");
    assert_eq!(terminal["response"]["usage"]["input_tokens"], 7);
    assert_eq!(terminal["response"]["usage"]["output_tokens"], 11);
    assert_eq!(terminal["response"]["usage"]["total_tokens"], 18);
    assert_eq!(
        terminal["response"]["output"][0]["content"][0]["text"],
        "two  spaces世界🙂two  spaces"
    );
}

#[test]
fn reasoning_text_and_tool_facts_keep_order_and_durable_ids() {
    let mut run = native_run(0x12121212121212121212121212121212);
    let mut projector = ResponsesWebSocketProjector::new(
        "published-model".to_string(),
        Some("resp_previous".to_string()),
    );
    let mut frames = projector
        .project(&run, answer_reasoning(&run, 1, "think  "))
        .expect("reasoning must project");
    frames.extend(
        projector
            .project(&run, answer_text(&run, 2, "answer"))
            .expect("text must project after reasoning"),
    );
    run.status = NativeRunStatus::Waiting;
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(3),
        completion_tokens: Some(5),
        total_tokens: Some(8),
        ..NativeUsage::default()
    });
    frames.extend(
        projector
            .project(&run, waiting_tool(&run, 3))
            .expect("tool terminal must project"),
    );

    let events = decoded(frames);
    let types = events
        .iter()
        .map(|event| event["type"].as_str().expect("type must be text"))
        .collect::<Vec<_>>();
    assert_eq!(
        types,
        vec![
            "response.output_item.added",
            "response.reasoning_text.delta",
            "response.output_item.done",
            "response.output_item.added",
            "response.output_text.delta",
            "response.output_item.done",
            "response.output_item.added",
            "response.output_item.done",
            "response.completed",
        ]
    );
    for (expected, event) in events.iter().enumerate() {
        assert_eq!(event["sequence_number"], json!(expected));
    }
    let response_id = response_id_from_run_id(run.id);
    assert_eq!(events[1]["response_id"], response_id);
    assert_eq!(events[4]["response_id"], response_id);
    assert_eq!(events[6]["response_id"], response_id);
    assert_eq!(events[6]["item"]["id"], "fc_call_weather");
    assert_eq!(events[8]["response"]["id"], response_id);
    assert_eq!(
        events[8]["response"]["previous_response_id"],
        "resp_previous"
    );
    assert_eq!(events[8]["response"]["usage"]["total_tokens"], 8);
    assert_eq!(
        events[8]["response"]["output"]
            .as_array()
            .expect("completed output must be an array")
            .len(),
        3
    );
}

#[test]
fn cancellation_failure_and_post_terminal_events_are_honest_and_unique() {
    let mut cancelled = native_run(0x13131313131313131313131313131313);
    cancelled.status = NativeRunStatus::Cancelled;
    let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
    let terminal = decoded(
        projector
            .project(
                &cancelled,
                RuntimeEventEnvelope::new(
                    cancelled.id,
                    1,
                    debug_stream_events::flow_cancelled(cancelled.id),
                ),
            )
            .expect("cancel must project"),
    );
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["type"], "response.cancelled");
    assert_eq!(terminal[0]["response"]["status"], "cancelled");
    assert!(projector
        .project(
            &cancelled,
            RuntimeEventEnvelope::new(
                cancelled.id,
                2,
                debug_stream_events::flow_finished(cancelled.id, json!({})),
            ),
        )
        .expect("post-terminal event must be absorbed")
        .is_empty());

    let mut failed = native_run(0x14141414141414141414141414141414);
    failed.status = NativeRunStatus::Failed;
    failed.error = Some(NativeError {
        code: "provider_failed".to_string(),
        message: "provider unavailable".to_string(),
        details: json!({}),
    });
    let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
    let terminal = decoded(
        projector
            .project(
                &failed,
                RuntimeEventEnvelope::new(
                    failed.id,
                    1,
                    debug_stream_events::flow_failed(failed.id, json!({})),
                ),
            )
            .expect("failure must project"),
    );
    assert_eq!(terminal[0]["type"], "response.failed");
    assert_eq!(terminal[0]["error"]["code"], "provider_failed");
    assert_eq!(terminal[0]["error"]["message"], "provider unavailable");
}

#[test]
fn issue_1474_responses_websocket_error_preserves_native_message_exactly() {
    let mut failed = native_run(0x14741474147414741474147414741474);
    failed.status = NativeRunStatus::Failed;
    failed.error = Some(NativeError {
        code: "provider_upstream_error".to_string(),
        message: PROVIDER_UPSTREAM_ERROR_BODY.to_string(),
        details: json!({}),
    });
    let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
    let terminal = decoded(
        projector
            .project(
                &failed,
                RuntimeEventEnvelope::new(
                    failed.id,
                    1,
                    debug_stream_events::flow_failed(failed.id, json!({})),
                ),
            )
            .expect("failure must project"),
    );

    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["type"], "response.failed");
    assert_eq!(
        terminal[0]["error"]["message"],
        PROVIDER_UPSTREAM_ERROR_BODY
    );
}

#[test]
fn provider_native_and_non_presentation_deltas_are_never_body_truth() {
    let run = native_run(0x15151515151515151515151515151515);
    let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
    let provider_native = RuntimeEventEnvelope::new(
        run.id,
        1,
        RuntimeEventPayload {
            event_type: "provider_native_event".to_string(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::Ephemeral,
            persist_required: false,
            trace_visible: true,
            payload: json!({ "text": "native secret body" }),
        },
    );
    let provider_delta = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::text_delta("llm", run.id, "raw provider body".to_string()),
    );

    assert!(projector
        .project(&run, provider_native)
        .expect("native diagnostic must be ignored")
        .is_empty());
    assert!(projector
        .project(&run, provider_delta)
        .expect("non-presentation delta must be ignored")
        .is_empty());
    let canonical = decoded(
        projector
            .project(&run, answer_text(&run, 3, "canonical body"))
            .expect("presentation delta must project"),
    );
    assert_eq!(canonical[0]["sequence_number"], 0);
    assert_eq!(canonical[1]["delta"], "canonical body");
}

#[test]
fn typed_mcp_approval_is_visible_and_done_joins_completed_output() {
    let mut run = native_run(0x18181818181818181818181818181818);
    let node_run_id = Uuid::new_v4();
    let approval = json!({
        "id": "approval_1",
        "type": "mcp_approval_request",
        "name": "delete_record"
    });
    let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
    let mut frames = projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                1,
                debug_stream_events::provider_output_item_added(
                    "node-llm",
                    node_run_id,
                    2,
                    approval.clone(),
                ),
            ),
        )
        .expect("typed MCP approval added must project");
    let unknown_native = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::provider_native_event(
            "node-llm",
            node_run_id,
            "openai_responses".to_string(),
            json!({ "type": "response.output_item.done", "item": approval.clone() }),
        ),
    );
    assert!(projector
        .project(&run, unknown_native)
        .expect("unknown native event must remain filtered")
        .is_empty());
    frames.extend(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    3,
                    debug_stream_events::provider_output_item_done(
                        "node-llm",
                        node_run_id,
                        2,
                        approval.clone(),
                    ),
                ),
            )
            .expect("typed MCP approval done must project"),
    );
    run.status = NativeRunStatus::Succeeded;
    frames.extend(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    4,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            )
            .expect("terminal must include completed MCP output"),
    );

    let events = decoded(frames);
    assert_eq!(events[0]["type"], "response.output_item.added");
    assert_eq!(events[0]["response_id"], response_id_from_run_id(run.id));
    assert_eq!(events[0]["output_index"], 2);
    assert_eq!(events[1]["type"], "response.output_item.done");
    assert_eq!(events[1]["sequence_number"], 1);
    assert_eq!(events[2]["type"], "response.completed");
    assert_eq!(events[2]["response"]["output"], json!([approval]));
}

#[test]
fn sequential_turns_reset_sequence_and_keep_distinct_durable_response_ids() {
    let first = native_run(0x16161616161616161616161616161616);
    let second = native_run(0x17171717171717171717171717171717);
    let project_created = |run: &NativeRunResult| {
        let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
        decoded(
            projector
                .project(
                    run,
                    RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
                )
                .expect("each turn must project its created event"),
        )
    };

    let first_events = project_created(&first);
    let second_events = project_created(&second);
    assert_eq!(first_events[0]["sequence_number"], 0);
    assert_eq!(second_events[0]["sequence_number"], 0);
    assert_eq!(
        first_events[0]["response"]["id"],
        response_id_from_run_id(first.id)
    );
    assert_eq!(
        second_events[0]["response"]["id"],
        response_id_from_run_id(second.id)
    );
    assert_ne!(
        first_events[0]["response"]["id"],
        second_events[0]["response"]["id"]
    );
}
