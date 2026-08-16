use serde_json::json;

use super::super::run_activity::{
    project_assistant_run_activity, AssistantRunActivityItem, AssistantRunToolStatus,
};
use crate::routes::debug_run_stream::RuntimeEventStreamEnvelopeResponse;

fn event(
    event_type: &str,
    sequence: i64,
    payload: serde_json::Value,
) -> RuntimeEventStreamEnvelopeResponse {
    RuntimeEventStreamEnvelopeResponse {
        event_id: format!("run:1:{sequence}"),
        run_id: "run:1".to_string(),
        node_run_id: Some("node-run:1".to_string()),
        event_type: event_type.to_string(),
        sequence,
        created_at: "2026-08-16T01:00:00Z".to_string(),
        text: payload
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        payload,
        delta_index: None,
        content_type: None,
    }
}

#[test]
fn assistant_activity_sequence_projection_uses_canonical_tool_contract_and_answer_segments() {
    let started = event(
        "assistant_tool_call_started",
        2,
        json!({
            "tool_call": {
                "id": "call-list",
                "name": "1flowbase_mcp_list",
                "arguments": { "path": "/后台设置" }
            }
        }),
    );
    let finished = event(
        "assistant_tool_call_finished",
        3,
        json!({
            "tool_call": {
                "id": "call-list",
                "name": "1flowbase_mcp_list",
                "arguments": { "path": "/后台设置" }
            },
            "tool_result": { "content": ["后台设置"], "is_error": false },
            "duration_ms": 42
        }),
    );
    let output = event(
        "text_delta",
        4,
        json!({
            "text": "最终回答",
            "presentation": { "kind": "answer", "segment_index": 7 }
        }),
    );

    match project_assistant_run_activity(&started).expect("tool start is visible") {
        AssistantRunActivityItem::Tool {
            sequence_start,
            sequence_end,
            tool_call_id,
            tool_name,
            input,
            output,
            status,
            ..
        } => {
            assert_eq!(sequence_start, 2);
            assert_eq!(sequence_end, 2);
            assert_eq!(tool_call_id, "call-list");
            assert_eq!(tool_name, "1flowbase_mcp_list");
            assert_eq!(input, json!({ "path": "/后台设置" }));
            assert!(output.is_none());
            assert!(matches!(status, AssistantRunToolStatus::Running));
        }
        _ => panic!("expected tool activity"),
    }
    match project_assistant_run_activity(&finished).expect("tool finish is visible") {
        AssistantRunActivityItem::Tool {
            output,
            duration_ms,
            is_error,
            ..
        } => {
            assert_eq!(
                output,
                Some(json!({ "content": ["后台设置"], "is_error": false }))
            );
            assert_eq!(duration_ms, Some(42));
            assert!(!is_error);
        }
        _ => panic!("expected tool activity"),
    }
    match project_assistant_run_activity(&output).expect("answer delta is visible") {
        AssistantRunActivityItem::Output {
            text,
            segment_index,
            ..
        } => {
            assert_eq!(text, "最终回答");
            assert_eq!(segment_index, Some(7));
        }
        _ => panic!("expected output activity"),
    }
}

#[test]
fn activity_projection_rejects_tool_aliases_instead_of_guessing() {
    let legacy = event(
        "assistant_tool_call_started",
        2,
        json!({
            "tool_call": {
                "tool_call_id": "legacy-call",
                "function": { "name": "legacy-tool" }
            }
        }),
    );

    assert!(project_assistant_run_activity(&legacy).is_none());
}

#[test]
fn assistant_activity_sequence_projection_exposes_the_full_durable_stream_interval() {
    let reasoning = event(
        "reasoning_delta",
        9,
        json!({
            "text": "先检查，再继续",
            "sequence_start": 3,
            "sequence_end": 9,
            "presentation": { "kind": "answer", "segment_index": 0 }
        }),
    );

    match project_assistant_run_activity(&reasoning).expect("reasoning is visible") {
        AssistantRunActivityItem::Reasoning {
            sequence_start,
            sequence_end,
            ..
        } => {
            assert_eq!(sequence_start, 3);
            assert_eq!(sequence_end, 9);
        }
        _ => panic!("expected reasoning activity"),
    }

    let node_debug_reasoning = event(
        "reasoning_delta",
        10,
        json!({
            "text": "节点调试副本",
            "sequence_start": 10,
            "sequence_end": 10
        }),
    );
    assert!(
        project_assistant_run_activity(&node_debug_reasoning).is_none(),
        "AC-004 assistant activity must project only canonical Answer Presentation reasoning"
    );
}
