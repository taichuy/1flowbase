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
use crate::routes::application_public_api::compat_sse::ResponsesProjectionMode;

#[tokio::test]
async fn transparent_websocket_preserves_all_live_native_fragments_before_terminal() {
    use crate::host_infrastructure::LocalRuntimeEventStream;
    use control_plane::ports::{RuntimeEventStream, RuntimeEventStreamPolicy};
    let fragments = [
        "HAND", "OFF", "_", "69", "faf", "c", "3", "c", "-", "3", "ac", "0", "-", "4", "ebb", "-",
        "968", "1", "-", "82", "ef", "452", "b", "358", "1",
    ];
    let expected = "HANDOFF_69fafc3c-3ac0-4ebb-9681-82ef452b3581";
    let mut run = native_run(0x20990000000000000000000000000010);
    let stream = LocalRuntimeEventStream::with_broadcast_capacity_for_tests(1);
    stream
        .open_run(run.id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    let mut subscription = stream.subscribe(run.id, Some(0)).await.unwrap();
    let node = Uuid::now_v7();
    for fragment in fragments {
        stream
            .append(
                run.id,
                debug_stream_events::provider_responses_output_delta(
                    "llm",
                    node,
                    json!({"type":"response.output_text.delta","delta":fragment}),
                ),
            )
            .await
            .unwrap();
        stream
            .append(
                run.id,
                debug_stream_events::text_delta("llm", node, fragment.into()),
            )
            .await
            .unwrap();
    }
    stream
        .append(
            run.id,
            debug_stream_events::provider_responses_output_delta(
                "llm",
                node,
                json!({"type":"response.output_text.done","text":expected}),
            ),
        )
        .await
        .unwrap();
    stream
        .append_terminal_if_missing_and_close(
            run.id,
            debug_stream_events::flow_finished(run.id, json!({})),
        )
        .await
        .unwrap();
    let mut projector = transparent_projector("fixture", None);
    let mut frames = Vec::new();
    let mut seen = 0_i64;
    while let Some(envelope) = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        subscription.live_events.recv(),
    )
    .await
    .unwrap()
    {
        seen += 1;
        assert_eq!(
            envelope.sequence, seen,
            "the common forwarding cursor requires ordered live events"
        );
        if envelope.event_type == "flow_finished" {
            run.status = NativeRunStatus::Succeeded;
        }
        frames.extend(projector.project(&run, envelope).unwrap());
        tokio::task::yield_now().await;
    }
    let frames = decoded(frames);
    let deltas = frames
        .iter()
        .filter(|event| event["type"] == "response.output_text.delta")
        .map(|event| event["delta"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        deltas, fragments,
        "done/completed must not stand in for missing live fragments"
    );
    assert_eq!(deltas.concat(), expected);
    assert_eq!(frames.last().unwrap()["type"], "response.completed");
    assert_eq!(
        frames
            .iter()
            .filter(|event| event["type"] == "response.completed")
            .count(),
        1
    );
    assert_eq!(seen, 52);
    for (index, frame) in frames.iter().enumerate() {
        assert_eq!(frame["sequence_number"], index as u64);
    }
}

fn committed_provider_output_item_done(
    node_id: &str,
    node_run_id: Uuid,
    output_index: usize,
    item: Value,
) -> RuntimeEventPayload {
    let mut event =
        debug_stream_events::provider_output_item_done(node_id, node_run_id, output_index, item);
    event.payload["committed_delivery"] = json!(true);
    event
}

#[test]
fn websocket_tool_done_projects_only_after_committed_delivery() {
    let run = native_run(2079);
    let node = Uuid::now_v7();
    let item = json!({
        "type": "custom_tool_call",
        "call_id": "call_committed",
        "name": "exec",
        "input": "pwd"
    });
    let mut projector = transparent_projector("fixture", None);
    let uncommitted = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::provider_output_item_done("llm", node, 0, item.clone()),
    );
    assert!(projector.project(&run, uncommitted).unwrap().is_empty());

    let committed = RuntimeEventEnvelope::new(
        run.id,
        2,
        committed_provider_output_item_done("llm", node, 0, item),
    );
    assert_eq!(projector.project(&run, committed).unwrap().len(), 1);
}

fn transparent_projector(
    model: impl Into<String>,
    previous: Option<String>,
) -> ResponsesWebSocketProjector {
    ResponsesWebSocketProjector::with_mode(
        model.into(),
        previous,
        ResponsesProjectionMode::TransparentProviderResponses,
    )
}

#[test]
fn frozen_projection_modes_never_mix_sources_and_preserve_utf8_text_identity() {
    let fragments = ["中", "🙂", "\n```rust\n", "let x = 1;", "\n```"];

    let mut semantic_run = native_run(0x20390000000000000000000000000001);
    let mut semantic = ResponsesWebSocketProjector::new("model".into(), None);
    let provider_node = Uuid::now_v7();
    assert!(semantic
        .project(
            &semantic_run,
            RuntimeEventEnvelope::new(
                semantic_run.id,
                1,
                debug_stream_events::provider_responses_output_delta(
                    "llm",
                    provider_node,
                    json!({"type":"response.output_text.delta","delta":"wrong-source"}),
                ),
            ),
        )
        .unwrap()
        .is_empty());
    let mut semantic_frames = Vec::new();
    for (index, fragment) in fragments.iter().enumerate() {
        semantic_frames.extend(
            semantic
                .project(
                    &semantic_run,
                    answer_text(&semantic_run, index as i64 + 2, fragment),
                )
                .unwrap(),
        );
    }
    semantic_run.status = NativeRunStatus::Succeeded;
    semantic_frames.extend(
        semantic
            .project(
                &semantic_run,
                RuntimeEventEnvelope::new(
                    semantic_run.id,
                    20,
                    debug_stream_events::flow_finished(semantic_run.id, json!({})),
                ),
            )
            .unwrap(),
    );
    assert_text_identity(decoded(semantic_frames), &fragments);

    let mut transparent_run = native_run(0x20390000000000000000000000000002);
    let mut transparent = transparent_projector("model", None);
    assert!(transparent
        .project(
            &transparent_run,
            answer_text(&transparent_run, 1, "wrong-source")
        )
        .unwrap()
        .is_empty());
    let message = json!({
        "id": "msg_provider",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "output_text", "text": fragments.concat()}]
    });
    let mut transparent_frames = transparent
        .project(
            &transparent_run,
            RuntimeEventEnvelope::new(
                transparent_run.id,
                2,
                debug_stream_events::provider_output_item_added(
                    "llm",
                    provider_node,
                    0,
                    message.clone(),
                ),
            ),
        )
        .unwrap();
    for (index, fragment) in fragments.iter().enumerate() {
        transparent_frames.extend(
            transparent
                .project(
                    &transparent_run,
                    RuntimeEventEnvelope::new(
                        transparent_run.id,
                        index as i64 + 3,
                        debug_stream_events::provider_responses_output_delta(
                            "llm",
                            provider_node,
                            json!({"type":"response.output_text.delta","delta":fragment}),
                        ),
                    ),
                )
                .unwrap(),
        );
    }
    transparent_frames.extend(
        transparent
            .project(
                &transparent_run,
                RuntimeEventEnvelope::new(
                    transparent_run.id,
                    10,
                    committed_provider_output_item_done("llm", provider_node, 0, message),
                ),
            )
            .unwrap(),
    );
    transparent_run.status = NativeRunStatus::Succeeded;
    transparent_frames.extend(
        transparent
            .project(
                &transparent_run,
                RuntimeEventEnvelope::new(
                    transparent_run.id,
                    11,
                    debug_stream_events::flow_finished(transparent_run.id, json!({})),
                ),
            )
            .unwrap(),
    );
    assert_text_identity(decoded(transparent_frames), &fragments);
}

fn assert_text_identity(events: Vec<Value>, fragments: &[&str]) {
    for (expected, event) in events.iter().enumerate() {
        assert_eq!(event["sequence_number"], expected as u64);
    }
    let delta = events
        .iter()
        .filter(|event| event["type"] == "response.output_text.delta")
        .filter_map(|event| event["delta"].as_str())
        .collect::<String>();
    let done = events
        .iter()
        .find(|event| event["type"] == "response.output_item.done")
        .unwrap();
    let completed = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .unwrap();
    let done_text = done["item"]["content"][0]["text"].as_str().unwrap();
    let completed_text = completed["response"]["output"][0]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert_eq!(delta, fragments.concat());
    assert_eq!(delta, done_text);
    assert_eq!(delta, completed_text);
}

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

#[test]
fn progress_heartbeat_is_protocol_event_without_changing_response_output() {
    let mut run = native_run(0x11111111111111111111111111111119);
    let mut projector = ResponsesWebSocketProjector::new("published-model".into(), None);
    assert!(projector.progress_heartbeat(&run).unwrap().is_none());
    let created = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
            )
            .unwrap(),
    );
    let progress = decoded(vec![projector.progress_heartbeat(&run).unwrap().unwrap()]);
    assert_eq!(created[0]["type"], "response.created");
    assert_eq!(progress[0]["type"], "response.in_progress");
    assert_eq!(progress[0]["sequence_number"], 1);
    assert_eq!(progress[0]["response"]["id"], created[0]["response"]["id"]);
    assert_eq!(progress[0]["response"]["output"], json!([]));
    run.status = NativeRunStatus::Succeeded;
    let completed = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    2,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            )
            .unwrap(),
    );
    assert_eq!(completed.last().unwrap()["sequence_number"], 2);
    assert!(projector.progress_heartbeat(&run).unwrap().is_none());
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
fn responses_usage_details_websocket_preserves_text_and_strict_order() {
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
        cache_read_tokens: Some(5),
        reasoning_tokens: Some(3),
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
        terminal["response"]["usage"]["input_tokens_details"]["cached_tokens"],
        5
    );
    assert_eq!(
        terminal["response"]["usage"]["output_tokens_details"]["reasoning_tokens"],
        3
    );
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
    assert_eq!(events[6]["item"]["call_id"], "call_weather");
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
    assert_eq!(terminal[0]["response"]["error"]["code"], "provider_failed");
    assert_eq!(
        terminal[0]["response"]["error"]["message"],
        "provider unavailable"
    );
    assert!(terminal[0].get("error").is_none());
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
        terminal[0]["response"]["error"]["message"],
        PROVIDER_UPSTREAM_ERROR_BODY
    );
    assert!(terminal[0].get("error").is_none());
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
fn recovery_lifecycle_metadata_never_commits_projection_and_terminal_is_once() {
    let mut run = native_run(0x2072d400000000000000000000000001);
    let mut projector = ResponsesWebSocketProjector::new("model".to_string(), None);
    let lifecycle = RuntimeEventEnvelope::new(
        run.id,
        1,
        RuntimeEventPayload {
            event_type: "provider_native_event".to_string(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::Ephemeral,
            persist_required: false,
            trace_visible: true,
            payload: json!({
                "transport_epoch": 17,
                "socket_incarnation": 4,
                "commit_level": "lifecycle_only",
                "disposition": "same_epoch_reconnect",
                "raw_cursor": "must-not-project",
                "turn_state": "must-not-project"
            }),
        },
    );
    assert!(projector
        .project(&run, lifecycle)
        .expect("typed Provider lifecycle fact must remain diagnostic")
        .is_empty());
    assert!(!projector.has_terminal());

    let semantic = decoded(
        projector
            .project(&run, answer_text(&run, 2, "visible once"))
            .expect("AI Native semantic fact must project"),
    );
    assert!(semantic
        .iter()
        .any(|event| event["type"] == "response.output_text.delta"));
    let encoded = serde_json::to_string(&semantic).unwrap();
    for forbidden in ["raw_cursor", "turn_state", "must-not-project"] {
        assert!(!encoded.contains(forbidden));
    }

    run.status = NativeRunStatus::Succeeded;
    let terminal = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    3,
                    debug_stream_events::flow_finished(run.id, json!({})),
                ),
            )
            .expect("first terminal fact must project"),
    );
    assert_eq!(
        terminal
            .iter()
            .filter(|event| event["type"] == "response.completed")
            .count(),
        1
    );
    assert!(projector.has_terminal());
    assert!(projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                4,
                debug_stream_events::flow_finished(run.id, json!({})),
            ),
        )
        .expect("duplicate terminal fact must be absorbed")
        .is_empty());
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
    let mut projector = transparent_projector("model", None);
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
                    committed_provider_output_item_done(
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

// #2028 AC-007: public projection consumes canonical tool items, never diagnostic events.
#[test]
fn issue_2028_native_tool_projection_keeps_wire_shape_once() {
    for item in [
        json!({"type":"custom_tool_call","id":"ct_1","call_id":"call_1","name":"exec","input":"text(await tools.exec_command({cmd: 'cat fixture'}));","status":"completed"}),
        json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":"read","arguments":"{\"path\":\"fixture\"}","status":"completed"}),
    ] {
        let run = native_run(2028);
        let mut projector = transparent_projector("model", None);
        let node = Uuid::new_v4();
        let facts = [
            debug_stream_events::provider_output_item_added("llm", node, 0, item.clone()),
            debug_stream_events::provider_native_event(
                "llm",
                node,
                "openai_responses".into(),
                json!({"type":"response.output_item.done","item":item}),
            ),
            committed_provider_output_item_done("llm", node, 0, item.clone()),
            debug_stream_events::flow_finished(run.id, json!({})),
        ];
        let mut frames = Vec::new();
        for (index, fact) in facts.into_iter().enumerate() {
            frames.extend(decoded(
                projector
                    .project(&run, RuntimeEventEnvelope::new(run.id, index as i64, fact))
                    .unwrap(),
            ));
        }
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0]["item"], item);
        assert_eq!(frames[1]["item"], item);
        assert_eq!(frames[2]["response"]["output"], json!([item]));
        assert_eq!(frames[2]["type"], "response.completed");
    }
}

// AC-012: formal NDJSON survives projection; semantic answer mirrors cannot
// create another message, nor can diagnostic frames satisfy the oracle.
#[test]
fn native_formal_output_preserves_phase_reasoning_and_order_without_text_duplicates() {
    let run = native_run(2030);
    let node = Uuid::new_v4();
    let mut projector = transparent_projector("fixture", None);
    let items = [
        json!({"id":"rs_1","type":"reasoning","summary":[],"encrypted_content":"opaque"}),
        json!({"id":"msg_1","type":"message","role":"assistant","phase":"commentary","content":[{"type":"output_text","text":"reading"}]}),
        json!({"id":"ct_1","type":"custom_tool_call","name":"exec","call_id":"call_1","input":"text(1)"}),
    ];
    let mut facts = vec![];
    for (index, item) in items.iter().enumerate() {
        for phase in ["added", "done"] {
            let line: extension_contracts::provider_contract::ProviderRuntimeLine =
                serde_json::from_value(
                    json!({"type":"output_item","phase":phase,"output_index":index,"item":item}),
                )
                .unwrap();
            match line.into_stream_event().unwrap() {
                extension_contracts::provider_contract::ProviderStreamEvent::OutputItem {
                    phase,
                    output_index,
                    item,
                } => {
                    facts.push(match phase {
                        extension_contracts::provider_contract::ProviderOutputItemPhase::Added => {
                            debug_stream_events::provider_output_item_added(
                                "llm",
                                node,
                                output_index,
                                item,
                            )
                        }
                        extension_contracts::provider_contract::ProviderOutputItemPhase::Done => {
                            committed_provider_output_item_done("llm", node, output_index, item)
                        }
                    });
                }
                _ => unreachable!(),
            }
        }
    }
    facts.push(debug_stream_events::answer_text_delta(
        "answer",
        "reading".into(),
        0,
        Some("llm"),
        Some(node),
        Some("text"),
    ));
    facts.push(debug_stream_events::flow_finished(run.id, json!({})));
    let mut frames = vec![];
    for (index, fact) in facts.into_iter().enumerate() {
        frames.extend(decoded(
            projector
                .project(&run, RuntimeEventEnvelope::new(run.id, index as i64, fact))
                .unwrap(),
        ));
    }
    let done: Vec<_> = frames
        .iter()
        .filter(|v| v["type"] == "response.output_item.done")
        .map(|v| v["item"].clone())
        .collect();
    assert_eq!(done, items);
    assert_eq!(frames.last().unwrap()["response"]["output"], json!(items));
    assert_eq!(frames.len(), 7);
}

// AC-003/AC-005: a resumed native turn keeps its own response identity and call ID.
#[test]
fn native_resumed_websocket_round_preserves_identity_and_tool_items() {
    let mut run = native_run(2036);
    let round = Uuid::from_u128(2037);
    run.metadata = json!({"response_round_id": round});
    let expected = response_id_from_run_id(round);
    let mut projector = transparent_projector("model", Some(response_id_from_run_id(run.id)));
    let item = json!({"id":"ct_native", "type":"custom_tool_call", "call_id":"call_original", "name":"exec", "input":"echo 2036"});
    let mut frames = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
            )
            .unwrap(),
    );
    for (sequence, event) in [
        debug_stream_events::provider_output_item_added(
            "llm",
            Uuid::from_u128(42),
            0,
            item.clone(),
        ),
        committed_provider_output_item_done("llm", Uuid::from_u128(42), 0, item.clone()),
    ]
    .into_iter()
    .enumerate()
    {
        frames.extend(decoded(
            projector
                .project(
                    &run,
                    RuntimeEventEnvelope::new(run.id, sequence as i64 + 2, event),
                )
                .unwrap(),
        ));
    }
    frames.extend(decoded(
        projector.project(&run, waiting_tool(&run, 4)).unwrap(),
    ));
    assert_eq!(
        frames.len(),
        4,
        "native tool items must not be synthesized again at wait"
    );
    for frame in &frames {
        assert_eq!(
            frame.get("response_id").unwrap_or(&frame["response"]["id"]),
            &json!(expected)
        );
    }
    let terminal = frames.last().unwrap();
    assert_eq!(terminal["type"], "response.completed");
    assert_eq!(terminal["response"]["output"], json!([item]));
    assert_eq!(
        terminal["response"]["previous_response_id"],
        response_id_from_run_id(run.id)
    );
}

#[test]
fn failed_terminal_preserves_committed_output_and_absorbs_late_success() {
    let mut run = native_run(2085);
    let node = Uuid::now_v7();
    let item = json!({"id":"fc_committed","type":"function_call","call_id":"call_committed","name":"exec","arguments":"{}"});
    let mut projector = transparent_projector("fixture", None);
    let frames = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    1,
                    committed_provider_output_item_done("llm", node, 0, item.clone()),
                ),
            )
            .unwrap(),
    );
    assert_eq!(frames.len(), 1);
    run.status = NativeRunStatus::Failed;
    run.error = Some(NativeError {
        code: "provider_upstream_error".into(),
        message: "original provider failure".into(),
        details: json!({}),
    });
    let terminal = decoded(
        projector
            .project(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    2,
                    debug_stream_events::flow_failed(run.id, json!({})),
                ),
            )
            .unwrap(),
    );
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0]["type"], "response.failed");
    assert_eq!(
        terminal[0]["response"]["error"]["message"],
        "original provider failure"
    );
    assert_eq!(terminal[0]["response"]["output"], json!([item]));
    assert!(projector
        .project(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                3,
                debug_stream_events::flow_finished(run.id, json!({})),
            )
        )
        .unwrap()
        .is_empty());
}
