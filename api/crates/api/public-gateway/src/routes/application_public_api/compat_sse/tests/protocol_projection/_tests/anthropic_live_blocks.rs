use super::*;

fn progress(run: &NativeRunResult, kind: &str, payload: Value) -> RuntimeEventEnvelope {
    RuntimeEventEnvelope::new(
        run.id,
        1,
        RuntimeEventPayload {
            event_type: kind.to_owned(),
            source: RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::Ephemeral,
            persist_required: false,
            trace_visible: false,
            payload,
        },
    )
}

fn block(index: u64, item: Value) -> Value {
    json!({"node_id":"llm","node_run_id":Uuid::from_u128(10),
        "response_round_id":Uuid::from_u128(20), "output_index":index,"item":item})
}

fn tool_start(callback: Uuid) -> Value {
    let mut payload = block(
        1,
        json!({"type":"function_call","id":"item-1","call_id":"call-1","name":"Read","arguments":""}),
    );
    payload["callback_task_id"] = json!(callback);
    payload
}

fn tool_delta(callback: Uuid, delta: &str) -> Value {
    json!({"node_id":"llm","node_run_id":Uuid::from_u128(10),
        "response_round_id":Uuid::from_u128(20), "output_index":1,
        "callback_task_id":callback,"item_id":"item-1","call_id":"call-1","delta":delta})
}

fn committed(callback: Uuid, arguments: Value) -> Value {
    json!({"callback_kind":"llm_tool_calls","callback_task_id":callback,
        "tool_calls":[{"id":"call-1","name":"Read","arguments":arguments}]})
}

async fn decoded(events: Vec<Result<Event, Infallible>>) -> Vec<DecodedAnthropicEvent> {
    let bytes = axum::body::to_bytes(
        test_projected_events_response(events).into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    decode_anthropic_sse(std::str::from_utf8(&bytes).unwrap())
}

#[tokio::test]
async fn anthropic_live_blocks_precede_callback_commit_barrier() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    let (early_tx, early_rx) = tokio::sync::oneshot::channel();
    let (commit_tx, commit_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut mapper = AnthropicStreamMapper::new("test".into());
        let reasoning = block(0, json!({"id":"reasoning-1","type":"reasoning"}));
        let mut events = mapper.runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", reasoning.clone()),
        );
        events.extend(mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                2,
                debug_stream_events::answer_reasoning_delta(
                    "answer",
                    "thinking".into(),
                    0,
                    Some("llm"),
                    Some(Uuid::from_u128(10)),
                    Some("reasoning"),
                ),
            ),
        ));
        events.extend(mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                3,
                debug_stream_events::reasoning_signature_delta(
                    "llm",
                    Uuid::from_u128(10),
                    "sig".into(),
                ),
            ),
        ));
        events.extend(mapper.runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_done", reasoning),
        ));
        events.extend(mapper.runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", tool_start(callback)),
        ));
        events.extend(mapper.runtime_event_to_sse(
            &run,
            progress(
                &run,
                "provider_tool_call_delta",
                tool_delta(callback, "{\"path\":"),
            ),
        ));
        early_tx.send(decoded(events).await).unwrap();
        commit_rx.await.unwrap();
        decoded(
            mapper
                .anthropic_tool_use_events(&committed(callback, json!({"path":"a"})), None)
                .unwrap(),
        )
        .await
    });
    let early = early_rx.await.unwrap();
    let signature = early
        .iter()
        .position(|e| e.data["delta"]["type"] == "signature_delta")
        .unwrap();
    let stop = early
        .iter()
        .position(|e| e.name == "content_block_stop")
        .unwrap();
    assert!(
        signature < stop,
        "signature must precede the explicit thinking stop"
    );
    assert_eq!(early[stop].data["index"], 0);
    let start = early
        .iter()
        .find(|e| e.data["content_block"]["type"] == "tool_use")
        .unwrap();
    assert_eq!(
        start.data["content_block"]["id"],
        encode_anthropic_callback_tool_use_id(callback, "call-1")
    );
    assert!(early
        .iter()
        .any(|e| e.data["delta"]["partial_json"] == "{\"path\":"));
    assert!(
        !early.iter().any(|e| e.name == "message_stop"
            || (e.name == "content_block_stop" && e.data["index"] == 1)),
        "a tool must not become executable before durable callback admission"
    );
    commit_tx.send(()).unwrap();
    let terminal = task.await.unwrap();
    let suffix = terminal
        .iter()
        .find(|e| e.data["delta"]["type"] == "input_json_delta")
        .unwrap();
    assert_eq!(suffix.data["delta"]["partial_json"], "\"a\"}");
    assert_eq!(
        terminal
            .iter()
            .filter(|e| e.name == "content_block_start")
            .count(),
        0
    );
    assert_eq!(
        terminal
            .iter()
            .filter(|e| e.name == "content_block_stop")
            .count(),
        1
    );
    assert_eq!(terminal.last().unwrap().name, "message_stop");
}

#[tokio::test]
async fn anthropic_live_blocks_preserve_argument_bytes_and_reject_foreign_scope() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    let mut mapper = AnthropicStreamMapper::new("test".into());
    let mut missing_reservation = tool_start(callback);
    missing_reservation
        .as_object_mut()
        .unwrap()
        .remove("callback_task_id");
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", missing_reservation)
        )
        .is_empty());
    assert_eq!(
        decoded(mapper.runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", tool_start(callback))
        ))
        .await
        .len(),
        1
    );
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", tool_start(callback))
        )
        .is_empty());
    let mut foreign = tool_delta(callback, "corrupt");
    foreign["node_run_id"] = json!(Uuid::from_u128(11));
    assert!(mapper
        .runtime_event_to_sse(&run, progress(&run, "provider_tool_call_delta", foreign))
        .is_empty());
    let args = "{ \"z\" : \"\\u4e2d\", \"a\": 1 }";
    let fragments = decoded(mapper.runtime_event_to_sse(
        &run,
        progress(&run, "provider_tool_call_delta", tool_delta(callback, args)),
    ))
    .await;
    assert_eq!(fragments[0].data["delta"]["partial_json"], args);
    let terminal = decoded(
        mapper
            .anthropic_tool_use_events(&committed(callback, json!({"a":1,"z":"中"})), None)
            .unwrap(),
    )
    .await;
    assert!(!terminal
        .iter()
        .any(|e| e.data["delta"]["type"] == "input_json_delta"));
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            progress(
                &run,
                "provider_tool_call_delta",
                tool_delta(callback, "late")
            )
        )
        .is_empty());
    assert!(mapper
        .anthropic_tool_use_events(&committed(callback, json!({"a":1,"z":"中"})), None)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn anthropic_live_blocks_fail_closed_on_different_callback_arguments() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    let mut mapper = AnthropicStreamMapper::new("test".into());
    mapper.runtime_event_to_sse(
        &run,
        progress(&run, "provider_content_block_start", tool_start(callback)),
    );
    mapper.runtime_event_to_sse(
        &run,
        progress(
            &run,
            "provider_tool_call_delta",
            tool_delta(callback, "{\"path\":\"old\"}"),
        ),
    );
    let result = decoded(
        mapper
            .anthropic_tool_use_events(&committed(callback, json!({"path":"new"})), None)
            .unwrap(),
    )
    .await;
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "error");
    assert!(!result
        .iter()
        .any(|e| e.name == "message_stop" || e.name == "content_block_stop"));
}

#[tokio::test]
async fn anthropic_live_blocks_done_cannot_close_a_different_content_block() {
    let run = native_run();
    let mut mapper = AnthropicStreamMapper::new("test".into());
    let initial = block(0, json!({"type":"reasoning","id":"reasoning-1"}));
    mapper.runtime_event_to_sse(
        &run,
        progress(&run, "provider_content_block_start", initial.clone()),
    );
    let mut foreign = initial.clone();
    foreign["response_round_id"] = json!(Uuid::from_u128(21));
    assert!(mapper
        .runtime_event_to_sse(&run, progress(&run, "provider_content_block_done", foreign))
        .is_empty());
    let closed = decoded(mapper.runtime_event_to_sse(
        &run,
        progress(&run, "provider_content_block_done", initial.clone()),
    ))
    .await;
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].name, "content_block_stop");
    assert!(mapper
        .runtime_event_to_sse(&run, progress(&run, "provider_content_block_done", initial))
        .is_empty());
}

fn raw_done(callback: Uuid, arguments: &str, admitted: bool) -> Value {
    json!({"node_id":"llm","node_run_id":Uuid::from_u128(10),"output_index":1,
        "reserved_callback_task_id":callback,"committed_delivery":admitted,
        "item":{"type":"function_call","id":"item-1","call_id":"call-1",
            "name":"Read","arguments":arguments}})
}

#[tokio::test]
async fn anthropic_live_blocks_complete_noncanonical_prefix_from_committed_raw_done() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    for (prefix, complete, arguments) in [
        ("{ \"path\" : ", "{ \"path\" : \"a\" }", json!({"path":"a"})),
        (
            "{\n\"z\":",
            "{\n\"z\":\"\\u4e2d\",\"a\":1}",
            json!({"a":1,"z":"中"}),
        ),
    ] {
        let mut mapper = AnthropicStreamMapper::new("test".into());
        mapper.runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", tool_start(callback)),
        );
        mapper.runtime_event_to_sse(
            &run,
            progress(
                &run,
                "provider_tool_call_delta",
                tool_delta(callback, prefix),
            ),
        );
        assert!(
            mapper
                .runtime_event_to_sse(
                    &run,
                    progress(
                        &run,
                        "provider_output_item_done",
                        raw_done(callback, complete, true)
                    )
                )
                .is_empty(),
            "retain authoritative bytes without closing a tool before the callback projection"
        );
        let events = decoded(
            mapper
                .anthropic_tool_use_events(&committed(callback, arguments), None)
                .unwrap(),
        )
        .await;
        assert!(!events.iter().any(|e| e.name == "error"));
        let suffix = events
            .iter()
            .find(|e| e.data["delta"]["type"] == "input_json_delta")
            .unwrap()
            .data["delta"]["partial_json"]
            .as_str()
            .unwrap();
        assert_eq!(format!("{prefix}{suffix}"), complete);
        assert_eq!(
            events
                .iter()
                .filter(|e| e.name == "content_block_stop")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn anthropic_live_blocks_do_not_use_uncommitted_or_foreign_argument_evidence() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    for mut evidence in [
        raw_done(callback, "{ \"path\" : \"a\" }", false),
        raw_done(callback, "{ \"path\" : \"a\" }", true),
    ] {
        if evidence["committed_delivery"] == true {
            evidence["node_run_id"] = json!(Uuid::from_u128(11));
        }
        let mut mapper = AnthropicStreamMapper::new("test".into());
        mapper.runtime_event_to_sse(
            &run,
            progress(&run, "provider_content_block_start", tool_start(callback)),
        );
        mapper.runtime_event_to_sse(
            &run,
            progress(
                &run,
                "provider_tool_call_delta",
                tool_delta(callback, "{ \"path\" : "),
            ),
        );
        mapper.runtime_event_to_sse(&run, progress(&run, "provider_output_item_done", evidence));
        let events = decoded(
            mapper
                .anthropic_tool_use_events(&committed(callback, json!({"path":"a"})), None)
                .unwrap(),
        )
        .await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "error");
        assert!(events[0].data["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unavailable"));
    }
}

#[tokio::test]
async fn anthropic_live_blocks_validate_raw_done_against_committed_callback() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    let mut mapper = AnthropicStreamMapper::new("test".into());
    mapper.runtime_event_to_sse(
        &run,
        progress(&run, "provider_content_block_start", tool_start(callback)),
    );
    mapper.runtime_event_to_sse(
        &run,
        progress(
            &run,
            "provider_tool_call_delta",
            tool_delta(callback, "{\"path\":\"a\"}"),
        ),
    );
    mapper.runtime_event_to_sse(
        &run,
        progress(
            &run,
            "provider_output_item_done",
            raw_done(callback, "{\"path\":\"changed\"}", true),
        ),
    );
    let events = decoded(
        mapper
            .anthropic_tool_use_events(&committed(callback, json!({"path":"a"})), None)
            .unwrap(),
    )
    .await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].name, "error");
}

#[tokio::test]
async fn anthropic_live_blocks_keep_empty_supplier_arguments_as_an_empty_input_object() {
    let run = native_run();
    let callback = Uuid::from_u128(30);
    let mut mapper = AnthropicStreamMapper::new("test".into());
    mapper.runtime_event_to_sse(
        &run,
        progress(&run, "provider_content_block_start", tool_start(callback)),
    );
    mapper.runtime_event_to_sse(
        &run,
        progress(
            &run,
            "provider_output_item_done",
            raw_done(callback, "", true),
        ),
    );
    let events = decoded(
        mapper
            .anthropic_tool_use_events(&committed(callback, json!({})), None)
            .unwrap(),
    )
    .await;
    assert!(!events.iter().any(|e| e.name == "error"));
    assert_eq!(
        events
            .iter()
            .filter(|e| e.name == "content_block_stop")
            .count(),
        1
    );
}
