use super::*;

#[test]
fn openai_delta_chunk_maps_reasoning_to_reasoning_content() {
    let chat_completion_id = "chatcmpl-test";
    let payload = openai_delta_chunk_payload(
        &native_run(),
        "deepseek-v4-pro",
        chat_completion_id,
        "reasoning_delta",
        "先分析用户问题".to_string(),
    )
    .expect("reasoning delta should map to an OpenAI-compatible chunk");

    assert_eq!(payload["id"], json!(chat_completion_id));
    assert_eq!(
        payload["choices"][0]["delta"]["reasoning_content"],
        json!("先分析用户问题")
    );
    assert_eq!(payload["choices"][0]["delta"].get("content"), None);
}

#[tokio::test]
async fn openai_terminal_does_not_reconstruct_structured_answer_segments() {
    let mut run = native_run();
    run.answer = Some("<think>旧思考</think>旧回答".to_string());
    run.answer_segments = Some(vec![
        AnswerProjectionSegment::reasoning("结构化思考"),
        AnswerProjectionSegment::message("结构化回答"),
    ]);
    let mut mapper =
        OpenAiChatStreamMapper::new("deepseek-v4-pro".to_string(), "chatcmpl-test".to_string());

    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    );
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("结构化思考"), "{body}");
    assert!(!body.contains("结构化回答"), "{body}");
    assert!(!body.contains("旧思考"), "{body}");
    assert!(!body.contains("旧回答"), "{body}");
}

#[test]
fn openai_waiting_callback_maps_to_tool_call_chunk() {
    let callback_task_id = Uuid::from_u128(0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa);
    let chat_completion_id = "chatcmpl-tool-test";
    let payload = openai_tool_call_chunk_payload(
        &native_run(),
        "1flowbase",
        chat_completion_id,
        &json!({
            "callback_kind": "llm_tool_calls",
            "callback_task_id": callback_task_id,
            "tool_calls": [
                {
                    "id": "call_weather",
                    "name": "lookup_weather",
                    "arguments": {"city": "Hangzhou"}
                }
            ]
        }),
    )
    .expect("LLM callback should map to OpenAI tool call chunk");

    assert_eq!(payload["id"], json!(chat_completion_id));
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["name"],
        json!("lookup_weather")
    );
    assert_eq!(
        payload["choices"][0]["delta"]["tool_calls"][0]["function"]["arguments"],
        json!("{\"city\":\"Hangzhou\"}")
    );
    let call_id = payload["choices"][0]["delta"]["tool_calls"][0]["id"]
        .as_str()
        .expect("tool call id should be encoded");
    assert!(call_id.contains("call_weather"));
}

#[test]
fn openai_chat_completion_id_changes_for_callback_resume() {
    let run_id = Uuid::from_u128(0x11111111111111111111111111111111);
    let callback_task_id = Uuid::from_u128(0x22222222222222222222222222222222);

    assert_ne!(
        openai_chat_completion_id_from_run_id(run_id),
        openai_chat_completion_id_from_callback_task(run_id, callback_task_id)
    );
}

#[test]
fn openai_responses_waiting_callback_maps_to_function_call_item() {
    let callback_task_id = Uuid::from_u128(0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);
    let output = openai_response_function_call_output_items(&json!({
        "callback_kind": "llm_tool_calls",
        "callback_task_id": callback_task_id,
        "tool_calls": [
            {
                "id": "call_inventory",
                "name": "lookup_inventory",
                "arguments": {"sku": "sku_123"}
            }
        ]
    }))
    .expect("LLM callback should map to Responses function_call output");

    assert_eq!(output[0]["type"], json!("function_call"));
    assert_eq!(output[0]["name"], json!("lookup_inventory"));
    assert_eq!(output[0]["arguments"], json!("{\"sku\":\"sku_123\"}"));
    assert_eq!(output[0]["call_id"], "call_inventory");
}

#[test]
fn openai_finish_chunk_uses_deepseek_compatible_terminal_shape() {
    let payload = openai_finish_chunk_payload(&native_run(), "1flowbase", "chatcmpl-test", "stop");

    assert_eq!(payload["choices"][0]["delta"]["content"], json!(""));
    assert_eq!(payload["choices"][0]["delta"]["role"], Value::Null);
    assert_eq!(payload["choices"][0]["finish_reason"], json!("stop"));
    assert_eq!(payload["usage"]["prompt_tokens"], json!(0));
    assert_eq!(payload["usage"]["completion_tokens"], json!(0));
    assert_eq!(payload["usage"]["total_tokens"], json!(0));
}

#[test]
fn openai_chat_terminal_emits_only_finish_without_live_content() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn openai_chat_terminal_does_not_reconstruct_thinking_or_text() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());

    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("先分析"), "{body}");
    assert!(!body.contains("最终回答"), "{body}");
    assert!(!body.contains("<think>"), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn openai_responses_terminal_does_not_reconstruct_thinking_or_text() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "<think>先分析</think>\n最终回答" }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("response.reasoning_text.delta"), "{body}");
    assert!(!body.contains("response.output_text.delta"), "{body}");
    assert!(!body.contains("先分析"), "{body}");
    assert!(!body.contains("最终回答"), "{body}");
    assert!(!body.contains("<think>"), "{body}");
    assert!(body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn openai_response_completed_event_includes_usage() {
    let mut run = native_run();
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(11),
        completion_tokens: Some(7),
        total_tokens: Some(18),
        ..Default::default()
    });
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "Final answer" })),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("event: response.completed"), "{body}");
    assert!(body.contains("\"usage\""), "{body}");
    assert!(body.contains("\"input_tokens\":11"), "{body}");
    assert!(body.contains("\"output_tokens\":7"), "{body}");
    assert!(body.contains("\"total_tokens\":18"), "{body}");
}

#[tokio::test]
async fn openai_chat_terminal_does_not_decode_artifact_preview_answer() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(
                run.id,
                json!({
                    "answer": {
                        "__runtime_debug_artifact": true,
                        "artifact_ref": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                        "is_truncated": false,
                        "preview": "\"最终回答\""
                    }
                }),
            ),
        ),
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(!body.contains("最终回答"), "{body}");
    assert!(body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(body.contains("[DONE]"), "{body}");
}

#[test]
fn openai_chat_terminal_ignores_provider_raw_delta_without_reconstruction() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let text_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta("node-llm", run.id, "已流式输出".to_string()),
        ),
    );
    let terminal_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
        ),
    );

    assert!(text_events.is_empty());
    assert_eq!(terminal_events.len(), 2);
}

#[test]
fn compatible_stream_stats_count_only_emitted_answer_deltas() {
    let run = native_run();
    let mut stats = CompatibleStreamStats::default();
    let answer_delta = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::answer_text_delta(
            "node-answer",
            "已输出".to_string(),
            0,
            Some("node-llm"),
            None,
            Some("text"),
        ),
    );
    stats.record_sent_runtime_event(&run, &answer_delta, true);

    let terminal_event = RuntimeEventEnvelope::new(
        run.id,
        2,
        debug_stream_events::flow_finished(run.id, json!({ "answer": "最终回答" })),
    );
    stats.record_sent_runtime_event(&run, &terminal_event, true);

    assert!(stats.emitted_content());
    assert_eq!(stats.emitted_content_bytes, "已输出".len());
}

#[test]
fn openai_chat_projects_answer_presentation_delta_not_provider_raw_delta() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let provider_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::text_delta(
                "node-llm",
                Uuid::from_u128(0x55555555555555555555555555555555),
                "provider raw".to_string(),
            ),
        ),
    );
    let presentation_events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            RuntimeEventPayload {
                event_type: "text_delta".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: false,
                payload: json!({
                    "type": "text_delta",
                    "node_run_id": Uuid::from_u128(0x66666666666666666666666666666666),
                    "node_id": "node-answer",
                    "text": "answer presentation",
                    "presentation": { "kind": "answer" }
                }),
            },
        ),
    );

    assert!(provider_events.is_empty());
    assert_eq!(presentation_events.len(), 1);
}

#[test]
fn callback_diagnostic_tail_and_synthetic_start_do_not_establish_a_round() {
    let run_id = Uuid::now_v7();
    let round_id = Uuid::now_v7();
    let mut replay = vec![RuntimeEventEnvelope::new(
        run_id,
        0,
        debug_stream_events::flow_started(run_id),
    )];
    for sequence in [41, 45] {
        replay.push(RuntimeEventEnvelope::new(
            run_id,
            sequence,
            RuntimeEventPayload {
                event_type: "client_protocol_trajectory".into(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::DurableRequired,
                persist_required: true,
                trace_visible: true,
                payload: json!({"type": "client_protocol_trajectory"}),
            },
        ));
    }
    assert!(
        !callback_round_replay_established(&replay, round_id),
        "nonempty diagnostic replay must not suppress the real callback marker"
    );
    replay[0].payload["response_round_id"] = json!(Uuid::now_v7());
    assert!(
        !callback_round_replay_established(&replay, round_id),
        "a foreign round marker must not establish the requested callback"
    );
    replay[0].payload["response_round_id"] = json!(round_id);
    assert!(callback_round_replay_established(&replay, round_id));
}

#[test]
fn legacy_completed_callback_replay_remains_established_without_a_tagged_marker() {
    let run_id = Uuid::now_v7();
    let replay = vec![RuntimeEventEnvelope::new(
        run_id,
        7,
        debug_stream_events::flow_finished(run_id, json!({})),
    )];
    assert!(
        callback_round_replay_established(&replay, Uuid::now_v7()),
        "generation-filtered completed legacy rounds must stay replayable"
    );
}
