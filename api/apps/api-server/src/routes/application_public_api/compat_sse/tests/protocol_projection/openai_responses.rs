use super::*;

#[derive(Default)]
struct DecodedResponsesStream {
    text_deltas: Vec<String>,
    completed_count: usize,
    error_messages: Vec<String>,
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
                decoded.error_messages.push(
                    payload["error"]["message"]
                        .as_str()
                        .expect("Responses failure should contain error.message")
                        .to_string(),
                );
            }
            _ => {}
        }
    }
    decoded
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
    assert_eq!(decoded.text_deltas.concat(), fixture.concat());
    assert_eq!(decoded.completed_count, 1);
    assert!(!body.contains("terminal answer must not be reconstructed"));
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
    let mut mapper = OpenAiResponseStreamMapper::new("1flowbase".to_string(), None);
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
            debug_stream_events::provider_output_item_done(
                "node-llm",
                node_run_id,
                2,
                approval.clone(),
            ),
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
    assert_eq!(decoded.output_item_events[0]["sequence_number"], 7);
    assert_eq!(
        decoded.output_item_events[1]["type"],
        "response.output_item.done"
    );
    assert_eq!(decoded.output_item_events[1]["output_index"], 2);
    assert_eq!(decoded.output_item_events[1]["item"], approval);
    assert_eq!(decoded.completed_output, vec![approval]);
}
