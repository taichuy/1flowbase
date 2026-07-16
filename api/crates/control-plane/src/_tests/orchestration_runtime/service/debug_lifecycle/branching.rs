use super::*;

#[tokio::test]
async fn live_debug_run_executes_if_else_selected_branch_only() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Branch Agent").await;

    let document = serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": seeded.flow_id.to_string(),
            "name": "Branch Agent",
            "description": "",
            "tags": []
        },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-if",
                    "type": "if_else",
                    "alias": "If / Else",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "branches": {
                            "kind": "if_else_branches",
                            "value": {
                                "branches": [
                                    {
                                        "id": "if",
                                        "kind": "if",
                                        "title": "If",
                                        "sourceHandle": "if",
                                        "condition": {
                                            "operator": "and",
                                            "conditions": [{
                                                "kind": "rule",
                                                "left": ["node-start", "model"],
                                                "comparator": "equals",
                                                "right": { "kind": "constant", "value": "gpt" }
                                            }]
                                        }
                                    },
                                    {
                                        "id": "else-if-1",
                                        "kind": "else_if",
                                        "title": "Else If 1",
                                        "sourceHandle": "else-if-1",
                                        "condition": {
                                            "operator": "and",
                                            "conditions": [{
                                                "kind": "rule",
                                                "left": ["node-start", "model"],
                                                "comparator": "equals",
                                                "right": { "kind": "constant", "value": "mimo" }
                                            }]
                                        }
                                    },
                                    {
                                        "id": "else",
                                        "kind": "else",
                                        "title": "Else",
                                        "sourceHandle": "else"
                                    }
                                ]
                            }
                        }
                    },
                    "outputs": []
                },
                {
                    "id": "node-if-answer",
                    "type": "answer",
                    "alias": "If Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": -80 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "if branch"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
                },
                {
                    "id": "node-elseif-answer",
                    "type": "answer",
                    "alias": "Else If Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 40 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "else-if branch"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
                },
                {
                    "id": "node-else-answer",
                    "type": "answer",
                    "alias": "Else Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 120 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "else branch"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-if",
                    "source": "node-start",
                    "target": "node-if",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-if-answer",
                    "source": "node-if",
                    "target": "node-if-answer",
                    "sourceHandle": "if",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-elseif-answer",
                    "source": "node-if",
                    "target": "node-elseif-answer",
                    "sourceHandle": "else-if-1",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-else-answer",
                    "source": "node-if",
                    "target": "node-else-answer",
                    "sourceHandle": "else",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    });

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "model": "gpt" } }),
            document_snapshot: Some(document.clone()),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        node_run(&completed, "node-if").debug_payload["selected_source_handle"],
        json!("if")
    );
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("if branch")
    );
    assert!(completed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-else-answer"));
    assert!(completed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-elseif-answer"));

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "model": "mimo" } }),
            document_snapshot: Some(document),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        node_run(&completed, "node-if").debug_payload["selected_source_handle"],
        json!("else-if-1")
    );
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("else-if branch")
    );
    assert!(completed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-if-answer"));
    assert!(completed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-else-answer"));
}

#[tokio::test]
async fn live_debug_run_resumes_if_else_selected_branch_callback_only() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Branch Callback Agent")
        .await;

    let document = serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": seeded.flow_id.to_string(),
            "name": "Branch Callback Agent",
            "description": "",
            "tags": []
        },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-if",
                    "type": "if_else",
                    "alias": "If / Else",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "branches": {
                            "kind": "if_else_branches",
                            "value": {
                                "branches": [
                                    {
                                        "id": "if",
                                        "kind": "if",
                                        "title": "If",
                                        "sourceHandle": "if",
                                        "condition": {
                                            "operator": "and",
                                            "conditions": [{
                                                "kind": "rule",
                                                "left": ["node-start", "model"],
                                                "comparator": "equals",
                                                "right": { "kind": "constant", "value": "gpt" }
                                            }]
                                        }
                                    },
                                    {
                                        "id": "else",
                                        "kind": "else",
                                        "title": "Else",
                                        "sourceHandle": "else"
                                    }
                                ]
                            }
                        }
                    },
                    "outputs": []
                },
                {
                    "id": "node-if-tool",
                    "type": "tool",
                    "alias": "If Tool",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": -80 },
                    "configVersion": 1,
                    "config": { "tool_name": "lookup_branch" },
                    "bindings": {
                        "model": { "kind": "selector", "value": ["node-start", "model"] }
                    },
                    "outputs": [{ "key": "result", "title": "Tool Result", "valueType": "string" }]
                },
                {
                    "id": "node-if-answer",
                    "type": "answer",
                    "alias": "If Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": -80 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "selector",
                            "value": ["node-if-tool", "result"]
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
                },
                {
                    "id": "node-else-answer",
                    "type": "answer",
                    "alias": "Else Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 120 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "else branch"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-if",
                    "source": "node-start",
                    "target": "node-if",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-if-tool",
                    "source": "node-if",
                    "target": "node-if-tool",
                    "sourceHandle": "if",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-if-tool-answer",
                    "source": "node-if-tool",
                    "target": "node-if-answer",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-else-answer",
                    "source": "node-if",
                    "target": "node-else-answer",
                    "sourceHandle": "else",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    });

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "model": "gpt" } }),
            document_snapshot: Some(document),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let waiting = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(
        waiting.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    assert_eq!(
        node_run(&waiting, "node-if").debug_payload["selected_source_handle"],
        json!("if")
    );
    assert!(
        waiting
            .node_runs
            .iter()
            .all(|node_run| node_run.node_id != "node-else-answer"),
        "inactive branch Answer must not be materialized before callback resume"
    );
    let checkpoint = waiting
        .checkpoints
        .last()
        .expect("tool callback should store checkpoint");
    assert!(checkpoint.locator_payload["active_node_ids"]
        .as_array()
        .expect("active_node_ids should be an array")
        .iter()
        .all(|node_id| node_id.as_str() != Some("node-else-answer")));

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({ "result": "selected callback" }),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("selected callback")
    );
    let completed_node_ids = completed
        .node_runs
        .iter()
        .map(|node_run| node_run.node_id.as_str())
        .collect::<Vec<_>>();
    assert!(
        completed_node_ids
            .iter()
            .all(|node_id| *node_id != "node-else-answer"),
        "inactive branch node run was persisted: {completed_node_ids:?}"
    );
}
