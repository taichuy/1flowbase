use super::*;
use serde_json::json;

#[test]
fn ac_010_native_request_overlay_preserves_completed_callback_truth() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({ "prompt": "weather" }),
            output_payload: json!({ "answer": "done" }),
            error_payload: None,
            metrics_payload: json!({ "usage": { "total_tokens": 12 } }),
            debug_payload: json!({
                "tool_callbacks": [
                    {
                        "id": "call-weather",
                        "name": "weather"
                    }
                ],
                "llm_rounds": [
                    {
                        "round_index": 0,
                        "assistant": {
                            "tool_calls": [
                                {
                                    "id": "call-weather",
                                    "name": "weather"
                                }
                            ]
                        }
                    }
                ],
                "debug_summary": {
                    "kept": true
                }
            }),
            started_at: now,
            finished_at: Some(now + time::Duration::seconds(2)),
        }],
        checkpoints: Vec::new(),
        callback_tasks: vec![domain::CallbackTaskRecord {
            id: callback_task_id,
            flow_run_id,
            node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Completed,
            request_payload: json!({
                "tool_calls": [
                    {
                        "id": "call-weather",
                        "name": "weather",
                        "call_usage": {
                            "input_tokens": 11,
                            "output_tokens": 3,
                            "total_tokens": 14
                        }
                    }
                ]
            }),
            response_payload: Some(json!({
                "tool_results": [
                    {
                        "tool_call_id": "call-weather",
                        "content": "22c"
                    }
                ]
            })),
            external_ref_payload: None,
            created_at: now + time::Duration::seconds(1),
            completed_at: Some(now + time::Duration::seconds(2)),
        }],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
    };

    let baseline = build_application_run_trace_projection(&detail).unwrap();
    let original = baseline
        .contents
        .iter()
        .find(|content| content.content_kind == "tool_callback")
        .unwrap();
    let original_node = baseline
        .nodes
        .iter()
        .find(|node| node.trace_node_id == original.trace_node_id)
        .unwrap();
    let item = json!({"type":"custom_tool_call","call_id":"call-weather","name":"weather","input":"weather --city Tokyo","status":"completed"});
    let mut detail = detail;
    detail.native_messages = vec![json!({"_source_item":item})];
    let projection = build_application_run_trace_projection(&detail).unwrap();
    let content = projection
        .contents
        .iter()
        .find(|content| content.content_kind == "tool_callback")
        .unwrap();
    let node = projection
        .nodes
        .iter()
        .find(|node| node.trace_node_id == content.trace_node_id)
        .unwrap();
    assert_eq!(content.payload["request_payload"], item);
    assert_eq!(content.payload["tool_call"], item);
    for field in [
        "callback_task_id",
        "callback_status",
        "callback_payload",
        "parsed_result",
        "tool_result",
        "call_usage",
        "duration_ms",
    ] {
        assert_eq!(
            content.payload[field], original.payload[field],
            "preserve {field}"
        );
    }
    assert_eq!(content.payload["callback_task_id"], json!(callback_task_id));
    assert_eq!(content.payload["tool_result"]["content"], json!("22c"));
    assert_eq!(node.status, original_node.status);
    assert_eq!(node.finished_at, original_node.finished_at);
    assert_eq!(node.duration_ms, original_node.duration_ms);
    assert_eq!(node.trace_node_id, original_node.trace_node_id);
    for source in original.source_refs.as_array().unwrap() {
        assert!(content.source_refs.as_array().unwrap().contains(source));
    }
    assert!(
        content
            .source_refs
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["source_kind"] == "application_run_conversation_message_items")
    );
}

fn flow_run(flow_run_id: Uuid, now: OffsetDateTime) -> domain::FlowRunRecord {
    domain::FlowRunRecord {
        id: flow_run_id,
        application_id: Uuid::now_v7(),
        flow_id: Uuid::now_v7(),
        draft_id: Uuid::now_v7(),
        compiled_plan_id: None,
        debug_session_id: "debug-session".to_string(),
        flow_schema_version: "1flowbase.flow/v2".to_string(),
        document_hash: "hash".to_string(),
        run_mode: domain::FlowRunMode::DebugFlowRun,
        target_node_id: None,
        title: "debug flow".to_string(),
        status: domain::FlowRunStatus::Succeeded,
        input_payload: json!({}),
        output_payload: json!({}),
        error_payload: None,
        created_by: Uuid::now_v7(),
        authorized_account: Some("owner@example.com".to_string()),
        api_key_id: None,
        publication_version_id: None,
        external_user: None,
        external_conversation_id: None,
        external_trace_id: None,
        compatibility_mode: None,
        idempotency_key: None,
        started_at: now,
        finished_at: Some(now + time::Duration::seconds(3)),
        created_at: now,
        updated_at: now + time::Duration::seconds(3),
    }
}
