use super::*;

#[test]
fn builder_projects_artifact_indexed_host_internal_tool_lifecycle() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let started_at = now + time::Duration::seconds(1);
    let finished_at = started_at + time::Duration::milliseconds(37);
    let detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({"prompt": "inspect tools"}),
            output_payload: json!({"answer": "done"}),
            error_payload: None,
            metrics_payload: json!({"internal_tool_call_count": 1}),
            debug_payload: json!({
                "llm_rounds": {
                    "artifact_ref": Uuid::now_v7(),
                    "tool_callbacks": [{
                        "id": "call_catalog",
                        "name": "catalog_mcp_call",
                        "artifact_ref": Uuid::now_v7(),
                        "callback_status": "returned",
                        "execution_status": "unknown",
                        "call_usage": {"total_tokens": 14}
                    }]
                },
                "runtime_internal_tool_events": [{
                    "event_type": "mcp_runtime_tool_call_completed",
                    "tool_call_id": "call_catalog",
                    "registration_id": "catalog|call|catalog_mcp_call",
                    "provider_name": "catalog_mcp_call",
                    "owner": {
                        "kind": "mcp_instance",
                        "instance_id": "catalog",
                        "operation": "call",
                        "source": {"kind": "run", "key": "0"}
                    },
                    "execution_kind": "host_internal",
                    "callback_status": "returned",
                    "execution_status": "succeeded",
                    "is_error": false,
                    "started_at": started_at,
                    "finished_at": finished_at,
                    "duration_ms": 37
                }]
            }),
            started_at: now,
            finished_at: Some(now + time::Duration::seconds(2)),
        }],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let tool = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "tool_callback")
        .expect("host-internal call should use the canonical tool callback projection");

    assert_eq!(tool.node_alias, "catalog_mcp_call");
    assert_eq!(tool.node_mode.as_deref(), Some("host_internal"));
    assert_eq!(
        tool.owner_kind.as_deref(),
        Some("runtime_internal_tool_call")
    );
    assert_eq!(
        tool.owner_id.as_deref(),
        Some("catalog|call|catalog_mcp_call")
    );
    assert_eq!(tool.status, "succeeded");
    assert_eq!(tool.started_at, started_at);
    assert_eq!(tool.finished_at, Some(finished_at));
    assert_eq!(tool.duration_ms, Some(37));
    assert_eq!(tool.metrics_payload["usage"]["total_tokens"], json!(14));

    let content = projection
        .contents
        .iter()
        .find(|content| content.trace_node_id == tool.trace_node_id)
        .expect("tool callback content should be projected");
    assert_eq!(
        content.payload["tool_result"]["owner"]["instance_id"],
        json!("catalog")
    );
    assert_eq!(
        content.payload["tool_result"]["execution_kind"],
        json!("host_internal")
    );
    assert!(content.payload["tool_result"]["artifact_ref"].is_string());
}
