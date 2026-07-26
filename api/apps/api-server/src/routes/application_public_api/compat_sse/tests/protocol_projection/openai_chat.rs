use super::*;

#[derive(Default)]
struct DecodedOpenAiChatStream {
    text_deltas: Vec<String>,
    finish_reasons: Vec<String>,
    done_count: usize,
}

fn decode_openai_chat_sse(body: &str) -> DecodedOpenAiChatStream {
    let mut decoded = DecodedOpenAiChatStream::default();
    for data in body.lines().filter_map(|line| line.strip_prefix("data: ")) {
        if data == "[DONE]" {
            decoded.done_count += 1;
            continue;
        }

        let payload: serde_json::Value =
            serde_json::from_str(data).expect("OpenAI Chat SSE data must be valid JSON");
        let choice = &payload["choices"][0];
        if choice["finish_reason"].is_null() {
            if let Some(text) = choice["delta"]["content"].as_str() {
                decoded.text_deltas.push(text.to_string());
            }
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            decoded.finish_reasons.push(reason.to_string());
        }
    }
    decoded
}

#[tokio::test]
async fn ac_001_ac_002_openai_chat_decoder_preserves_ordered_text_deltas_exactly() {
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
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
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
        assert_eq!(
            projected.len(),
            1,
            "each canonical text event must project immediately"
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
        .expect("OpenAI Chat SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("OpenAI Chat SSE should be UTF-8");
    let decoded = decode_openai_chat_sse(&body);
    let expected_deltas = fixture.map(str::to_string);

    assert_eq!(decoded.text_deltas, expected_deltas);
    assert_eq!(decoded.text_deltas.concat(), fixture.concat());
    assert!(!body.contains("terminal answer must not be reconstructed"));
}

#[tokio::test]
async fn ac_003_openai_chat_first_terminal_is_absorbing_and_finishes_once() {
    let run = native_run();
    let mut mapper =
        OpenAiChatStreamMapper::new("1flowbase".to_string(), "chatcmpl-test".to_string());
    let mut events = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            1,
            debug_stream_events::answer_text_delta(
                "node-answer",
                "kept".to_string(),
                0,
                Some("node-llm"),
                None,
                Some("text"),
            ),
        ),
    );
    let terminal = mapper.runtime_event_to_sse(
        &run,
        RuntimeEventEnvelope::new(
            run.id,
            2,
            debug_stream_events::flow_finished(run.id, json!({ "answer": "kept" })),
        ),
    );
    assert_eq!(
        terminal.len(),
        2,
        "success terminal must emit finish and DONE"
    );
    events.extend(terminal);

    assert!(
        mapper
            .runtime_event_to_sse(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    3,
                    debug_stream_events::flow_finished(run.id, json!({ "answer": "kept" })),
                ),
            )
            .is_empty(),
        "duplicate terminal must be absorbed"
    );
    assert!(
        mapper
            .runtime_event_to_sse(
                &run,
                RuntimeEventEnvelope::new(
                    run.id,
                    4,
                    debug_stream_events::answer_text_delta(
                        "node-answer",
                        "must-not-appear".to_string(),
                        1,
                        Some("node-llm"),
                        None,
                        Some("text"),
                    ),
                ),
            )
            .is_empty(),
        "post-terminal text must be absorbed"
    );

    let response = test_projected_events_response(events);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("OpenAI Chat SSE body should be readable");
    let body = String::from_utf8(body.to_vec()).expect("OpenAI Chat SSE should be UTF-8");
    let decoded = decode_openai_chat_sse(&body);

    assert_eq!(decoded.text_deltas, ["kept".to_string()]);
    assert_eq!(decoded.finish_reasons, ["stop".to_string()]);
    assert_eq!(decoded.done_count, 1);
    assert!(!body.contains("must-not-appear"));
}
