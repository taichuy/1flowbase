use super::*;

#[test]
fn builder_projects_linked_agent_tools_as_subagent_llm_nodes() {
    let flow_run_id = Uuid::now_v7();
    let parent_node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let subagent_run_id = Uuid::now_v7();
    let subagent_llm_node_run_id = Uuid::now_v7();
    let subagent_callback_task_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: parent_node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Parent LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({ "prompt": "coordinate work" }),
            output_payload: json!({ "answer": "done" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            started_at: now,
            finished_at: Some(now + time::Duration::seconds(10)),
        }],
        checkpoints: Vec::new(),
        callback_tasks: vec![domain::CallbackTaskRecord {
            id: callback_task_id,
            flow_run_id,
            node_run_id: parent_node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Completed,
            request_payload: json!({
                "tool_calls": [
                    { "id": "call-read", "name": "Read" },
                    {
                        "id": "call-agent",
                        "name": "Agent",
                        "arguments": {
                            "description": "Backend worker",
                            "prompt": "Long implementation prompt"
                        }
                    }
                ]
            }),
            response_payload: Some(json!({
                "tool_results": [
                    { "tool_call_id": "call-read", "content": "read complete" },
                    { "tool_call_id": "call-agent", "content": "agent complete" }
                ]
            })),
            external_ref_payload: None,
            created_at: now + time::Duration::seconds(1),
            completed_at: Some(now + time::Duration::seconds(9)),
        }],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: vec![domain::ApplicationRunSubagentTrace {
            parent_tool_call_id: "call-agent".to_string(),
            parent_callback_task_id: callback_task_id,
            source_flow_run: domain::FlowRunRecord {
                id: subagent_run_id,
                title: "Long implementation prompt".to_string(),
                status: domain::FlowRunStatus::Succeeded,
                started_at: now + time::Duration::seconds(2),
                finished_at: Some(now + time::Duration::seconds(8)),
                ..flow_run(subagent_run_id, now + time::Duration::seconds(2))
            },
            node_runs: vec![domain::NodeRunRecord {
                id: subagent_llm_node_run_id,
                flow_run_id: subagent_run_id,
                node_id: "node-subagent-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "4.6".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: json!({ "prompt": "fix backend error body" }),
                output_payload: json!({ "answer": "patched" }),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({}),
                started_at: now + time::Duration::seconds(3),
                finished_at: Some(now + time::Duration::seconds(7)),
            }],
            callback_tasks: vec![domain::CallbackTaskRecord {
                id: subagent_callback_task_id,
                flow_run_id: subagent_run_id,
                node_run_id: subagent_llm_node_run_id,
                callback_kind: "llm_tool_calls".to_string(),
                status: domain::CallbackTaskStatus::Completed,
                request_payload: json!({
                    "tool_calls": [
                        { "id": "sub-call-bash", "name": "Bash" }
                    ]
                }),
                response_payload: Some(json!({
                    "tool_results": [
                        { "tool_call_id": "sub-call-bash", "content": "cargo test passed" }
                    ]
                })),
                external_ref_payload: None,
                created_at: now + time::Duration::seconds(4),
                completed_at: Some(now + time::Duration::seconds(5)),
            }],
            events: Vec::new(),
            runtime_events: Vec::new(),
        }],
        task_rounds: Vec::new(),
        child_task_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let parent = projection
        .nodes
        .iter()
        .find(|node| node.stable_locator == format!("run:{flow_run_id}/node:{parent_node_run_id}"))
        .expect("parent llm node should be projected");
    let parent_children = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(parent.trace_node_id))
        .collect::<Vec<_>>();
    let tools = parent_children
        .iter()
        .find(|node| node.node_kind == "tool_group")
        .expect("ordinary tools group should remain projected");
    let agents = parent_children
        .iter()
        .find(|node| node.node_kind == "agent_group")
        .expect("linked subagents should be projected under an Agents group");
    let tool_aliases = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(tools.trace_node_id))
        .map(|node| node.node_alias.as_str())
        .collect::<Vec<_>>();
    let subagent_node = projection
        .nodes
        .iter()
        .find(|node| node.parent_trace_node_id == Some(agents.trace_node_id))
        .expect("Agents group should contain the linked subagent llm node");
    let subagent_tools = projection
        .nodes
        .iter()
        .find(|node| {
            node.parent_trace_node_id == Some(subagent_node.trace_node_id)
                && node.node_kind == "tool_group"
        })
        .expect("subagent llm node should expose its own Tools group");
    let subagent_tool_aliases = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(subagent_tools.trace_node_id))
        .map(|node| node.node_alias.as_str())
        .collect::<Vec<_>>();

    assert_eq!(parent.child_count, 2);
    assert_eq!(tools.child_count, 1);
    assert_eq!(tool_aliases, vec!["Read"]);
    assert_eq!(agents.node_alias, "Agents");
    assert_eq!(agents.child_count, 1);
    assert_eq!(subagent_node.node_kind, "node_run");
    assert_eq!(subagent_node.node_type.as_deref(), Some("llm"));
    assert_eq!(subagent_node.node_alias, "Backend worker");
    assert_eq!(subagent_node.child_count, 1);
    assert_eq!(subagent_node.source_flow_run_id, Some(subagent_run_id));
    assert_eq!(
        subagent_node.parent_callback_task_id,
        Some(callback_task_id)
    );
    assert_eq!(
        subagent_node.parent_tool_call_id.as_deref(),
        Some("call-agent")
    );
    assert_eq!(
        subagent_node.trace_relation_kind.as_deref(),
        Some("subagent")
    );
    assert_eq!(
        subagent_node.source_trace_node_id,
        Some(trace_node_id_for_locator(
            subagent_run_id,
            &format!("run:{subagent_run_id}/node:{subagent_llm_node_run_id}")
        ))
    );
    assert_eq!(subagent_tools.node_alias, "Tools");
    assert_eq!(subagent_tool_aliases, vec!["Bash"]);
    let subagent_content = projection
        .contents
        .iter()
        .find(|content| {
            content.trace_node_id == subagent_node.trace_node_id
                && content.content_kind == "node_run"
        })
        .expect("subagent node should expose projected content");
    assert_eq!(
        subagent_content.source_refs[0]["source_kind"],
        json!("subagent_node_run")
    );
    assert_eq!(
        subagent_content.source_refs[0]["source_locator"],
        json!(subagent_llm_node_run_id.to_string())
    );
    assert_eq!(
        subagent_content.payload["debug_payload"]["parent_agent_tool_call"]["description"],
        json!("Backend worker")
    );
    assert!(subagent_content
        .payload
        .materialize_for_persistence()
        .as_object()
        .expect("subagent content payload should be an object")
        .get("input_payload")
        .is_none());
}

#[test]
fn builder_projects_linked_agent_without_llm_node_run_as_fallback_llm_node() {
    let flow_run_id = Uuid::now_v7();
    let parent_node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let subagent_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
        native_messages: Vec::new(),
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: parent_node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Parent LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({ "prompt": "coordinate work" }),
            output_payload: json!({ "answer": "delegated" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            started_at: now,
            finished_at: Some(now + time::Duration::seconds(10)),
        }],
        checkpoints: Vec::new(),
        callback_tasks: vec![domain::CallbackTaskRecord {
            id: callback_task_id,
            flow_run_id,
            node_run_id: parent_node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Completed,
            request_payload: json!({
                "tool_calls": [
                    { "id": "call-agent", "name": "Agent" }
                ]
            }),
            response_payload: Some(json!({
                "tool_results": [
                    { "tool_call_id": "call-agent", "content": "agent failed" }
                ]
            })),
            external_ref_payload: None,
            created_at: now + time::Duration::seconds(1),
            completed_at: Some(now + time::Duration::seconds(9)),
        }],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: vec![domain::ApplicationRunSubagentTrace {
            parent_tool_call_id: "call-agent".to_string(),
            parent_callback_task_id: callback_task_id,
            source_flow_run: domain::FlowRunRecord {
                id: subagent_run_id,
                title: "Failed investigation agent".to_string(),
                status: domain::FlowRunStatus::Failed,
                input_payload: json!({
                    "node-start": {
                        "query": "find the failing backend path"
                    }
                }),
                output_payload: json!({ "partial": "looked at logs" }),
                error_payload: Some(json!({ "message": "provider returned 520" })),
                started_at: now + time::Duration::seconds(2),
                finished_at: Some(now + time::Duration::seconds(8)),
                ..flow_run(subagent_run_id, now + time::Duration::seconds(2))
            },
            node_runs: Vec::new(),
            callback_tasks: Vec::new(),
            events: Vec::new(),
            runtime_events: Vec::new(),
        }],
        task_rounds: Vec::new(),
        child_task_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let parent = projection
        .nodes
        .iter()
        .find(|node| node.stable_locator == format!("run:{flow_run_id}/node:{parent_node_run_id}"))
        .expect("parent llm node should be projected");
    let parent_children = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(parent.trace_node_id))
        .collect::<Vec<_>>();
    let agents = parent_children
        .iter()
        .find(|node| node.node_kind == "agent_group")
        .expect("linked failed subagent should still be projected under Agents");
    let subagent_node = projection
        .nodes
        .iter()
        .find(|node| node.parent_trace_node_id == Some(agents.trace_node_id))
        .expect("Agents group should contain a fallback subagent llm node");

    assert_eq!(parent.child_count, 1);
    assert!(
        parent_children
            .iter()
            .all(|node| node.node_kind != "tool_group"),
        "linked Agent tool call must not be duplicated as a parent Tool"
    );
    assert_eq!(agents.child_count, 1);
    assert_eq!(subagent_node.node_kind, "node_run");
    assert_eq!(subagent_node.node_type.as_deref(), Some("llm"));
    assert_eq!(subagent_node.node_alias, "Subagent");
    assert_eq!(subagent_node.status, domain::NodeRunStatus::Failed.as_str());
    assert!(!subagent_node.has_children);
    assert_eq!(subagent_node.child_count, 0);
    assert_eq!(subagent_node.source_flow_run_id, Some(subagent_run_id));
    assert_eq!(subagent_node.source_trace_node_id, None);
    assert_eq!(
        subagent_node.parent_callback_task_id,
        Some(callback_task_id)
    );
    assert_eq!(
        subagent_node.parent_tool_call_id.as_deref(),
        Some("call-agent")
    );
    assert_eq!(
        subagent_node.trace_relation_kind.as_deref(),
        Some("subagent")
    );

    let content = projection
        .contents
        .iter()
        .find(|content| content.trace_node_id == subagent_node.trace_node_id)
        .expect("fallback subagent node should expose node content");
    assert_eq!(content.content_kind, "node_run");
    assert_eq!(
        content.payload["payload_index"]["source_flow_run_id"],
        json!(subagent_run_id.to_string())
    );
    assert_eq!(content.payload["payload_index"]["node_run_count"], json!(0));
    assert_eq!(
        content.payload["input_payload"]["node-start"]["query"],
        json!("find the failing backend path")
    );
    assert_eq!(
        content.payload["output_payload"]["partial"],
        json!("looked at logs")
    );
    assert_eq!(
        content.payload["error_payload"]["message"],
        json!("provider returned 520")
    );
}
