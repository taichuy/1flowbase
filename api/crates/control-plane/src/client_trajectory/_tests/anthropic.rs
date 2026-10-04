use super::*;

fn request() -> Vec<u8> {
    serde_json::to_vec(&json!({"model":"claude-sonnet-5-5","max_tokens":1000,"stream":true,
        "system":[{"type":"text","text":"Remember 北京"}], "thinking":{"type":"adaptive"},
        "tools":[{"name":"weather","input_schema":{"type":"object","properties":{"city":{"type":"string"}}}}],
        "messages":[{"role":"user","content":"Hello 🌍"},
            {"role":"assistant","content":[{"type":"tool_use","id":"old","name":"weather","input":{"city":"北京"}}]},
            {"role":"user","content":[{"type":"tool_result","tool_use_id":"old","content":"sunny"}]}]
    })).unwrap()
}
fn events() -> Vec<serde_json::Value> {
    vec![
        json!({"type":"message_start","message":{"id":"msg-1","role":"assistant","content":[],"usage":{"input_tokens":9,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"think 🌍"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed-"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"value"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu-1","name":"weather","input":{}}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"city\":"}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"北京\"}"}}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":4}}),
        json!({"type":"message_stop"}),
    ]
}
async fn recorded(
    kind: ClientTrajectoryFrameKind,
    wire: &[u8],
) -> Vec<AppendClientTrajectoryInput> {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder.use_anthropic_messages_protocol();
    let req = request();
    recorder
        .record(ClientTrajectoryFrameKind::Request, &req)
        .await
        .unwrap();
    let run = Uuid::now_v7();
    let node = Uuid::now_v7();
    recorder.link_llm_node(run, node);
    for chunk in wire.chunks(1) {
        recorder.record(kind, chunk).await.unwrap();
    }
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap().clone();
    assert_eq!(raw(&records, "submitted"), req);
    assert_eq!(raw(&records, "emitted"), wire);
    assert!(records.iter().any(
        |r| matches!(&r.fact,ClientTrajectoryFact::NodeLink{node_run_id} if *node_run_id==node)
    ));
    records
}
#[tokio::test]
async fn anthropic_stream_preserves_raw_utf8_tool_json_signature_schema_and_run_link() {
    let wire = events()
        .iter()
        .map(|e| {
            format!(
                "event: {}\r\ndata: {e}\r\n\r\n",
                e["type"].as_str().unwrap()
            )
        })
        .collect::<String>();
    let records = recorded(ClientTrajectoryFrameKind::ResponseSse, wire.as_bytes()).await;
    assert!(complete(&records));
    let steps: Vec<_> = records
        .iter()
        .filter_map(|r| match &r.fact {
            ClientTrajectoryFact::Step { step } => Some(step),
            _ => None,
        })
        .collect();
    assert!(steps.iter().all(|s| s.protocol == "anthropic_messages"));
    for category in [
        "system",
        "user",
        "tool_definition",
        "tool_call",
        "tool_result",
        "reasoning",
        "usage",
    ] {
        assert!(steps.iter().any(|s| s.category == category), "{category}");
    }
    let call = steps
        .iter()
        .find(|s| s.origin == "emitted" && s.category == "tool_call")
        .unwrap();
    assert_eq!(call.call_id.as_deref(), Some("toolu-1"));
    assert!(call.available_sections.iter().any(|s| s == "schema"));
    let submitted_call = steps
        .iter()
        .find(|s| s.call_id.as_deref() == Some("old") && s.category == "tool_call")
        .unwrap();
    let result = steps.iter().find(|s| s.category == "tool_result").unwrap();
    assert_eq!(result.related_step_id, Some(submitted_call.id));
    assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Section{step_id,section,value} if *step_id==call.id && section=="parameters" && *value==json!({"city":"北京"}))));
    assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Section{section,value,..} if section=="overview" && value["signature"]=="signed-value" && value["thinking"]=="think 🌍")));
    assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Section{section,value,..} if section=="usage" && value["input_tokens"]==9 && value["output_tokens"]==4)));
    assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::ResponseLink{response_id} if response_id=="msg-1")));
}
#[tokio::test]
async fn anthropic_blocking_message_and_truthful_error_are_terminal() {
    for response in [
        json!({"id":"msg-unary","type":"message","role":"assistant","content":[{"type":"text","text":"你好 🌍"},{"type":"thinking","thinking":"reason","signature":"opaque"}],"usage":{"input_tokens":5,"output_tokens":2}}),
        json!({"type":"error","error":{"type":"overloaded_error","message":"original upstream message"},"request_id":"req-upstream"}),
    ] {
        let records = recorded(
            ClientTrajectoryFrameKind::ResponseJson,
            &serde_json::to_vec(&response).unwrap(),
        )
        .await;
        assert!(complete(&records));
        let category = if response["type"] == "error" {
            "error"
        } else {
            "assistant"
        };
        assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Step{step} if step.category==category && step.origin=="emitted")));
        if category == "error" {
            assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Section{section,value,..} if section=="overview" && *value==response)));
        }
    }
}
#[tokio::test]
async fn anthropic_eof_and_malformed_tool_json_preserve_partial_evidence_as_incomplete() {
    let cases = [
        vec![
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"unfinished"}}),
        ],
        vec![
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"partial","name":"weather","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"city\":"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_stop"}),
        ],
        events()
            .into_iter()
            .filter(|v| v["type"] != "message_stop")
            .collect(),
    ];
    for case in cases {
        let wire = case
            .iter()
            .map(|v| format!("data: {v}\n\n"))
            .collect::<String>();
        let records = recorded(ClientTrajectoryFrameKind::ResponseSse, wire.as_bytes()).await;
        assert!(!complete(&records));
        assert!(records
            .iter()
            .any(|r| matches!(&r.fact,ClientTrajectoryFact::Step{step} if step.origin=="emitted")));
    }
}
#[tokio::test]
async fn anthropic_stream_error_preserves_partial_content_and_error_without_fake_success() {
    let wire="data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"partial\"}}\n\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"upstream failure\"}}\n\n";
    let records = recorded(ClientTrajectoryFrameKind::ResponseSse, wire.as_bytes()).await;
    assert!(!complete(&records));
    for category in ["reasoning", "error"] {
        assert!(records.iter().any(|r|matches!(&r.fact,ClientTrajectoryFact::Step{step} if step.origin=="emitted" && step.category==category)));
    }
}

#[tokio::test]
async fn anthropic_count_tokens_response_uses_same_raw_capture_and_usage_read_model() {
    let response = br#" {"input_tokens":17} "#;
    let records = recorded(ClientTrajectoryFrameKind::ResponseJson, response).await;
    assert!(complete(&records));
    assert!(records.iter().any(|r| matches!(&r.fact,
        ClientTrajectoryFact::Section { section, value, .. }
        if section == "usage" && value["input_tokens"] == 17)));
}
