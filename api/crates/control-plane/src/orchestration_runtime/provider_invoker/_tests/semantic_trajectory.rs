use super::*;
use crate::ports::{RuntimeEventDurability, RuntimeEventSource};

fn raw(sequence: u64, kind: &str, transport: &str, bytes: &[u8]) -> RuntimeEventPayload {
    RuntimeEventPayload {
        event_type: "provider_protocol_observation".into(),
        source: RuntimeEventSource::Provider,
        durability: RuntimeEventDurability::DurableRequired,
        persist_required: true,
        trace_visible: false,
        payload: json!({"flow_run_id":"run", "node_id":"model", "node_run_id":"node", "invocation_id":"invocation", "provider_attempt_index":0,"sequence":sequence,"kind":kind,"protocol":"fixture","transport":transport,"encoding":"base64","body":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    }
}
fn project(parts: Vec<&[u8]>, transport: &str) -> Vec<Value> {
    let mut projector = SemanticProjector::default();
    projector.observe(&raw(1, "request_prepared", "http", b"{}"));
    for (index, bytes) in parts.iter().enumerate() {
        projector.observe(&raw(index as u64 + 2, "response_body", transport, bytes));
    }
    projector
        .observe(&raw(parts.len() as u64 + 2, "stream_end", transport, b""))
        .into_iter()
        .map(|event| {
            let mut p = event.payload;
            p.as_object_mut().unwrap().remove("raw_sequence_start");
            p.as_object_mut().unwrap().remove("raw_sequence_end");
            p
        })
        .collect()
}
#[test]
fn semantic_steps_ignore_sse_and_utf8_chunk_boundaries() {
    let bytes =
        "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"}}]}\r\n\r\ndata: [DONE]\r\n\r\n"
            .as_bytes();
    let expected = project(vec![bytes], "sse");
    for split in 1..bytes.len() {
        assert_eq!(
            project(vec![&bytes[..split], &bytes[split..]], "sse"),
            expected,
            "split {split}"
        );
    }
    assert_eq!(
        expected
            .iter()
            .filter(|p| p["kind"] == "model_reply")
            .count(),
        1
    );
    assert!(expected.iter().any(|p| p["preview"] == "你好"));
}
#[test]
fn native_protocol_families_have_semantics_not_chunk_steps() {
    for body in [
        json!({"choices":[{"message":{"content":"hello","tool_calls":[{"id":"call-1","function":{"arguments":"{}"}}]}}]}),
        json!({"output":[{"type":"message","content":[{"type":"output_text","text":"hello"}]},{"type":"function_call","call_id":"call-1","arguments":"{}"}]}),
        json!({"content":[{"type":"text","text":"hello"},{"type":"tool_use","id":"call-1","input":{}}]}),
        json!({"candidates":[{"content":{"parts":[{"text":"hello"},{"functionCall":{"id":"call-1","args":{}}}]}}]}),
    ] {
        let bytes = serde_json::to_vec(&body).unwrap();
        for transport in ["http", "websocket"] {
            let steps = project(vec![&bytes], transport);
            assert_eq!(
                steps.iter().filter(|p| p["kind"] == "model_reply").count(),
                1
            );
            assert_eq!(steps.iter().filter(|p| p["kind"] == "tool_call").count(), 1);
            assert!(!steps.iter().any(|p| p["kind"] == "tool_result"));
        }
    }
}
#[test]
fn submitted_tool_result_is_not_claimed_as_execution() {
    let mut projector = SemanticProjector::default();
    let result = projector.observe(&raw(
        1,
        "request_prepared",
        "http",
        br#"{"input":[{"type":"function_call_output","call_id":"call-1","output":"ok"}]}"#,
    ));
    let step = result
        .iter()
        .find(|e| e.payload["kind"] == "tool_result")
        .unwrap();
    assert_eq!(step.payload["provenance"], "submitted_tool_result");
    assert_eq!(step.payload["tool_call_id"], "call-1");
}
#[test]
fn missing_bytes_and_limits_do_not_produce_complete_reply() {
    for oversized in [false, true] {
        let mut projector = SemanticProjector::default();
        projector.observe(&raw(1, "request_prepared", "http", b"{}"));
        projector.observe(&raw(
            if oversized { 2 } else { 3 },
            "response_body",
            "sse",
            &vec![b'x'; if oversized { MAX_BUFFER + 1 } else { 5 }],
        ));
        let result = projector.observe(&raw(4, "stream_end", "sse", b""));
        assert!(
            result
                .iter()
                .any(|e| e.payload["kind"] == "observation_gap"
                    && e.payload["status"] == "incomplete")
        );
        assert!(!result.iter().any(|e| e.payload["kind"] == "model_reply"));
    }
}
#[test]
fn terminal_integrity_releases_bounded_state() {
    let mut projector = SemanticProjector::default();
    projector.observe(&raw(1, "request_prepared", "http", b"{}"));
    let mut integrity = raw(2, "", "http", b"");
    integrity.event_type = "provider_protocol_integrity".into();
    integrity.payload["status"] = json!("incomplete");
    let out = projector.observe(&integrity);
    assert!(projector.attempts.is_empty());
    assert!(out.iter().any(|e| e.payload["status"] == "incomplete"));
}

#[test]
fn responses_and_anthropic_sse_keep_tool_identity_separate_from_reply() {
    for frames in [
        vec![
            json!({"type":"response.output_text.delta","output_index":0,"delta":"hello"}),
            json!({"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","call_id":"call-1"}}),
            json!({"type":"response.function_call_arguments.delta","output_index":1,"delta":"{}"}),
        ],
        vec![
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hello"}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call-1","input":{}}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{}"}}),
        ],
    ] {
        let bytes = frames
            .iter()
            .map(|v| format!("data: {v}\n\n"))
            .collect::<String>()
            .into_bytes();
        let expected = project(vec![&bytes], "sse");
        assert_eq!(
            expected
                .iter()
                .filter(|p| p["kind"] == "model_reply")
                .count(),
            1
        );
        let call = expected.iter().find(|p| p["kind"] == "tool_call").unwrap();
        assert_eq!(call["tool_call_id"], "call-1");
        assert_eq!(call["preview"], "{}");
        assert_eq!(project(bytes.chunks(1).collect(), "sse"), expected);
    }
}

#[test]
fn errors_unknown_protocol_and_step_limits_are_explicit() {
    let error = project(vec![br#"{"error":{"message":"failed"}}"#], "http");
    assert!(error.iter().any(|p| p["kind"] == "error"));
    let unknown = project(vec![br#"{"unknown":"shape"}"#], "http");
    assert!(unknown
        .iter()
        .any(|p| p["kind"] == "observation_gap" && p["status"] == "unavailable"));
    let calls = (0..MAX_STEPS + 1)
        .map(|i| json!({"type":"function_call","call_id":format!("call-{i}")}))
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&json!({"output":calls})).unwrap();
    let capped = project(vec![&bytes], "http");
    assert!(capped.len() <= MAX_STEPS + 1);
    assert!(capped.iter().any(|p| p["reason"] == "step_limit"));
}
