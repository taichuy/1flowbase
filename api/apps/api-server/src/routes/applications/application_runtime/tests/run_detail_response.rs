use super::*;

#[test]
fn run_detail_response_moves_waiting_prefix_answer_into_answer_snapshot() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let waiting_node_run_id = Uuid::now_v7();
    let virtual_answer_node_run_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        flow_run: test_flow_run_record(
            application.id,
            flow_run_id,
            domain::FlowRunStatus::WaitingCallback,
            serde_json::json!({ "answer": "LLM1 final\n----\n" }),
        ),
        node_runs: vec![
            domain::NodeRunRecord {
                id: waiting_node_run_id,
                flow_run_id,
                node_id: "node-llm-2".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM2".to_string(),
                status: domain::NodeRunStatus::WaitingCallback,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({ "tool_calls": [] }),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({}),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: None,
            },
            domain::NodeRunRecord {
                id: virtual_answer_node_run_id,
                flow_run_id,
                node_id: "node-answer".to_string(),
                node_type: "answer".to_string(),
                node_alias: "Answer".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({
                    "presentation": {
                        "kind": "answer",
                        "complete": false,
                        "materialized_from": "waiting_prefix"
                    }
                }),
                output_payload: serde_json::json!({
                    "answer": "LLM1 final\n----\n"
                }),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({
                    "answer_presentation": {
                        "partial": true,
                        "materialized_from": "waiting_prefix"
                    }
                }),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
        ],
        checkpoints: vec![domain::CheckpointRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            node_run_id: Some(waiting_node_run_id),
            status: "waiting_callback".to_string(),
            reason: "等待 callback 回填".to_string(),
            locator_payload: serde_json::json!({
                "node_id": "node-llm-2",
                "next_node_index": 2
            }),
            variable_snapshot: serde_json::json!({}),
            external_ref_payload: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }],
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let response = to_application_run_detail_response(&application, detail);

    assert_eq!(response.node_runs.len(), 1);
    assert_eq!(response.node_runs[0].node_id, "node-llm-2");
    let answer_snapshot = response
        .answer_snapshot
        .expect("waiting_prefix answer should become answer_snapshot");
    assert_eq!(answer_snapshot.text, "LLM1 final\n----\n");
    assert!(!answer_snapshot.complete);
    assert_eq!(answer_snapshot.materialized_from, "waiting_prefix");
    assert_eq!(answer_snapshot.answer_node_id, "node-answer");
    assert_eq!(
        answer_snapshot.answer_node_run_id,
        virtual_answer_node_run_id.to_string()
    );
    assert_eq!(
        answer_snapshot.waiting_node_id.as_deref(),
        Some("node-llm-2")
    );
    assert_eq!(
        answer_snapshot.waiting_node_run_id.as_deref(),
        Some(waiting_node_run_id.to_string().as_str())
    );
    assert!(response
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-answer"));
}

#[test]
fn run_detail_response_exposes_stitched_trace_sources() {
    let application = test_application_record();
    let current_flow_run_id = Uuid::now_v7();
    let source_flow_run_id = Uuid::now_v7();
    let source_node_run_id = Uuid::now_v7();
    let source_answer_node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        flow_run: test_flow_run_record(
            application.id,
            current_flow_run_id,
            domain::FlowRunStatus::Succeeded,
            serde_json::json!({ "answer": "done" }),
        ),
        node_runs: Vec::new(),
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: vec![domain::ApplicationRunStitchedTrace {
            source_flow_run: test_flow_run_record(
                application.id,
                source_flow_run_id,
                domain::FlowRunStatus::Cancelled,
                serde_json::json!({}),
            ),
            node_runs: vec![
                domain::NodeRunRecord {
                    id: source_node_run_id,
                    flow_run_id: source_flow_run_id,
                    node_id: "node-llm".to_string(),
                    node_type: "llm".to_string(),
                    node_alias: "LLM".to_string(),
                    status: domain::NodeRunStatus::Succeeded,
                    input_payload: serde_json::json!({}),
                    output_payload: serde_json::json!({ "usage": { "total_tokens": 33520 } }),
                    error_payload: None,
                    metrics_payload: serde_json::json!({}),
                    debug_payload: serde_json::json!({
                        "visible_internal_llm_tool_trace": [
                            {
                                "kind": "visible_internal_llm_tool_trace",
                                "tool_call_id": "call_image",
                                "tool_name": "image_llm",
                                "status": "succeeded"
                            }
                        ]
                    }),
                    started_at: OffsetDateTime::UNIX_EPOCH,
                    finished_at: Some(OffsetDateTime::UNIX_EPOCH),
                },
                domain::NodeRunRecord {
                    id: source_answer_node_run_id,
                    flow_run_id: source_flow_run_id,
                    node_id: "node-answer".to_string(),
                    node_type: "answer".to_string(),
                    node_alias: "Answer".to_string(),
                    status: domain::NodeRunStatus::Succeeded,
                    input_payload: serde_json::json!({
                        "presentation": {
                            "materialized_from": "waiting_prefix"
                        }
                    }),
                    output_payload: serde_json::json!({ "answer": "route prefix" }),
                    error_payload: None,
                    metrics_payload: serde_json::json!({}),
                    debug_payload: serde_json::json!({}),
                    started_at: OffsetDateTime::UNIX_EPOCH,
                    finished_at: Some(OffsetDateTime::UNIX_EPOCH),
                },
            ],
            callback_tasks: vec![domain::CallbackTaskRecord {
                id: callback_task_id,
                flow_run_id: source_flow_run_id,
                node_run_id: source_node_run_id,
                callback_kind: "llm_tool_calls".to_string(),
                status: domain::CallbackTaskStatus::Completed,
                request_payload: serde_json::json!({
                    "tool_calls": [
                        { "id": "call_image", "name": "image_llm" }
                    ]
                }),
                response_payload: None,
                external_ref_payload: None,
                created_at: OffsetDateTime::UNIX_EPOCH,
                completed_at: Some(OffsetDateTime::UNIX_EPOCH),
            }],
            events: Vec::new(),
            runtime_events: Vec::new(),
        }],
        subagent_traces: Vec::new(),
    };

    let response = to_application_run_detail_response(&application, detail);

    assert_eq!(response.callback_tasks.len(), 0);
    assert_eq!(response.stitched_trace.len(), 1);
    assert_eq!(
        response.stitched_trace[0].source_flow_run.id,
        source_flow_run_id.to_string()
    );
    assert_eq!(
        response.stitched_trace[0].node_runs[0].id,
        source_node_run_id.to_string()
    );
    assert_eq!(response.stitched_trace[0].node_runs.len(), 1);
    assert!(response.stitched_trace[0]
        .node_runs
        .iter()
        .all(|node_run| node_run.node_type != "answer"));
    assert_eq!(
        response.stitched_trace[0].callback_tasks[0].id,
        callback_task_id.to_string()
    );
    assert_eq!(
        response.detail.stitched_trace[0].callback_tasks[0].flow_run_id,
        source_flow_run_id.to_string()
    );
}

#[test]
fn visible_internal_llm_route_trace_uses_precise_node_run_id_before_reused_node_id() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let routed_node_run_id = Uuid::now_v7();
    let later_node_run_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        flow_run: test_flow_run_record(
            application.id,
            flow_run_id,
            domain::FlowRunStatus::Succeeded,
            serde_json::json!({ "answer": "done" }),
        ),
        node_runs: vec![
            domain::NodeRunRecord {
                id: routed_node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({}),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({
                    "callback_kind": "llm_tool_calls",
                    "callback_task_id": Uuid::now_v7().to_string()
                }),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
            domain::NodeRunRecord {
                id: later_node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({}),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({
                    "llm_rounds": [
                        {
                            "round_index": 0,
                            "assistant": {
                                "role": "assistant",
                                "content": "later continuation"
                            }
                        }
                    ]
                }),
                started_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1),
                finished_at: Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(1)),
            },
        ],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };
    let runtime_events = vec![test_runtime_event_record(
        flow_run_id,
        Some(routed_node_run_id),
        "visible_internal_llm_tool_completed",
        serde_json::json!({
            "main_node_id": "node-llm",
            "target_node_id": "node-llm-1",
            "tool_name": "image_llm",
            "tool_call_id": "call_image",
            "node_run_id": routed_node_run_id.to_string(),
            "provider_route": {
                "model": "mimo-v2.5",
                "provider_code": "anthropic"
            }
        }),
    )];

    let detail =
        enrich_application_run_detail_visible_internal_llm_route_traces(detail, &runtime_events);

    let routed_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.id == routed_node_run_id)
        .expect("routed node run should stay visible");
    assert_eq!(
        routed_node.debug_payload["visible_internal_llm_tool_trace"][0]["tool_name"],
        serde_json::json!("image_llm")
    );
    assert_eq!(
        routed_node.debug_payload["visible_internal_llm_tool_trace"][0]["route_model"],
        serde_json::json!("mimo-v2.5")
    );
    assert_eq!(
        routed_node.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["id"],
        serde_json::json!("call_image")
    );
    assert_eq!(
        routed_node.debug_payload["llm_rounds"][0]["assistant"]["tool_calls"][0]["name"],
        serde_json::json!("image_llm")
    );
    let later_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.id == later_node_run_id)
        .expect("later node run should stay visible");
    assert!(later_node
        .debug_payload
        .get("visible_internal_llm_tool_trace")
        .is_none());
}

#[test]
fn visible_internal_llm_fusion_branch_trace_uses_branch_node_run_payloads() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let main_node_run_id = Uuid::now_v7();
    let branch_node_run_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        flow_run: test_flow_run_record(
            application.id,
            flow_run_id,
            domain::FlowRunStatus::Succeeded,
            serde_json::json!({ "answer": "done" }),
        ),
        node_runs: vec![
            domain::NodeRunRecord {
                id: main_node_run_id,
                flow_run_id,
                node_id: "node-main-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({}),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({
                    "llm_rounds": [
                        {
                            "round_index": 0,
                            "assistant": {
                                "role": "assistant",
                                "tool_calls": [
                                    {
                                        "id": "call_fusion",
                                        "name": "fusion_review"
                                    }
                                ]
                            }
                        }
                    ]
                }),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
            domain::NodeRunRecord {
                id: branch_node_run_id,
                flow_run_id,
                node_id: "node-panel-a".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM2".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({
                    "prompt_messages": [
                        {
                            "role": "user",
                            "content": "review refund policy"
                        }
                    ],
                    "model": "risk-v1"
                }),
                output_payload: serde_json::json!({
                    "text": "panel A says strict",
                    "provider_route": {
                        "model": "risk-v1"
                    }
                }),
                error_payload: None,
                metrics_payload: serde_json::json!({
                    "usage": {
                        "total_tokens": 42
                    }
                }),
                debug_payload: serde_json::json!({
                    "llm_rounds": [
                        {
                            "round_index": 0,
                            "assistant": {
                                "content": "risk result"
                            }
                        }
                    ],
                    "provider_debug": "branch debug detail"
                }),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
        ],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };
    let runtime_events = vec![test_runtime_event_record(
        flow_run_id,
        Some(main_node_run_id),
        "visible_internal_llm_tool_completed",
        serde_json::json!({
            "event_type": "visible_internal_llm_tool_completed",
            "main_node_id": "node-main-llm",
            "target_node_id": "node-panel-a",
            "tool_name": "fusion_review",
            "tool_call_id": "call_fusion",
            "tool_mode": "fusion",
            "execution_mode": "bounded_parallel_panel",
            "node_id": "node-panel-a",
            "node_alias": "LLM2",
            "node_type": "llm",
            "provider_route": {
                "model": "risk-v1"
            },
            "content": "panel A says strict"
        }),
    )];

    let detail =
        enrich_application_run_detail_visible_internal_llm_route_traces(detail, &runtime_events);

    let main_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.id == main_node_run_id)
        .expect("main node run should stay visible");
    let branch_trace =
        &main_node.debug_payload["visible_internal_llm_tool_trace"][0]["branch_traces"][0];
    assert_eq!(
        branch_trace["input_payload"]["prompt_messages"][0]["content"],
        serde_json::json!("review refund policy")
    );
    assert_eq!(
        branch_trace["debug_payload"]["provider_debug"],
        serde_json::json!("branch debug detail")
    );
    assert_eq!(
        branch_trace["output_payload"]["text"],
        serde_json::json!("panel A says strict")
    );
    assert_eq!(
        branch_trace["metrics_payload"]["usage"]["total_tokens"],
        serde_json::json!(42)
    );
}

#[test]
fn run_detail_response_hides_historical_waiting_prefix_after_run_finishes() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let waiting_node_run_id = Uuid::now_v7();
    let virtual_answer_node_run_id = Uuid::now_v7();
    let final_answer_node_run_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        flow_run: test_flow_run_record(
            application.id,
            flow_run_id,
            domain::FlowRunStatus::Succeeded,
            serde_json::json!({ "answer": "final answer" }),
        ),
        node_runs: vec![
            domain::NodeRunRecord {
                id: waiting_node_run_id,
                flow_run_id,
                node_id: "node-llm-2".to_string(),
                node_type: "llm".to_string(),
                node_alias: "LLM2".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({ "text": "final answer" }),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({}),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
            domain::NodeRunRecord {
                id: virtual_answer_node_run_id,
                flow_run_id,
                node_id: "node-answer".to_string(),
                node_type: "answer".to_string(),
                node_alias: "Answer".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({
                    "presentation": {
                        "kind": "answer",
                        "complete": false,
                        "materialized_from": "waiting_prefix"
                    }
                }),
                output_payload: serde_json::json!({
                    "answer": "prefix answer"
                }),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({
                    "answer_presentation": {
                        "partial": true,
                        "materialized_from": "waiting_prefix"
                    }
                }),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
            domain::NodeRunRecord {
                id: final_answer_node_run_id,
                flow_run_id,
                node_id: "node-answer".to_string(),
                node_type: "answer".to_string(),
                node_alias: "Answer".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: serde_json::json!({}),
                output_payload: serde_json::json!({
                    "answer": "final answer"
                }),
                error_payload: None,
                metrics_payload: serde_json::json!({}),
                debug_payload: serde_json::json!({}),
                started_at: OffsetDateTime::UNIX_EPOCH,
                finished_at: Some(OffsetDateTime::UNIX_EPOCH),
            },
        ],
        checkpoints: vec![domain::CheckpointRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            node_run_id: Some(waiting_node_run_id),
            status: "waiting_callback".to_string(),
            reason: "历史等待点".to_string(),
            locator_payload: serde_json::json!({
                "node_id": "node-llm-2"
            }),
            variable_snapshot: serde_json::json!({}),
            external_ref_payload: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }],
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let response = to_application_run_detail_response(&application, detail);

    assert!(response.answer_snapshot.is_none());
    assert!(response
        .node_runs
        .iter()
        .all(|node_run| node_run.id != virtual_answer_node_run_id.to_string()));
    assert!(response
        .node_runs
        .iter()
        .any(|node_run| node_run.id == final_answer_node_run_id.to_string()));
}
