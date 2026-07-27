use super::*;
use serde_json::json;

#[test]
fn stable_trace_node_id_is_deterministic_for_locator() {
    let flow_run_id = Uuid::now_v7();
    let locator = format!("run:{flow_run_id}/node:{}", Uuid::now_v7());

    let first = trace_node_id_for_locator(flow_run_id, &locator);
    let second = trace_node_id_for_locator(flow_run_id, &locator);
    let other = trace_node_id_for_locator(flow_run_id, &(locator.clone() + "/tools"));

    assert_eq!(first, second);
    assert_ne!(first, other);
}

#[test]
fn legacy_locator_component_uses_source_payload_fingerprint() {
    let first = legacy_locator_component(
        "visible_internal_llm_tool_trace",
        "000001/000001",
        &json!({ "route": "image", "branch": 1 }),
    );
    let second = legacy_locator_component(
        "visible_internal_llm_tool_trace",
        "000001/000001",
        &json!({ "route": "image", "branch": 1 }),
    );
    let changed = legacy_locator_component(
        "visible_internal_llm_tool_trace",
        "000001/000001",
        &json!({ "route": "image", "branch": 2 }),
    );

    assert!(first.starts_with("legacy:"));
    assert_eq!(first, second);
    assert_ne!(first, changed);
}

#[test]
fn ac_004_answer_node_truth_trace_hides_legacy_waiting_snapshots() {
    let flow_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let final_answer_node_run_id = Uuid::now_v7();
    let node_run = |id: Uuid,
                    node_id: &str,
                    node_type: &str,
                    input_payload: serde_json::Value,
                    debug_payload: serde_json::Value,
                    offset: i64| {
        domain::NodeRunRecord {
            id,
            flow_run_id,
            node_id: node_id.to_string(),
            node_type: node_type.to_string(),
            node_alias: match node_type {
                "start" => "Start",
                "llm" => "LLM",
                "answer" => "Answer",
                other => other,
            }
            .to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload,
            output_payload: json!({ "answer": "visible" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload,
            started_at: now + time::Duration::seconds(offset),
            finished_at: Some(now + time::Duration::seconds(offset)),
        }
    };
    let detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![
            node_run(
                Uuid::now_v7(),
                "node-start",
                "start",
                json!({}),
                json!({}),
                0,
            ),
            node_run(Uuid::now_v7(), "node-llm", "llm", json!({}), json!({}), 1),
            node_run(
                Uuid::now_v7(),
                "node-answer",
                "answer",
                json!({
                    "presentation": {
                        "kind": "answer",
                        "materialized_from": "waiting_prefix"
                    }
                }),
                json!({}),
                2,
            ),
            node_run(Uuid::now_v7(), "node-llm", "llm", json!({}), json!({}), 3),
            node_run(
                Uuid::now_v7(),
                "node-answer",
                "answer",
                json!({}),
                json!({
                    "answer_presentation": {
                        "materialized_from": "canonical_stream_state"
                    }
                }),
                4,
            ),
            node_run(Uuid::now_v7(), "node-llm", "llm", json!({}), json!({}), 5),
            node_run(
                final_answer_node_run_id,
                "node-answer",
                "answer",
                json!({ "answer_template": "final answer" }),
                json!({}),
                6,
            ),
        ],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let top_level_node_runs = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id.is_none() && node.node_kind == "node_run")
        .collect::<Vec<_>>();

    assert_eq!(
        top_level_node_runs
            .iter()
            .filter_map(|node| node.node_type.as_deref())
            .collect::<Vec<_>>(),
        vec!["start", "llm", "answer"]
    );
    let answer = top_level_node_runs
        .iter()
        .find(|node| node.node_type.as_deref() == Some("answer"))
        .expect("the executed final Answer node should remain visible");
    assert_eq!(answer.owner_id, final_answer_node_run_id.to_string());
}

#[test]
fn builder_projects_node_run_tool_group_and_tool_callbacks() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
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
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let locators: Vec<&str> = projection
        .nodes
        .iter()
        .map(|node| node.stable_locator.as_str())
        .collect();

    assert_eq!(
        projection.projection_version,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION
    );
    assert!(locators.contains(&format!("run:{flow_run_id}/node:{node_run_id}").as_str()));
    assert!(locators.contains(&format!("run:{flow_run_id}/node:{node_run_id}/tools").as_str()));
    assert!(locators.contains(
        &format!("run:{flow_run_id}/node:{node_run_id}/tools/tool:call-weather").as_str()
    ));
    let tool_callback_node = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "tool_callback")
        .expect("tool callback node should be projected");
    assert_eq!(
        tool_callback_node.metrics_payload["usage"]["total_tokens"],
        json!(14)
    );
    assert_eq!(
        tool_callback_node.metrics_payload["usage"]["input_tokens"],
        json!(11)
    );
    assert_eq!(
        tool_callback_node.metrics_payload["usage"]["output_tokens"],
        json!(3)
    );
    assert_eq!(projection.contents.len(), 2);
    let node_run_content = projection
        .contents
        .iter()
        .find(|content| content.content_kind == "node_run")
        .expect("node run content should be projected");
    assert_eq!(
        node_run_content.payload["payload_index"]["node_run_count"],
        json!(1)
    );
    assert!(node_run_content.payload.get("node_run").is_none());
    assert!(projection.contents.iter().any(|content| {
        content.content_kind == "tool_callback"
            && content.payload["tool_call_id"] == json!("call-weather")
            && content.payload["call_usage"]["total_tokens"] == json!(14)
            && content.payload["tool_result"]["content"] == json!("22c")
    }));
}

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

#[test]
fn builder_projects_node_run_content_as_lightweight_refs() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let checkpoint_id = Uuid::now_v7();
    let event_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({ "prompt": "refund" }),
            output_payload: json!({ "answer": "done" }),
            error_payload: None,
            metrics_payload: json!({ "usage": { "total_tokens": 12 } }),
            debug_payload: json!({
                "visible_internal_llm_tool_trace": ["large route trace"],
                "debug_summary": {
                    "kept": true
                }
            }),
            started_at: now,
            finished_at: Some(now + time::Duration::seconds(2)),
        }],
        checkpoints: vec![domain::CheckpointRecord {
            id: checkpoint_id,
            flow_run_id,
            node_run_id: Some(node_run_id),
            status: "waiting_callback".to_string(),
            reason: "human_input".to_string(),
            locator_payload: json!({ "node_id": "node-llm" }),
            variable_snapshot: json!({ "large": ["snapshot"] }),
            external_ref_payload: None,
            created_at: now,
        }],
        callback_tasks: Vec::new(),
        events: vec![domain::RunEventRecord {
            id: event_id,
            flow_run_id,
            node_run_id: Some(node_run_id),
            sequence: 1,
            event_type: "node_completed".to_string(),
            payload: json!({ "large": ["event"] }),
            created_at: now,
        }],
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let node_run_content = projection
        .contents
        .iter()
        .find(|content| content.content_kind == "node_run")
        .expect("node run content should be projected");

    assert!(node_run_content.payload.get("node_run").is_none());
    assert!(node_run_content.payload.get("checkpoints").is_none());
    assert!(node_run_content.payload.get("events").is_none());
    assert_eq!(
        node_run_content.payload["payload_index"]["node_run_count"],
        json!(1)
    );
    assert_eq!(
        node_run_content.payload["payload_index"]["checkpoint_count"],
        json!(1)
    );
    assert_eq!(
        node_run_content.payload["payload_index"]["event_count"],
        json!(1)
    );
    assert!(node_run_content.payload["detail_refs"]
        .as_array()
        .is_some_and(|refs| refs.iter().any(|value| {
            value["detail_kind"] == json!("node_run")
                && value["source_locator"] == json!(node_run_id.to_string())
        })));
    assert_eq!(
        node_run_content.source_refs[0]["source_kind"],
        json!("node_run")
    );
}

#[test]
fn builder_projects_tool_route_fusion_and_branch_nodes() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({ "prompt": "review" }),
            output_payload: json!({ "answer": "done" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "visible_internal_llm_tool_trace": [
                    {
                        "kind": "visible_internal_llm_tool_trace",
                        "route_kind": "fusion",
                        "tool_call_id": "call-review",
                        "tool_name": "problem_review",
                        "status": "succeeded",
                        "route_model": "mimo-v2.5",
                        "branch_traces": [
                            {
                                "branch_ref": "panel-a",
                                "node_id": "node-panel-a",
                                "node_alias": "LLM2",
                                "node_type": "llm",
                                "status": "succeeded",
                                "output_summary": {
                                    "preview": "panel A says strict"
                                }
                            }
                        ]
                    }
                ]
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
                    { "id": "call-review", "name": "problem_review" }
                ]
            }),
            response_payload: Some(json!({
                "tool_results": [
                    {
                        "tool_call_id": "call-review",
                        "content": "review complete"
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
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let tool_callback = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "tool_callback")
        .expect("tool callback node should be projected");
    let fusion = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "fusion")
        .expect("fusion route node should be projected");
    let branch = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "branch")
        .expect("branch node should be projected");

    assert!(tool_callback.has_children);
    assert_eq!(tool_callback.child_count, 1);
    assert_eq!(
        fusion.parent_trace_node_id,
        Some(tool_callback.trace_node_id)
    );
    assert!(fusion.has_children);
    assert_eq!(fusion.child_count, 1);
    assert_eq!(branch.parent_trace_node_id, Some(fusion.trace_node_id));
    assert!(fusion.stable_locator.ends_with("/fusion:call-review"));
    assert!(branch.stable_locator.ends_with("/branch:panel-a"));
    assert!(projection.contents.iter().any(|content| {
        content.trace_node_id == fusion.trace_node_id
            && content.content_kind == "fusion"
            && content.payload["route_model"] == json!("mimo-v2.5")
    }));
    assert!(projection.contents.iter().any(|content| {
        content.trace_node_id == branch.trace_node_id
            && content.content_kind == "branch"
            && content.payload["output_summary"]["preview"] == json!("panel A says strict")
    }));
}

#[test]
fn builder_projects_intercepted_route_tool_callback_status() {
    let flow_run_id = Uuid::now_v7();
    let node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![domain::NodeRunRecord {
            id: node_run_id,
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            status: domain::NodeRunStatus::Succeeded,
            input_payload: json!({ "prompt": "describe image" }),
            output_payload: json!({ "answer": "done" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "visible_internal_llm_tool_trace": [
                    {
                        "kind": "visible_internal_llm_tool_trace",
                        "route_kind": "route",
                        "tool_call_id": "call-image",
                        "tool_name": "image_llm",
                        "status": "failed",
                        "events": [
                            {
                                "event_type": "visible_internal_llm_tool_failed",
                                "error_payload": {
                                    "error_code": "visible_internal_llm_tool_failed",
                                    "details": {
                                        "error_code": "visible_internal_llm_tool_media_unavailable",
                                        "recoverable": true
                                    }
                                }
                            }
                        ],
                        "tool_result": {
                            "tool_call_id": "call-image",
                            "name": "image_llm",
                            "content": "read the file with a client file tool first"
                        }
                    }
                ]
            }),
            started_at: now,
            finished_at: Some(now + time::Duration::seconds(3)),
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
                    { "id": "call-image", "name": "image_llm" }
                ]
            }),
            response_payload: Some(json!({
                "tool_results": [
                    {
                        "tool_call_id": "call-image",
                        "content": "read the file with a client file tool first"
                    }
                ]
            })),
            external_ref_payload: None,
            created_at: now + time::Duration::seconds(1),
            completed_at: Some(now + time::Duration::seconds(3)),
        }],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let tool_callback = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "tool_callback")
        .expect("tool callback node should be projected");
    let route = projection
        .nodes
        .iter()
        .find(|node| node.parent_trace_node_id == Some(tool_callback.trace_node_id))
        .expect("route child node should be projected");
    let tool_callback_content = projection
        .contents
        .iter()
        .find(|content| content.trace_node_id == tool_callback.trace_node_id)
        .expect("tool callback content should be projected");

    assert_eq!(tool_callback.status, "intercepted");
    assert_eq!(route.status, "intercepted");
    assert_eq!(
        tool_callback_content.payload["execution_status"],
        json!("intercepted")
    );
    assert_eq!(
        tool_callback_content.payload["route_trace"]["status"],
        json!("failed")
    );
}

#[test]
fn builder_merges_callback_task_tools_with_internal_route_tools() {
    let flow_run_id = Uuid::now_v7();
    let callback_node_run_id = Uuid::now_v7();
    let route_node_run_id = Uuid::now_v7();
    let callback_task_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let ordinary_tool_calls = json!([
        { "id": "call-read-memory", "name": "Read" },
        { "id": "call-read-agents", "name": "Read" },
        { "id": "call-git-pull", "name": "Bash" },
        { "id": "call-git-log", "name": "Bash" },
        { "id": "call-git-log-detail", "name": "Bash" }
    ]);
    let branch_traces = json!([
        {
            "branch_ref": "panel-a",
            "node_id": "node-llm-2",
            "node_alias": "LLM2",
            "node_type": "llm",
            "status": "succeeded"
        },
        {
            "branch_ref": "panel-b",
            "node_id": "node-llm-3",
            "node_alias": "LLM3",
            "node_type": "llm",
            "status": "succeeded"
        },
        {
            "branch_ref": "panel-c",
            "node_id": "node-llm-4",
            "node_alias": "LLM4",
            "node_type": "llm",
            "status": "succeeded"
        },
        {
            "branch_ref": "panel-d",
            "node_id": "node-llm-5",
            "node_alias": "LLM5",
            "node_type": "llm",
            "status": "succeeded"
        }
    ]);
    let detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: vec![
            domain::NodeRunRecord {
                id: callback_node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "Main LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: json!({ "prompt": "prepare context" }),
                output_payload: json!({ "tool_calls": ordinary_tool_calls }),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({}),
                started_at: now,
                finished_at: Some(now + time::Duration::seconds(5)),
            },
            domain::NodeRunRecord {
                id: route_node_run_id,
                flow_run_id,
                node_id: "node-llm".to_string(),
                node_type: "llm".to_string(),
                node_alias: "Main LLM".to_string(),
                status: domain::NodeRunStatus::Succeeded,
                input_payload: json!({ "prompt": "review latest commits" }),
                output_payload: json!({ "answer": "review complete" }),
                error_payload: None,
                metrics_payload: json!({}),
                debug_payload: json!({
                    "llm_rounds": [
                        {
                            "round_index": 3,
                            "assistant": {
                                "tool_calls": [
                                    {
                                        "id": "call-problem-review",
                                        "name": "problem_review"
                                    }
                                ]
                            },
                            "tool_results": [
                                {
                                    "tool_call_id": "call-problem-review",
                                    "name": "problem_review",
                                    "content": "problem review result"
                                }
                            ]
                        }
                    ],
                    "visible_internal_llm_tool_trace": [
                        {
                            "kind": "visible_internal_llm_tool_trace",
                            "route_kind": "fusion",
                            "tool_call_id": "call-problem-review",
                            "tool_name": "problem_review",
                            "status": "succeeded",
                            "route_model": "gemini-3-flash",
                            "branch_traces": branch_traces
                        }
                    ]
                }),
                started_at: now + time::Duration::seconds(6),
                finished_at: Some(now + time::Duration::seconds(12)),
            },
        ],
        checkpoints: Vec::new(),
        callback_tasks: vec![domain::CallbackTaskRecord {
            id: callback_task_id,
            flow_run_id,
            node_run_id: callback_node_run_id,
            callback_kind: "llm_tool_calls".to_string(),
            status: domain::CallbackTaskStatus::Completed,
            request_payload: json!({
                "tool_calls": ordinary_tool_calls
            }),
            response_payload: Some(json!({
                "tool_results": [
                    { "tool_call_id": "call-read-memory", "content": "memory" },
                    { "tool_call_id": "call-read-agents", "content": "agents" },
                    { "tool_call_id": "call-git-pull", "content": "pulled" },
                    { "tool_call_id": "call-git-log", "content": "log" },
                    { "tool_call_id": "call-git-log-detail", "content": "details" }
                ]
            })),
            external_ref_payload: None,
            created_at: now + time::Duration::seconds(1),
            completed_at: Some(now + time::Duration::seconds(5)),
        }],
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
    };

    let projection = build_application_run_trace_projection(&detail).unwrap();
    let tools = projection
        .nodes
        .iter()
        .find(|node| node.node_kind == "tool_group")
        .expect("tool group should be projected");
    let tool_callbacks: Vec<_> = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(tools.trace_node_id))
        .collect();
    let problem_review = tool_callbacks
        .iter()
        .find(|node| node.node_alias == "problem_review")
        .expect("internal route tool should be projected beside callback task tools");
    let fusion = projection
        .nodes
        .iter()
        .find(|node| node.parent_trace_node_id == Some(problem_review.trace_node_id))
        .expect("problem_review should expose its fusion route");
    let branch_count = projection
        .nodes
        .iter()
        .filter(|node| node.parent_trace_node_id == Some(fusion.trace_node_id))
        .count();

    assert_eq!(tools.child_count, 6);
    assert_eq!(tool_callbacks.len(), 6);
    assert!(problem_review.has_children);
    assert_eq!(problem_review.child_count, 1);
    assert_eq!(problem_review.node_mode.as_deref(), Some("fusion"));
    assert_eq!(fusion.node_kind, "fusion");
    assert_eq!(fusion.child_count, 4);
    assert_eq!(branch_count, 4);
    assert!(projection.contents.iter().any(|content| {
        content.trace_node_id == problem_review.trace_node_id
            && content.content_kind == "tool_callback"
            && content.payload["callback_status"] == json!("returned")
            && content.payload["parsed_result"]["content"] == json!("problem review result")
            && content.payload["tool_result"]["content"] == json!("problem review result")
    }));
}

#[test]
fn builder_projects_stitched_trace_as_collapsed_context_group() {
    let flow_run_id = Uuid::now_v7();
    let prior_run_id = Uuid::now_v7();
    let prior_node_run_id = Uuid::now_v7();
    let now = OffsetDateTime::UNIX_EPOCH;
    let mut detail = domain::ApplicationRunDetail {
        flow_run: flow_run(flow_run_id, now),
        node_runs: Vec::new(),
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: Vec::new(),
        stitched_trace: Vec::new(),
        subagent_traces: Vec::new(),
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
        flow_run: domain::FlowRunRecord {
            target_node_id: Some("node-llm".to_string()),
            ..flow_run(flow_run_id, now)
        },
        node_runs: vec![domain::NodeRunRecord {
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

fn status_record(
    flow_run_id: Uuid,
    projection_version: i32,
    status: domain::ApplicationRunTraceProjectionStatus,
    source_watermark: &str,
    now: OffsetDateTime,
) -> domain::ApplicationRunTraceProjectionStatusRecord {
    domain::ApplicationRunTraceProjectionStatusRecord {
        flow_run_id,
        projection_version,
        status,
        source_watermark: source_watermark.to_string(),
        attempt_count: 1,
        last_attempt_at: Some(now),
        last_success_at: Some(now),
        last_error_code: None,
        last_error_stage: None,
        last_error_source_kind: None,
        last_error_source_locator: None,
        last_error_message: None,
        last_error_ref: None,
        retriable: false,
        created_at: now,
        updated_at: now,
    }
}
