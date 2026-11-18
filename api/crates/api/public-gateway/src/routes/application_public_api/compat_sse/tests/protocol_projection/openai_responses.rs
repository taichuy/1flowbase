use super::*;

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

#[derive(Default)]
struct DecodedResponsesStream {
    sequence_numbers: Vec<u64>,
    text_deltas: Vec<String>,
    completed_count: usize,
    error_messages: Vec<String>,
    root_error_count: usize,
    output_item_events: Vec<serde_json::Value>,
    completed_output: Vec<serde_json::Value>,
}

fn decode_responses_sse(body: &str) -> DecodedResponsesStream {
    let mut decoded = DecodedResponsesStream::default();
    let mut event_name = None;
    for line in body.lines() {
        if let Some(name) = line.strip_prefix("event: ") {
            event_name = Some(name);
            continue;
        }
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        let payload: serde_json::Value =
            serde_json::from_str(data).expect("Responses SSE data must be valid JSON");
        decoded.sequence_numbers.push(
            payload["sequence_number"]
                .as_u64()
                .expect("every Responses SSE event must have a sequence number"),
        );
        match event_name.take() {
            Some("response.output_text.delta") => decoded
                .text_deltas
                .push(payload["delta"].as_str().unwrap_or_default().to_string()),
            Some("response.output_item.added" | "response.output_item.done") => {
                decoded.output_item_events.push(payload)
            }
            Some("response.completed") => {
                decoded.completed_count += 1;
                decoded.completed_output = payload["response"]["output"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
            }
            Some("response.failed") => {
                if payload.get("error").is_some() {
                    decoded.root_error_count += 1;
                }
                decoded.error_messages.push(
                    payload["response"]["error"]["message"]
                        .as_str()
                        .expect("Responses failure should contain response.error.message")
                        .to_string(),
                );
            }
            _ => {}
        }
    }
    decoded
}

#[tokio::test]
async fn responses_usage_details_terminal_sse_projects_cached_and_reasoning_tokens() {
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    run.usage = Some(NativeUsage {
        prompt_tokens: Some(33024),
        completion_tokens: Some(120),
        total_tokens: Some(33144),
        cache_read_tokens: Some(3328),
        reasoning_tokens: Some(80),
        ..NativeUsage::default()
    });
    let mut mapper = OpenAiResponseStreamMapper::new("provider/model".into(), None);
    let events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "done" })),
        ),
    );
    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Responses SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("Responses SSE should be UTF-8");
    let terminal = body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).expect("valid SSE JSON"))
        .find(|event| event["type"] == "response.completed")
        .expect("terminal event");
    assert_eq!(
        terminal["response"]["usage"],
        json!({
            "input_tokens": 33024,
            "input_tokens_details": { "cached_tokens": 3328 },
            "output_tokens": 120,
            "output_tokens_details": { "reasoning_tokens": 80 },
            "total_tokens": 33144
        })
    );
}

#[test]
fn responses_tool_done_projects_only_after_committed_delivery() {
    let run = native_run();
    let node = Uuid::new_v4();
    let item = json!({
        "type": "function_call",
        "call_id": "call_committed",
        "name": "exec",
        "arguments": "{}"
    });
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let uncommitted = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::provider_output_item_done("llm", node, 0, item.clone()),
    );
    assert!(mapper.runtime_event_to_sse(&run, uncommitted).is_empty());

    let committed = RuntimeEventEnvelope::new(
        run.id,
        2,
        committed_provider_output_item_done("llm", node, 0, item),
    );
    let events = mapper.runtime_event_to_sse(&run, committed);
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn issue_1474_responses_sse_error_preserves_native_message_exactly() {
    let mut run = native_run();
    run.status = NativeRunStatus::Failed;
    run.error = Some(NativeError {
        code: "provider_upstream_error".to_string(),
        message: PROVIDER_UPSTREAM_ERROR_BODY.to_string(),
        details: json!({}),
    });
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
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
        .expect("Responses error SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("Responses error SSE should be UTF-8");
    let decoded = decode_responses_sse(&body);

    assert_eq!(
        decoded.error_messages,
        vec![PROVIDER_UPSTREAM_ERROR_BODY.to_string()]
    );
    assert_eq!(decoded.root_error_count, 0);
}

#[tokio::test]
async fn ac_001_ac_002_responses_decoder_preserves_ordered_text_deltas_exactly() {
    let mut run = native_run();
    run.answer = Some("terminal answer must not be reconstructed".to_string());
    let fixture = [
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
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(run.id, 1, debug_stream_events::flow_started(run.id)),
    );

    for (index, text) in fixture.iter().enumerate() {
        let projected = mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 2,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    (*text).to_string(),
                    index,
                    Some("node-llm"),
                    None,
                    Some("text"),
                ),
            ),
        );
        assert!(
            !projected.is_empty(),
            "every canonical delta projects immediately"
        );
        events.extend(projected);
    }
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            fixture.len() as i64 + 2,
            debug_stream_events::flow_finished(
                run.id,
                json!({ "answer": "terminal answer must not be reconstructed" }),
            ),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Responses SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("Responses SSE must be UTF-8");
    let decoded = decode_responses_sse(&body);

    assert_eq!(decoded.text_deltas, fixture.map(str::to_string));
    assert_eq!(
        decoded.sequence_numbers,
        (0..decoded.sequence_numbers.len() as u64).collect::<Vec<_>>()
    );
    assert_eq!(decoded.text_deltas.concat(), fixture.concat());
    assert_eq!(decoded.completed_count, 1);
    let done = decoded
        .output_item_events
        .iter()
        .find(|event| event["type"] == "response.output_item.done")
        .unwrap();
    assert_eq!(done["item"]["content"][0]["text"], fixture.concat());
    assert_eq!(
        decoded.completed_output[0]["content"][0]["text"],
        fixture.concat()
    );
    assert!(!body.contains("terminal answer must not be reconstructed"));
}

#[tokio::test]
async fn transparent_sse_uses_only_provider_responses_and_preserves_utf8_text_identity() {
    let mut run = native_run();
    let node = Uuid::now_v7();
    let fragments = ["中文", "🙂", "\n**bold**", "\n```\ncode\n```"];
    let message = json!({
        "id": "msg_provider",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "output_text", "text": fragments.concat()}]
    });
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                1,
                debug_stream_events::answer_text_delta(
                    "answer",
                    "wrong-source".into(),
                    0,
                    Some("llm"),
                    Some(node),
                    Some("text"),
                ),
            )
        )
        .is_empty());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::provider_output_item_added("llm", node, 0, message.clone()),
        ),
    );
    for (index, fragment) in fragments.iter().enumerate() {
        events.extend(mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                index as i64 + 3,
                debug_stream_events::provider_responses_output_delta(
                    "llm",
                    node,
                    json!({"type":"response.output_text.delta","delta":fragment}),
                ),
            ),
        ));
    }
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            10,
            committed_provider_output_item_done("llm", node, 0, message),
        ),
    ));
    run.status = NativeRunStatus::Succeeded;
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            11,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    ));

    let body = axum::body::to_bytes(
        test_projected_events_response(events).into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    let decoded = decode_responses_sse(std::str::from_utf8(&body).unwrap());
    assert_eq!(
        decoded.sequence_numbers,
        (0..decoded.sequence_numbers.len() as u64).collect::<Vec<_>>()
    );
    let delta = decoded.text_deltas.concat();
    let done = decoded
        .output_item_events
        .iter()
        .find(|event| event["type"] == "response.output_item.done")
        .unwrap();
    assert_eq!(delta, fragments.concat());
    assert_eq!(delta, done["item"]["content"][0]["text"]);
    assert_eq!(delta, decoded.completed_output[0]["content"][0]["text"]);
}

#[test]
fn ac_003_responses_terminal_is_absorbing() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let terminal = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    );
    assert_eq!(terminal.len(), 1);
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                2,
                debug_stream_events::answer_text_delta(
                    "node-answer",
                    "late".to_string(),
                    0,
                    Some("node-llm"),
                    None,
                    Some("text"),
                ),
            ),
        )
        .is_empty());
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                3,
                debug_stream_events::flow_finished(run.id, json!({})),
            ),
        )
        .is_empty());
}

#[test]
fn ac_006_provider_native_events_are_not_a_public_response_truth() {
    let run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
    let native = RuntimeEventEnvelope::new(
        run.id,
        1,
        debug_stream_events::provider_native_event(
            "node-llm",
            Uuid::new_v4(),
            "openai_responses".to_string(),
            json!({
                "type": "response.output_text.delta",
                "delta": "must-not-escape"
            }),
        ),
    );

    assert!(mapper.runtime_event_to_sse(&run, native).is_empty());
}

#[tokio::test]
async fn responses_projects_typed_mcp_approval_and_keeps_unknown_native_hidden() {
    let run = native_run();
    let node_run_id = Uuid::new_v4();
    let approval = json!({
        "id": "approval_1",
        "type": "mcp_approval_request",
        "name": "delete_record"
    });
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "1flowbase".to_string(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            7,
            debug_stream_events::provider_output_item_added(
                "node-llm",
                node_run_id,
                2,
                approval.clone(),
            ),
        ),
    );
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                8,
                debug_stream_events::provider_native_event(
                    "node-llm",
                    node_run_id,
                    "openai_responses".to_string(),
                    json!({ "type": "response.output_item.done", "item": approval.clone() }),
                ),
            ),
        )
        .is_empty());
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            9,
            committed_provider_output_item_done("node-llm", node_run_id, 2, approval.clone()),
        ),
    ));
    events.extend(mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            10,
            debug_stream_events::flow_finished(run.id, json!({})),
        ),
    ));

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Responses SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("Responses SSE must be UTF-8");
    let decoded = decode_responses_sse(&body);
    assert_eq!(decoded.output_item_events.len(), 2);
    assert_eq!(
        decoded.output_item_events[0]["type"],
        "response.output_item.added"
    );
    assert_eq!(decoded.output_item_events[0]["sequence_number"], 0);
    assert_eq!(
        decoded.output_item_events[1]["type"],
        "response.output_item.done"
    );
    assert_eq!(decoded.output_item_events[1]["output_index"], 2);
    assert_eq!(decoded.output_item_events[1]["item"], approval);
    assert_eq!(decoded.completed_output, vec![approval]);
}

// Root #2028 AC-012: the SSE entry uses the same formal source as WebSocket.
#[tokio::test]
async fn native_sse_keeps_phase_and_opaque_items_without_answer_mirrors() {
    let run = native_run();
    let node = Uuid::new_v4();
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let items = [
        json!({"id":"rs_1","type":"reasoning","summary":[],"encrypted_content":"opaque"}),
        json!({"id":"msg_1","type":"message","role":"assistant","phase":"final_answer","content":[{"type":"output_text","text":"nonce"}]}),
    ];
    let mut facts = vec![];
    for (index, item) in items.iter().enumerate() {
        facts.push(debug_stream_events::provider_output_item_added(
            "llm",
            node,
            index,
            item.clone(),
        ));
        facts.push(committed_provider_output_item_done(
            "llm",
            node,
            index,
            item.clone(),
        ));
    }
    facts.push(debug_stream_events::answer_text_delta(
        "answer",
        "nonce".into(),
        0,
        Some("llm"),
        Some(node),
        Some("text"),
    ));
    facts.push(debug_stream_events::flow_finished(run.id, json!({})));
    let mut events = vec![];
    for (index, fact) in facts.into_iter().enumerate() {
        events.extend(
            mapper
                .runtime_event_to_sse(&run, RuntimeEventEnvelope::new(run.id, index as i64, fact)),
        );
    }
    let response = test_projected_events_response(events);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let decoded = decode_responses_sse(std::str::from_utf8(&bytes).unwrap());
    assert_eq!(decoded.completed_output, items);
    assert_eq!(decoded.output_item_events.len(), 4);
    assert_eq!(decoded.completed_count, 1);
    assert!(decoded.text_deltas.is_empty());
}

#[tokio::test]
async fn failed_terminal_preserves_committed_output_and_absorbs_late_success() {
    let mut run = native_run();
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::TransparentProviderResponses,
    );
    let item = json!({"id":"fc_committed","type":"function_call","call_id":"call_committed","name":"exec","arguments":"{}"});
    let committed = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            committed_provider_output_item_done("llm", Uuid::now_v7(), 0, item.clone()),
        ),
    );
    assert_eq!(committed.len(), 1);
    run.status = NativeRunStatus::Failed;
    run.error = Some(NativeError {
        code: "provider_upstream_error".into(),
        message: "original provider failure".into(),
        details: json!({}),
    });
    let terminal = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_failed(run.id, json!({})),
        ),
    );
    assert!(mapper
        .runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                3,
                debug_stream_events::flow_finished(run.id, json!({})),
            )
        )
        .is_empty());
    let response = test_projected_events_response(terminal);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let frames = body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0]["type"], "response.failed");
    assert_eq!(
        frames[0]["response"]["error"]["message"],
        "original provider failure"
    );
    assert_eq!(frames[0]["response"]["output"], json!([item]));
}

#[tokio::test]
async fn semantic_responses_done_completed_and_blocking_preserve_presented_prefix() {
    use control_plane::application_public_api::compat::openai::projection::{
        function_call_items, output_item, OutputKind,
    };
    let mut run = native_run();
    let round = Uuid::now_v7();
    run.metadata["response_round_id"] = json!(round);
    let calls = vec![json!({"id":"call_visible","name":"read","arguments":{"path":"src"}})];
    let mut expected = vec![
        output_item(run.id, OutputKind::Reasoning, Some("plan".into())),
        output_item(run.id, OutputKind::Message, Some("checking".into())),
    ];
    expected.extend(function_call_items(&calls));
    let mut mapper = OpenAiResponseStreamMapper::new("fixture".into(), Some("resp_parent".into()));
    let mut events = Vec::new();
    for (index, event) in [
        debug_stream_events::answer_reasoning_delta("answer", "plan".into(), 0, None, None, None),
        debug_stream_events::answer_text_delta("answer", "checking".into(), 0, None, None, None),
        RuntimeEventPayload { event_type:"waiting_callback".into(), source:RuntimeEventSource::Runtime,
            durability:RuntimeEventDurability::DurableRequired, persist_required:true, trace_visible:true,
            payload:json!({"callback_task_id":Uuid::now_v7(),"callback_kind":"llm_tool_calls","tool_calls":calls}) },
    ].into_iter().enumerate() {
        events.extend(mapper.runtime_event_to_sse(&run, RuntimeEventEnvelope::new(run.id, index as i64 + 1, event)));
    }
    let body = axum::body::to_bytes(
        test_projected_events_response(events).into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    let decoded = decode_responses_sse(&body);
    let done = decoded
        .output_item_events
        .iter()
        .filter(|item| item["type"] == "response.output_item.done")
        .collect::<Vec<_>>();
    assert_eq!(
        done.iter()
            .map(|item| item["output_index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        done.iter()
            .map(|item| item["item"].clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(decoded.completed_output, expected);
    run.status = NativeRunStatus::Waiting;
    run.metadata["responses_round"] = json!({"output":expected});
    let blocking = crate::routes::application_public_api::openai::to_openai_responses_response(
        run,
        "fixture".into(),
        Some("resp_parent".into()),
    )
    .unwrap();
    let blocking = serde_json::to_value(blocking).unwrap();
    assert_eq!(blocking["output"], json!(decoded.completed_output));
    assert_eq!(blocking["id"], format!("resp_{round}"));
    assert_eq!(blocking["previous_response_id"], "resp_parent");
}

#[tokio::test]
async fn responses_sse_preserves_full_upstream_error_object() {
    for upstream in [
        json!({"message":"rejected\nnext line","type":"vendor_refusal","code":"unknown_vendor_code","param":null,"extra":{"opaque":[null,1]}}),
        json!({"message":null,"type":null,"code":null,"future":true}),
        json!({"extra":[]}),
        json!({"message":"","code":""}),
    ] {
        let mut run = native_run();
        run.status = NativeRunStatus::Failed;
        run.error = Some(NativeError {
            code: "unknown_vendor_code".into(),
            message: "fallback".into(),
            details: json!({"upstream_error":upstream,"semantic_terminal":true}),
        });
        let mut mapper = OpenAiResponseStreamMapper::new("fixture".into(), None);
        let events = mapper.runtime_event_to_sse(
            &run,
            RuntimeEventEnvelope::new(
                run.id,
                1,
                debug_stream_events::flow_failed(run.id, json!({})),
            ),
        );
        let body = axum::body::to_bytes(
            test_projected_events_response(events).into_body(),
            usize::MAX,
        )
        .await
        .unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        let frames = body
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .map(|data| serde_json::from_str::<Value>(data).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["response"]["error"], upstream);
    }
}

#[tokio::test]
async fn preferred_responses_sse_uses_ordered_host_selection_for_live_and_cold_output() {
    use control_plane::ports::{LlmInputRepresentation, LlmInputSelection};
    for native in [false, true] {
        for cold in [false, true] {
            let mut run = native_run();
            run.status = NativeRunStatus::Succeeded;
            let node = Uuid::now_v7();
            let item = json!({"id":"msg_selected","type":"message","role":"assistant","content":[{"type":"output_text","text":"selected marker"}]});
            let mut mapper = OpenAiResponseStreamMapper::with_mode(
                "fixture".into(),
                None,
                ResponsesProjectionMode::AwaitingHostSelection,
            );
            let mut facts = vec![
                debug_stream_events::flow_started(run.id),
                LlmInputSelection {
                    node_id: "llm".into(),
                    node_run_id: node,
                    representation: if native {
                        LlmInputRepresentation::NativeOpaque
                    } else {
                        LlmInputRepresentation::SemanticMapped
                    },
                }
                .runtime_event(),
            ];
            if native {
                facts.push(debug_stream_events::provider_responses_output_delta(
                    "llm",
                    node,
                    json!({"type":"response.output_text.delta","delta":"selected marker"}),
                ));
                facts.push(debug_stream_events::provider_output_item_added(
                    "llm",
                    node,
                    0,
                    item.clone(),
                ));
                facts.push(debug_stream_events::provider_output_item_done(
                    "llm",
                    node,
                    0,
                    item.clone(),
                ));
            }
            facts.push(debug_stream_events::answer_text_delta(
                "answer",
                "selected marker".into(),
                0,
                Some("llm"),
                Some(node),
                Some("text"),
            ));
            facts.push(debug_stream_events::flow_finished(run.id, json!({})));
            let mut events = vec![];
            for (index, fact) in facts.into_iter().enumerate() {
                let envelope = if cold {
                    crate::routes::application_public_api::stream_terminal_fallback::durable_record_to_runtime_event_envelope(domain::RuntimeEventRecord {
                        id: Uuid::now_v7(), flow_run_id: run.id, node_run_id: fact.payload["node_run_id"].as_str().and_then(|id| Uuid::parse_str(id).ok()),
                        span_id: None, parent_span_id: None, sequence: index as i64 + 1,
                        event_type: fact.event_type, layer: domain::RuntimeEventLayer::RuntimeItem,
                        source: domain::RuntimeEventSource::Host, trust_level: domain::RuntimeTrustLevel::HostFact,
                        item_id: None, ledger_ref: None, payload: fact.payload,
                        visibility: domain::RuntimeEventVisibility::Workspace, durability: domain::RuntimeEventDurability::Durable,
                        created_at: time::OffsetDateTime::now_utc(),
                    })
                } else {
                    RuntimeEventEnvelope::new(run.id, index as i64 + 1, fact)
                };
                events.extend(mapper.runtime_event_to_sse(&run, envelope));
            }
            let bytes = axum::body::to_bytes(
                test_projected_events_response(events).into_body(),
                usize::MAX,
            )
            .await
            .unwrap();
            let decoded = decode_responses_sse(std::str::from_utf8(&bytes).unwrap());
            assert_eq!(decoded.text_deltas, vec!["selected marker"]);
            assert_eq!(decoded.completed_count, 1);
            assert_eq!(decoded.completed_output.len(), 1);
            if native {
                assert_eq!(decoded.completed_output, vec![item]);
            } else {
                assert_eq!(
                    decoded.completed_output[0]["content"][0]["text"],
                    "selected marker"
                );
            }
        }
    }
}

#[tokio::test]
async fn preferred_sse_missing_host_selection_fails_instead_of_completing_empty() {
    let mut run = native_run();
    run.status = NativeRunStatus::Succeeded;
    let node = Uuid::now_v7();
    let mut mapper = OpenAiResponseStreamMapper::with_mode(
        "fixture".into(),
        None,
        ResponsesProjectionMode::AwaitingHostSelection,
    );
    let mut events = vec![];
    for (index, fact) in [
        debug_stream_events::answer_text_delta(
            "answer",
            "unselected".into(),
            0,
            Some("llm"),
            Some(node),
            Some("text"),
        ),
        debug_stream_events::flow_finished(run.id, json!({})),
    ]
    .into_iter()
    .enumerate()
    {
        events.extend(
            mapper
                .runtime_event_to_sse(&run, RuntimeEventEnvelope::new(run.id, index as i64, fact)),
        );
    }
    let bytes = axum::body::to_bytes(
        test_projected_events_response(events).into_body(),
        usize::MAX,
    )
    .await
    .unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    let decoded = decode_responses_sse(body);
    assert_eq!(decoded.completed_count, 0);
    assert!(decoded.text_deltas.is_empty());
    assert_eq!(
        decoded.error_messages,
        vec!["host input selection is unavailable"]
    );
    assert!(body.contains("llm_input_selection_missing"));
}

#[tokio::test]
async fn responses_resume_sse_usage_is_round_local_for_every_terminal_kind() {
    use crate::routes::application_public_api::openai::usage::ResponseUsageBaseline;
    for event_type in ["flow_finished", "flow_incomplete", "waiting_callback"] {
        for known in [true, false] {
            for mode in [
                ResponsesProjectionMode::SemanticNativeResponses,
                ResponsesProjectionMode::TransparentProviderResponses,
            ] {
                let mut run = native_run();
                run.usage = Some(NativeUsage {
                    prompt_tokens: Some(220),
                    completion_tokens: Some(35),
                    total_tokens: Some(255),
                    cache_read_tokens: Some(150),
                    reasoning_tokens: Some(12),
                    ..NativeUsage::default()
                });
                let payload = if known {
                    json!({"native_usage_baseline_source":"usage_ledger","native_usage_baseline":{"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"cache_read_tokens":60,"reasoning_tokens":5}})
                } else {
                    json!({})
                };
                let baseline = ResponseUsageBaseline::from_callback_payload(&payload);
                let mut mapper = OpenAiResponseStreamMapper::with_mode(
                    "provider/model".into(),
                    Some("resp_prior".into()),
                    mode,
                )
                .with_usage_baseline(baseline);
                let payload = if event_type == "waiting_callback" {
                    json!({"callback_task_id":Uuid::now_v7(),"callback_kind":"llm_tool_calls","required_action":{"action_type":"submit_tool_outputs","payload":{"tool_calls":[{"id":"call-test","name":"probe","arguments":{}}]}}})
                } else {
                    json!({})
                };
                let envelope = RuntimeEventEnvelope::new(
                    run.id,
                    1,
                    RuntimeEventPayload {
                        event_type: event_type.into(),
                        source: RuntimeEventSource::Runtime,
                        durability: RuntimeEventDurability::DurableRequired,
                        persist_required: true,
                        trace_visible: true,
                        payload,
                    },
                );
                let response =
                    test_projected_events_response(mapper.runtime_event_to_sse(&run, envelope));
                let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap();
                let body = String::from_utf8(bytes.to_vec()).unwrap();
                let terminal: Value = body
                    .lines()
                    .filter_map(|line| line.strip_prefix("data: "))
                    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                    .find(|event| {
                        matches!(
                            event["type"].as_str(),
                            Some("response.completed" | "response.incomplete")
                        )
                    })
                    .expect("terminal Responses SSE");
                if known {
                    assert_eq!(
                        terminal["response"]["usage"],
                        json!({"input_tokens":120,"output_tokens":15,"total_tokens":135,"input_tokens_details":{"cached_tokens":90},"output_tokens_details":{"reasoning_tokens":7}})
                    );
                } else {
                    assert!(terminal["response"]["usage"].is_null());
                }
                assert_eq!(run.usage.unwrap().prompt_tokens, Some(220));
            }
        }
    }
}
