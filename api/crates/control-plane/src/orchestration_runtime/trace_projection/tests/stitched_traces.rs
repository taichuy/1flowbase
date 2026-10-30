use super::*;

#[test]
fn builder_projects_stitched_trace_as_collapsed_context_group() {
    let flow_run_id = Uuid::now_v7();
    let prior_run_id = Uuid::now_v7();
    let prior_node_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: flow_run(flow_run_id, now),
        node_runs: Vec::new(),
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
    };
    detail
        .stitched_trace
        .push(domain::ApplicationRunStitchedTrace {
            source_flow_run: domain::FlowRunRecord {
                id: prior_run_id,
                title: "prior run".to_string(),
                ..flow_run(prior_run_id, now - time::Duration::seconds(10))
            },
            node_runs: vec![domain::NodeRunRecord {
                execution_identity: None,
                id: prior_node_run_id,
                flow_run_id: prior_run_id,
                node_id: "prior-node".to_string(),
                node_type: "llm".to_string(),
                node_alias: "Prior LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: json!({}),
                output_payload: json!({}),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({}),
                started_at: now,
                finished_at: Some(now),
            }],
            callback_tasks: Vec::new(),
            events: Vec::new(),
            runtime_events: Vec::new(),
        });

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let stitched_group = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "stitched_context")
        .expect("stitched context group should be projected");
    let prior_run = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "stitched_run")
        .expect("prior run summary should be child of stitched group");
    let prior_node = projection
        .nodes
        .iter()
        .find(|node| {
            node.parent_trace_node_id == Some(prior_run.trace_node_id)
                && node.node_kind == "node_run"
                && node.node_id.as_deref() == Some("prior-node")
        })
        .expect("stitched run should project historical node children");
    let prior_node_content = projection
        .contents
        .iter()
        .find(|content| content.trace_node_id == prior_node.trace_node_id)
        .expect("historical node content should be projected");

    assert_eq!(stitched_group.parent_trace_node_id, None);
    assert_eq!(stitched_group.child_count, 1);
    assert_eq!(
        prior_run.parent_trace_node_id,
        Some(stitched_group.trace_node_id)
    );
    assert!(!prior_run.has_content);
    assert!(prior_run.has_children);
    assert_eq!(prior_run.child_count, 1);
    assert_eq!(prior_run.source_flow_run_id, Some(prior_run_id));
    assert_eq!(prior_node.source_flow_run_id, Some(prior_run_id));
    assert_eq!(
        prior_node_content.payload["payload_index"]["source_flow_run_id"],
        json!(prior_run_id)
    );
    assert!(projection
        .contents
        .iter()
        .all(|content| content.trace_node_id != prior_run.trace_node_id));
    assert!(projection
        .nodes
        .iter()
        .all(|node| node.stable_locator != format!("run:{flow_run_id}/node:prior-node")));
}

#[test]
fn builder_nests_stitched_trace_under_current_llm_root_as_tool_sibling() {
    let flow_run_id = Uuid::now_v7();
    let current_node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let prior_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: domain::FlowRunRecord {
            target_node_id: Some("node-llm".to_string()),
            ..flow_run(flow_run_id, now)
        },
        node_runs: vec![domain::NodeRunRecord {
            execution_identity: None,
            id: current_node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({}),
            output_payload: json!({}),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            started_at: now,
            finished_at: Some(now),
        }],
        checkpoints: Vec::new(),
        callback_tasks: vec![domain::CallbackTaskRecord {
            id: callback_task_id,
            flow_run_id,
            node_run_id: current_node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Completed,
            request_payload: json!({
                "tool_calls": [
                    {
                        "id": "call-read",
                        "name": "Read"
                    }
                ]
            }),
            response_payload: Some(json!({
                "tool_results": [
                    {
                        "tool_call_id": "call-read",
                        "content": "ok"
                    }
                ]
            })),
            external_ref_payload: None,
            created_at: now,
            completed_at: Some(now),
        }],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),

        task_rounds: Vec::new(),

        child_task_traces: Vec::new(),
    };
    detail
        .stitched_trace
        .push(domain::ApplicationRunStitchedTrace {
            source_flow_run: domain::FlowRunRecord {
                id: prior_run_id,
                title: "prior run".to_string(),
                ..flow_run(prior_run_id, now - time::Duration::seconds(10))
            },
            node_runs: vec![domain::NodeRunRecord {
                execution_identity: None,
                id: Uuid::now_v7(),
                flow_run_id: prior_run_id,
                node_id: "prior-node".to_string(),
                node_type: "llm".to_string(),
                node_alias: "Prior LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: json!({}),
                output_payload: json!({}),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({}),
                started_at: now,
                finished_at: Some(now),
            }],
            callback_tasks: Vec::new(),
            events: Vec::new(),
            runtime_events: Vec::new(),
        });

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let llm_node = projection
        .nodes
        .iter()
        .find(|node| {
            node.node_kind == "node_run"
                && node.node_id.as_deref() == Some("node-llm")
                && node.parent_trace_node_id.is_none()
        })
        .expect("current llm node should be projected");
    let stitched_group = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "stitched_context")
        .expect("stitched context group should be projected");
    let tools_group = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "tool_group")
        .expect("tools group should be projected");

    assert_eq!(
        stitched_group.parent_trace_node_id,
        Some(llm_node.trace_node_id)
    );
    assert_eq!(
        tools_group.parent_trace_node_id,
        Some(llm_node.trace_node_id)
    );
    assert!(
        stitched_group.order_key < tools_group.order_key,
        "stitched context should be ordered before tools under the llm root"
    );
    assert_eq!(
        projection
            .nodes
            .iter()
            .filter(|node| node.node_kind == "stitched_context")
            .count(),
        1
    );
}

#[test]
fn projection_status_needs_rebuild_for_missing_stale_or_changed_source() {
    let flow_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let fresh = status_record(
        flow_run_id,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        domain::ApplicationRunTraceProjectionStatus::Succeeded,
        "source:1",
        now,
    );
    let stale_version = status_record(
        flow_run_id,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION - 1,
        domain::ApplicationRunTraceProjectionStatus::Succeeded,
        "source:1",
        now,
    );
    let failed = status_record(
        flow_run_id,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        domain::ApplicationRunTraceProjectionStatus::Failed,
        "source:1",
        now,
    );

    assert!(projection_status_needs_lazy_rebuild(None, "source:1"));
    assert!(!projection_status_needs_lazy_rebuild(
        Some(&fresh),
        "source:1"
    ));
    assert!(projection_status_needs_lazy_rebuild(
        Some(&fresh),
        "source:2"
    ));
    assert!(projection_status_needs_lazy_rebuild(
        Some(&stale_version),
        "source:1"
    ));
    assert!(!projection_status_needs_lazy_rebuild(
        Some(&failed),
        "source:1"
    ));
}

// #2035 AC-004/005: the task anchor's LLM node owns a Rounds group for later
// member calls (with their client tool callbacks) and an Agents group for child tasks.
#[test]
fn issue_2035_builder_projects_task_rounds_and_child_tasks_under_llm_node() {
    let flow_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let llm_node_run_id = Uuid::now_v7();
    let llm = |id: Uuid, run_id: Uuid, offset: i64| domain::NodeRunRecord {
        execution_identity: None,
        id,
        flow_run_id: run_id,
        node_id: "node-llm".to_string(),
        node_type: "llm".to_string(),
        node_alias: "gpt".to_string(),
        status: domain::NodeRunStatus::Succeeded,
        input_payload: json!({}),
        output_payload: json!({}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        started_at: now + time::Duration::seconds(offset),
        finished_at: Some(now + time::Duration::seconds(offset + 1)),
    };
    let round_run_id = Uuid::now_v7();
    let compact_run_id = Uuid::now_v7();
    let child_run_id = Uuid::now_v7();
    let mut round_run = flow_run(round_run_id, now + time::Duration::seconds(10));
    round_run.application_id = Uuid::nil();
    let mut compact_run = flow_run(compact_run_id, now + time::Duration::seconds(20));
    compact_run.application_id = Uuid::nil();
    let mut child_run = flow_run(child_run_id, now + time::Duration::seconds(30));
    child_run.title = "review diff".to_string();
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![llm(llm_node_run_id, flow_run_id, 0)],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
        task_rounds: vec![
            domain::ApplicationRunTaskRoundTrace {
                call_kind: "generate".to_string(),
                source_flow_run: round_run,
                node_runs: vec![llm(Uuid::now_v7(), round_run_id, 10)],
                callback_tasks: Vec::new(),
                native_messages: vec![json!({
                    "role":"assistant","content":"exec",
                    "_source_item":{"type":"custom_tool_call","call_id":"call-1","name":"exec","input":"cat entry.txt"},
                    "tool_result":{"type":"custom_tool_call_output","call_id":"call-1","output":"FINAL=1"}
                })],
            },
            domain::ApplicationRunTaskRoundTrace {
                call_kind: "compact".to_string(),
                source_flow_run: compact_run,
                node_runs: vec![llm(Uuid::now_v7(), compact_run_id, 20)],
                callback_tasks: Vec::new(),
                native_messages: Vec::new(),
            },
        ],
        child_task_traces: vec![domain::ApplicationRunChildTaskTrace {
            subagent_kind: Some("review".to_string()),
            source_flow_run: child_run,
            node_runs: vec![llm(Uuid::now_v7(), child_run_id, 30)],
            callback_tasks: Vec::new(),
        }],
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let llm_root = projection
        .nodes
        .iter()
        .find(|node| {
            node.parent_trace_node_id.is_none() && node.node_type.as_deref() == Some("llm")
        })
        .expect("anchor llm root");
    let groups = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(llm_root.trace_node_id))
        .collect::<Vec<_>>();
    assert_eq!(
        groups
            .iter()
            .map(|node| node.node_kind.as_str())
            .collect::<Vec<_>>(),
        vec!["round_group", "agent_group"],
        "{groups:?}"
    );
    let rounds = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(groups[0].trace_node_id))
        .collect::<Vec<_>>();
    assert_eq!(rounds.len(), 2);
    assert_eq!(rounds[0].node_kind, "task_round");
    assert_eq!(rounds[0].source_flow_run_id, Some(round_run_id));
    assert_eq!(rounds[0].node_alias, "Round 2");
    assert_eq!(rounds[1].node_alias, "Compaction 3");
    assert_eq!(rounds[1].node_mode.as_deref(), Some("compact"));
    // The round's client tool callback lives under the round, not at the root.
    let tool = projection
        .nodes
        .iter()
        .find(|node| {
            node.node_kind == "tool_callback" && node.owner_id.as_deref() == Some("call-1")
        })
        .expect("round tool callback");
    let mut parent = tool.parent_trace_node_id;
    let mut reached_round = false;
    while let Some(id) = parent {
        let node = projection
            .nodes
            .iter()
            .find(|node| node.trace_node_id == id)
            .unwrap();
        if node.trace_node_id == rounds[0].trace_node_id {
            reached_round = true;
            break;
        }
        parent = node.parent_trace_node_id;
    }
    assert!(reached_round);
    assert_eq!(tool.status, "returned");
    let agents = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(groups[1].trace_node_id))
        .collect::<Vec<_>>();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].node_kind, "child_task");
    assert_eq!(agents[0].source_flow_run_id, Some(child_run_id));
    assert_eq!(agents[0].node_alias, "review · review diff");
    assert!(projection
        .nodes
        .iter()
        .all(|node| node.parent_trace_node_id.is_some()
            || node.trace_node_id == llm_root.trace_node_id));
}
