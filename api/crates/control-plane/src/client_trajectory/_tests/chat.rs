use super::*;

fn chat_request() -> Vec<u8> {
    serde_json::to_vec(&json!({"model":"deepseek-chat","stream":true,
        "tools":[{"type":"function","function":{"name":"weather","parameters":{"type":"object"}}}],
        "messages":[{"role":"system","content":"Be helpful"},{"role":"user","content":"北京 🌍"},
            {"role":"assistant","content":null,"tool_calls":[{"id":"old","type":"function","function":{"name":"weather","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"old","content":"sunny"}]
    })).unwrap()
}
fn chat_wire(done: bool) -> String {
    let events = [
        json!({"id":"chat-1","choices":[{"index":0,"delta":{"reasoning_content":"think 🌍"},"finish_reason":null}]}),
        json!({"id":"chat-1","choices":[{"index":0,"delta":{"content":"你好","tool_calls":[{"index":0,"id":"call-","type":"function","function":{"name":"wea","arguments":"{\"city\":"}}]},"finish_reason":null}]}),
        json!({"id":"chat-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"1","function":{"name":"ther","arguments":"\"北京\"}"}}]},"finish_reason":"tool_calls"}]}),
        json!({"id":"chat-1","choices":[],"usage":{"prompt_tokens":9,"completion_tokens":4}}),
    ];
    let mut wire = events
        .iter()
        .map(|e| format!("data: {e}\r\n\r\n"))
        .collect::<String>();
    if done {
        wire.push_str("data: [DONE]\n\n");
    }
    wire
}
#[tokio::test]
async fn chat_sse_exact_bytes_fragmented_utf8_tool_arguments_schema_and_usage() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    let request = chat_request();
    let wire = chat_wire(true);
    recorder
        .record(ClientTrajectoryFrameKind::Request, &request)
        .await
        .unwrap();
    let run = Uuid::now_v7();
    let node = Uuid::now_v7();
    recorder.link_llm_node(run, node);
    for byte in wire.as_bytes() {
        recorder
            .record(ClientTrajectoryFrameKind::ResponseSse, &[*byte])
            .await
            .unwrap();
    }
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    assert_eq!(raw(&records, "submitted"), request);
    assert_eq!(raw(&records, "emitted"), wire.as_bytes());
    let steps: Vec<_> = records
        .iter()
        .filter_map(|r| match &r.fact {
            ClientTrajectoryFact::Step { step } => Some(step),
            _ => None,
        })
        .collect();
    assert!(steps
        .iter()
        .all(|step| step.protocol == "chat_completions" || step.name == "Responses request"));
    for category in [
        "system",
        "user",
        "assistant",
        "reasoning",
        "tool_call",
        "tool_result",
        "tool_definition",
        "usage",
    ] {
        assert!(
            steps.iter().any(|step| step.category == category),
            "missing {category}"
        );
    }
    let call = steps
        .iter()
        .find(|s| s.origin == "emitted" && s.category == "tool_call")
        .unwrap();
    assert_eq!(call.name, "weather");
    assert_eq!(call.call_id.as_deref(), Some("call-1"));
    assert_eq!(
        call.parameters_preview.as_deref(),
        Some("{\"city\":\"北京\"}")
    );
    assert!(call.available_sections.iter().any(|s| s == "schema"));
    let result = steps.iter().find(|s| s.category == "tool_result").unwrap();
    assert!(result.related_step_id.is_some());
    assert!(records.iter().any(
        |r| matches!(r.fact,ClientTrajectoryFact::NodeLink{node_run_id} if node_run_id==node)
    ));
}
#[tokio::test]
async fn chat_missing_done_and_unfinished_deltas_are_incomplete() {
    for wire in [chat_wire(false), "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"},\"finish_reason\":null}]}\n\ndata: [DONE]\n\n".into()] {
        let writer = Arc::new(MemoryWriter::default());
        let recorder = capture(writer.clone());
        recorder.record(ClientTrajectoryFrameKind::Request, &chat_request()).await.unwrap();
        recorder.bind_run(Uuid::now_v7(), None);
        recorder.record(ClientTrajectoryFrameKind::ResponseSse, wire.as_bytes()).await.unwrap();
        recorder.finish();
        recorder.wait_finished().await;
        assert!(!complete(&writer.records.lock().unwrap()));
    }
}
#[tokio::test]
async fn chat_unary_preserves_message_content_and_error_terminal() {
    for response in [
        json!({"id":"chat-unary","object":"chat.completion","choices":[{"index":0,"message":{"role":"assistant","content":"你好 🌍","reasoning_content":"reason"},"finish_reason":"stop"}],"usage":{"total_tokens":12}}),
        json!({"error":{"code":"provider_error","message":"failure"}}),
    ] {
        let writer = Arc::new(MemoryWriter::default());
        let recorder = capture(writer.clone());
        recorder
            .record(ClientTrajectoryFrameKind::Request, &chat_request())
            .await
            .unwrap();
        recorder.bind_run(Uuid::now_v7(), None);
        let bytes = serde_json::to_vec(&response).unwrap();
        recorder
            .record(ClientTrajectoryFrameKind::ResponseJson, &bytes)
            .await
            .unwrap();
        recorder.finish();
        recorder.wait_finished().await;
        let records = writer.records.lock().unwrap();
        assert!(complete(&records));
        assert_eq!(raw(&records, "emitted"), bytes);
        assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Step{step} if step.origin=="emitted" && step.category==if response.get("error").is_some(){"error"}else{"assistant"})));
    }
}

#[tokio::test]
async fn chat_tool_fragments_preserve_numeric_index_order_and_exact_arguments() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder
        .record(ClientTrajectoryFrameKind::Request, &chat_request())
        .await
        .unwrap();
    recorder.bind_run(Uuid::now_v7(), None);
    let chunk = json!({"choices": [{"index": 0, "delta": {"tool_calls": [
        {"index": 10, "id": "second", "type": "function", "function": {"name": "later", "arguments": "{}"}},
        {"index": 2, "id": "first", "type": "function", "function": {"name": "earlier", "arguments": "{\"city\":\"北京\"}"}}
    ]}, "finish_reason": "tool_calls"}]});
    let wire = format!("data: {chunk}\n\ndata: [DONE]\n\n");
    recorder
        .record(ClientTrajectoryFrameKind::ResponseSse, wire.as_bytes())
        .await
        .unwrap();
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    assert_eq!(raw(&records, "emitted"), wire.as_bytes());
    let calls: Vec<_> = records
        .iter()
        .filter_map(|record| match &record.fact {
            ClientTrajectoryFact::Step { step }
                if step.origin == "emitted" && step.category == "tool_call" =>
            {
                Some(step)
            }
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].call_id.as_deref(), Some("first"));
    assert_eq!(
        calls[0].parameters_preview.as_deref(),
        Some("{\"city\":\"北京\"}")
    );
    assert_eq!(calls[1].call_id.as_deref(), Some("second"));
    assert_eq!(calls[1].parameters_preview.as_deref(), Some("{}"));
}
