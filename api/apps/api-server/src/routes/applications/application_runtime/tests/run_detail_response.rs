use super::*;
use interface_runtime::{InterfaceContract, ManagedInterfaceProjection};

use super::super::{
    interface_debug_artifacts::ApplicationRuntimeDebugArtifactsOutput as ArtifactsOutput,
    interface_debug_commands::ApplicationRuntimeDebugCommandsOutput as CommandsOutput,
};

#[test]
fn context_snapshot_response_projects_current_run_runtime_event_for_recovery() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let context_snapshot = test_runtime_event_record(
        flow_run_id,
        Some(node_run_id),
        "context_snapshot",
        serde_json::json!({
            "type": "context_snapshot",
            "node_id": "node-llm",
            "node_run_id": node_run_id,
            "input_tokens": 13_681,
            "effective_context_window": 128_000,
            "remaining_tokens": 114_319,
            "measurement": {
                "method": "generic_estimate",
                "accuracy": "estimated",
                "coverage": "complete",
                "unknown_block_count": 0
            }
        }),
    );

    let snapshot = to_context_snapshot_response(&[context_snapshot])
        .expect("terminal snapshot should project the latest Gateway context");
    assert_eq!(snapshot.run_id, flow_run_id.to_string());
    assert_eq!(snapshot.node_id, "node-llm");
    assert_eq!(snapshot.node_run_id, Some(node_run_id.to_string()));
    assert_eq!(snapshot.input_tokens, 13_681);
    assert_eq!(snapshot.effective_context_window, Some(128_000));
    assert_eq!(snapshot.remaining_tokens, Some(114_319));
    assert_eq!(snapshot.measurement.accuracy, "estimated");
}

#[test]
fn run_detail_response_does_not_expose_publication_creator_as_scheduler_principal() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let mut flow_run = test_flow_run_record(
        application.id,
        flow_run_id,
        domain::FlowRunStatus::Succeeded,
        serde_json::json!({}),
    );
    flow_run.run_mode = domain::FlowRunMode::WorkflowScheduleRun;
    flow_run.authorized_account = Some("publication creator".to_string());
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run,
        node_runs: Vec::new(),
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
    };

    let response = to_application_run_detail_response(&application, detail);

    assert_eq!(response.run.execution_stage, "published");
    assert_eq!(response.run.invocation_source, "workflow_schedule");
    assert_eq!(response.run.principal.kind, "scheduler");
    assert_eq!(response.run.principal.id, None);
    assert_eq!(response.run.principal.display_name, None);
}

#[test]
fn run_detail_response_moves_waiting_prefix_answer_into_answer_snapshot() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let waiting_node_run_id = Uuid::now_v7();
    let virtual_answer_node_run_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
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

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
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
    assert_eq!(
        answer_snapshot.answer_node_id.as_deref(),
        Some("node-answer")
    );
    assert_eq!(
        answer_snapshot.answer_node_run_id.as_deref(),
        Some(virtual_answer_node_run_id.to_string().as_str())
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
fn ac_004_answer_node_truth_waiting_snapshot_uses_flow_run_output_without_synthetic_node_run() {
    let application = test_application_record();
    let flow_run_id = Uuid::now_v7();
    let waiting_node_run_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: test_flow_run_record(
            application.id,
            flow_run_id,
            domain::FlowRunStatus::WaitingCallback,
            serde_json::json!({ "answer": "checking policy" }),
        ),
        node_runs: vec![domain::NodeRunRecord {
            id: waiting_node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: serde_json::json!({}),
            output_payload: serde_json::json!({ "tool_calls": [] }),
            error_payload: None,
            metrics_payload: serde_json::json!({}),
            debug_payload: serde_json::json!({}),
            started_at: OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
        }],
        checkpoints: vec![domain::CheckpointRecord {
            id: Uuid::now_v7(),
            flow_run_id,
            node_run_id: Some(waiting_node_run_id),
            status: "waiting_callback".to_string(),
            reason: "等待 callback 回填".to_string(),
            locator_payload: serde_json::json!({
                "node_id": "node-llm",
                "next_node_index": 1
            }),
            variable_snapshot: serde_json::json!({}),
            external_ref_payload: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        }],
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
    };

    let response = to_application_run_detail_response(&application, detail);

    assert_eq!(response.node_runs.len(), 1);
    assert_eq!(response.node_runs[0].node_type, "llm");
    let snapshot = response
        .answer_snapshot
        .expect("waiting flow output should remain available as an Answer snapshot");
    assert_eq!(snapshot.text, "checking policy");
    assert_eq!(snapshot.materialized_from, "flow_run_output");
    assert_eq!(snapshot.answer_node_id, None);
    assert_eq!(snapshot.answer_node_run_id, None);
    assert_eq!(snapshot.waiting_node_id.as_deref(), Some("node-llm"));
    assert_eq!(
        snapshot.waiting_node_run_id.as_deref(),
        Some(waiting_node_run_id.to_string().as_str())
    );
}

fn stitched_run_detail_response_fixture() -> (ApplicationRunDetailResponse, Uuid, Uuid, Uuid) {
    let application = test_application_record();
    let current_flow_run_id = Uuid::now_v7();
    let source_flow_run_id = Uuid::now_v7();
    let source_node_run_id = Uuid::now_v7();
    let source_answer_node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
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

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
    };

    let response = to_application_run_detail_response(&application, detail);

    (
        response,
        source_flow_run_id,
        source_node_run_id,
        callback_task_id,
    )
}

#[test]
fn run_detail_response_exposes_stitched_trace_sources() {
    let (response, source_flow_run_id, source_node_run_id, callback_task_id) =
        stitched_run_detail_response_fixture();
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
        native_messages: Vec::new(),
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

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
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
        native_messages: Vec::new(),
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

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
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
        native_messages: Vec::new(),
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

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
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

fn managed_detail_projections(
    detail: &ApplicationRunDetailResponse,
) -> [ManagedInterfaceProjection; 2] {
    let projections = [
        ManagedInterfaceProjection::from_contract(&ArtifactsOutput::Snapshot(detail.clone()))
            .unwrap(),
        ManagedInterfaceProjection::from_contract(&CommandsOutput::Run(detail.clone())).unwrap(),
    ];
    for projection in &projections {
        extension_contracts::ManagedProjectionContract {
            contract_id: projection.contract().contract_id().to_owned(),
            contract_version: projection.contract().version().to_owned(),
            schema: projection.schema().clone(),
        }
        .compile_for_registration()
        .expect("run detail retains a valid closed observation schema")
        .validate(projection.value())
        .expect("actual interface output conforms to its schema");
    }
    projections
}

fn assert_managed_detail_rejected(detail: &ApplicationRunDetailResponse) {
    assert!(ArtifactsOutput::Snapshot(detail.clone())
        .project_for_managed_hook()
        .is_none());
    assert!(CommandsOutput::Run(detail.clone())
        .project_for_managed_hook()
        .is_none());
}

#[test]
fn managed_run_detail_preserves_variant_identity_and_private_nested_content() {
    const SECRET: &str = "R2_PRIVATE_PAYLOAD";
    let (mut detail, _, _, _) = stitched_run_detail_response_fixture();
    detail.node_runs = detail.stitched_trace[0].node_runs.clone();
    detail.callback_tasks = detail.stitched_trace[0].callback_tasks.clone();
    detail.run.title = SECRET.into();
    detail.run.principal.display_name = Some(SECRET.into());
    for flow_run in [
        &mut detail.flow_run,
        &mut detail.detail.flow_run,
        &mut detail.stitched_trace[0].source_flow_run,
    ] {
        flow_run.query = Some(SECRET.into());
        flow_run.input_payload = serde_json::json!({SECRET: [SECRET]});
        flow_run.error_payload = None;
    }
    detail.node_runs[0].input_payload_view = serde_json::json!({SECRET: SECRET});
    detail.node_runs[0].debug_payload = serde_json::json!([SECRET]);
    detail.callback_tasks[0].request_payload = serde_json::json!({SECRET: SECRET});
    detail.context_snapshot = to_context_snapshot_response(&[test_runtime_event_record(
        Uuid::now_v7(),
        None,
        "context_snapshot",
        serde_json::json!({
            "type": "context_snapshot", "node_id": "node-llm",
            "input_tokens": 17, "effective_context_window": 128_000,
            "remaining_tokens": 127_983,
            "measurement": {"method": "generic_estimate", "accuracy": "estimated",
                "coverage": "complete", "unknown_block_count": 0}
        }),
    )]);
    assert!(detail.context_snapshot.is_some());

    let [snapshot, run] = managed_detail_projections(&detail);
    assert_eq!(
        snapshot.contract().contract_id(),
        "console-application-runtime-debug-artifacts-output"
    );
    assert_eq!(
        run.contract().contract_id(),
        "console-application-runtime-debug-commands-output"
    );
    assert_eq!(snapshot.contract().version(), "1");
    assert_eq!(run.contract().version(), "1");
    assert_eq!(snapshot.value()["variant"], "Snapshot");
    assert_eq!(run.value()["variant"], "Run");
    assert_eq!(snapshot.value()["0"], run.value()["0"]);
    let branch_schema = |projection: &ManagedInterfaceProjection, tag: &str| {
        projection.schema()["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|branch| branch["properties"]["variant"]["const"] == tag)
            .unwrap()["properties"]["0"]
            .clone()
    };
    assert_eq!(
        branch_schema(&snapshot, "Snapshot"),
        branch_schema(&run, "Run")
    );

    let value = &snapshot.value()["0"];
    for path in [
        "/flow_run",
        "/detail/flow_run",
        "/stitched_trace/0/source_flow_run",
    ] {
        let flow = value.pointer(path).unwrap();
        assert_eq!(
            flow["query"],
            serde_json::json!({"byte_count": SECRET.len()})
        );
        assert_eq!(
            flow["input_payload"],
            serde_json::json!({"kind": "object", "item_count": 1, "byte_count": 0})
        );
        assert_eq!(flow["error_payload"], serde_json::Value::Null);
    }
    assert_eq!(
        value["node_runs"][0]["debug_payload"],
        serde_json::json!({"kind": "array", "item_count": 1, "byte_count": 0})
    );
    assert_eq!(
        value["callback_tasks"][0]["response_payload"],
        serde_json::Value::Null
    );
    assert_eq!(
        value["stitched_trace"][0]["node_runs"],
        serde_json::json!({"item_count": 1})
    );
    assert_eq!(
        value["stitched_trace"][0]["callback_tasks"],
        serde_json::json!({"item_count": 1})
    );
    assert_eq!(
        value["detail"]["stitched_trace"],
        serde_json::json!({"item_count": 1})
    );
    assert_eq!(
        value["context_snapshot"]["effective_context_window"],
        128_000
    );
    assert!(value["context_snapshot"].get("input_tokens").is_none());
    assert!(value["context_snapshot"].get("remaining_tokens").is_none());
    assert!(!snapshot.value().to_string().contains(SECRET));
    assert!(!run.value().to_string().contains(SECRET));
}

#[test]
fn managed_run_detail_preserves_observed_list_bounds_and_count_only_lists() {
    let (detail, _, _, _) = stitched_run_detail_response_fixture();
    macro_rules! assert_list_bound {
        ($field:ident, $item:expr) => {{
            let mut bounded = detail.clone();
            bounded.$field = vec![$item; 32];
            for projection in managed_detail_projections(&bounded) {
                assert_eq!(
                    projection.value()["0"][stringify!($field)]
                        .as_array()
                        .unwrap()
                        .len(),
                    32
                );
            }
            bounded.$field.push(bounded.$field[0].clone());
            assert_managed_detail_rejected(&bounded);
        }};
    }
    assert_list_bound!(node_runs, detail.stitched_trace[0].node_runs[0].clone());
    assert_list_bound!(
        callback_tasks,
        detail.stitched_trace[0].callback_tasks[0].clone()
    );
    assert_list_bound!(stitched_trace, detail.stitched_trace[0].clone());
    assert_list_bound!(
        events,
        RunEventResponse {
            id: "event".into(),
            flow_run_id: detail.flow_run.id.clone(),
            node_run_id: None,
            sequence: 1,
            event_type: "started".into(),
            payload: serde_json::json!({}),
            created_at: "1970-01-01T00:00:00Z".into(),
        }
    );
    assert_list_bound!(
        checkpoints,
        CheckpointResponse {
            id: "checkpoint".into(),
            flow_run_id: detail.flow_run.id.clone(),
            node_run_id: None,
            status: "waiting".into(),
            reason: "private checkpoint reason".into(),
            locator_payload: serde_json::json!({}),
            variable_snapshot: serde_json::json!({}),
            external_ref_payload: None,
            created_at: "1970-01-01T00:00:00Z".into(),
        }
    );

    let mut count_only = detail;
    let node = count_only.stitched_trace[0].node_runs[0].clone();
    count_only.detail.node_runs = vec![node.clone(); 33];
    count_only.stitched_trace[0].node_runs = vec![node; 33];
    for projection in managed_detail_projections(&count_only) {
        assert_eq!(
            projection.value()["0"]["detail"]["node_runs"],
            serde_json::json!({"item_count": 33})
        );
        assert_eq!(
            projection.value()["0"]["stitched_trace"][0]["node_runs"],
            serde_json::json!({"item_count": 33})
        );
        assert_eq!(
            projection.value()["0"]["context_snapshot"],
            serde_json::Value::Null
        );
    }
}

#[test]
fn managed_run_detail_propagates_nested_text_byte_limits_to_both_interfaces() {
    let (detail, _, _, _) = stitched_run_detail_response_fixture();
    let changes: [fn(&mut ApplicationRunDetailResponse, String); 4] = [
        |detail, value| detail.run.id = value,
        |detail, value| detail.flow_run.id = value,
        |detail, value| detail.detail.flow_run.id = value,
        |detail, value| detail.stitched_trace[0].source_flow_run.id = value,
    ];
    for change in changes {
        let mut bounded = detail.clone();
        change(&mut bounded, "é".repeat(128));
        managed_detail_projections(&bounded);
        change(&mut bounded, "x".repeat(257));
        assert_managed_detail_rejected(&bounded);
    }
}
