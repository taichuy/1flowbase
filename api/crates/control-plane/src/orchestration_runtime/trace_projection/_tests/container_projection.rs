use super::*;

#[test]
fn container_projection_preserves_nested_rounds_repeated_receipts_and_live_failure_context() {
    let run = Uuid::now_v7();
    let outer = Uuid::now_v7();
    let nested = Uuid::now_v7();
    let make = |execution_id,
                node_id: &str,
                node_type: &str,
                path: Vec<domain::ContainerRoundPath>,
                offset| domain::NodeRunRecord {
        execution_identity: Some(domain::NodeExecutionIdentity {
            execution_id,
            node_id: node_id.into(),
            parent_execution_id: path.last().map(|p| p.container_execution_id),
            round_path: path,
        }),
        id: Uuid::now_v7(),
        flow_run_id: run,
        node_id: node_id.into(),
        node_type: node_type.into(),
        node_alias: node_id.into(),
        status: domain::NodeRunStatus::Running,
        input_payload: json!({"application_id":"codex", "job_id":"cleanup"}),
        output_payload: json!({}),
        error_payload: None,
        metrics_payload: json!({}),
        debug_payload: json!({}),
        started_at: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(offset),
        finished_at: None,
    };
    let path = |id, node: &str, round_index| domain::ContainerRoundPath {
        container_execution_id: id,
        container_node_id: node.into(),
        round_index,
    };
    let mut failed = make(
        Uuid::now_v7(),
        "submit",
        "http_request",
        vec![path(outer, "iterate", 1), path(nested, "loop", 0)],
        4,
    );
    failed.status = domain::NodeRunStatus::Failed;
    failed.finished_at = Some(failed.started_at + time::Duration::seconds(30));
    failed.error_payload = Some(json!({"message":"Cleanup request timed out"}));
    let mut detail = domain::ApplicationRunDetail {
        flow_run: flow_run(run, OffsetDateTime::UNIX_EPOCH),
        native_messages: vec![],
        node_runs: vec![
            make(outer, "iterate", "iteration", vec![], 0),
            make(
                Uuid::now_v7(),
                "submit",
                "http_request",
                vec![path(outer, "iterate", 0)],
                1,
            ),
            make(nested, "loop", "loop", vec![path(outer, "iterate", 1)], 3),
            failed.clone(),
        ],
        checkpoints: vec![],
        callback_tasks: vec![],
        events: vec![],
        stitched_trace: vec![],
        subagent_traces: vec![],
        task_rounds: vec![],
        child_task_traces: vec![],
    };
    detail.flow_run.status = domain::FlowRunStatus::Running;
    detail.flow_run.finished_at = None;
    let projection = build_application_run_trace_projection(&detail).unwrap();
    let node = |receipt: Uuid| {
        projection
            .nodes
            .iter()
            .find(|n| n.owner_id.as_deref() == Some(receipt.to_string().as_str()))
            .unwrap()
    };
    let outer_node = node(detail.node_runs[0].id);
    let repeated = node(detail.node_runs[1].id);
    let nested_node = node(detail.node_runs[2].id);
    let failed_node = node(failed.id);
    let group = |container, round| {
        projection
            .nodes
            .iter()
            .find(|n| {
                n.owner_id.as_deref() == Some(format!("{container}:{round}").as_str())
                    && n.owner_kind.as_deref() == Some("container_round")
            })
            .unwrap()
    };
    assert_eq!(
        group(outer, 0).parent_trace_node_id,
        Some(outer_node.trace_node_id)
    );
    assert_eq!(
        repeated.parent_trace_node_id,
        Some(group(outer, 0).trace_node_id)
    );
    assert_eq!(
        nested_node.parent_trace_node_id,
        Some(group(outer, 1).trace_node_id)
    );
    assert_eq!(
        group(nested, 0).parent_trace_node_id,
        Some(nested_node.trace_node_id)
    );
    assert_eq!(
        failed_node.parent_trace_node_id,
        Some(group(nested, 0).trace_node_id)
    );
    assert_ne!(repeated.trace_node_id, failed_node.trace_node_id);
    assert!(group(outer, 0).order_key < group(outer, 1).order_key);
    assert_eq!(group(nested, 0).status, "failed");
    assert_eq!(
        projection
            .contents
            .iter()
            .find(|c| c.trace_node_id == failed_node.trace_node_id)
            .unwrap()
            .payload["error_payload"],
        failed.error_payload.unwrap()
    );
    if let Ok(path) = std::env::var("ONEFLOWBASE_WORKFLOW_OBSERVATION_FIXTURE") {
        let summaries = projection.nodes.iter().map(|node| json!({
            "trace_node_id": node.trace_node_id, "parent_trace_node_id": node.parent_trace_node_id,
            "node_kind": node.node_kind, "flow_run_id": run,
            "node_run_id": if node.owner_kind.as_deref() == Some("node_run") { node.owner_id.clone() } else { None },
            "node_id": node.node_id, "node_type": node.node_type, "node_alias": node.node_alias,
            "status": node.status, "started_at": node.started_at.format(&time::format_description::well_known::Rfc3339).unwrap(),
            "finished_at": node.finished_at.map(|at| at.format(&time::format_description::well_known::Rfc3339).unwrap()),
            "duration_ms": node.duration_ms, "has_children": node.has_children, "has_content": node.has_content,
            "child_count": node.child_count, "metrics_payload": node.metrics_payload,
        })).collect::<Vec<_>>();
        let contents = projection.contents.iter().map(|content| json!({"trace_node_id":content.trace_node_id,"node_kind":"node_run","payload":content.payload})).collect::<Vec<_>>();
        let records = detail
            .node_runs
            .iter()
            .map(|node| {
                let mut value = serde_json::to_value(node).unwrap();
                value["started_at"] = json!(node
                    .started_at
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap());
                value["finished_at"] = json!(node.finished_at.map(|at| at
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap()));
                value
            })
            .collect::<Vec<_>>();
        std::fs::write(
            path,
            serde_json::to_vec(&json!({"nodes":summaries,"contents":contents,"records":records}))
                .unwrap(),
        )
        .unwrap();
    }
}
