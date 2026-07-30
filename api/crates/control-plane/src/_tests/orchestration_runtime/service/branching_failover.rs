use super::*;
use plugin_framework::provider_contract::{ProviderFinishReason, ProviderInvocationResult};

fn branching_answer_flow_document(flow_id: Uuid) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "Branching Answer Agent",
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
                                                "left": ["node-start", "query"],
                                                "comparator": "exists"
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
                    "id": "node-llm-selected",
                    "type": "llm",
                    "alias": "Selected LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        }
                    },
                    "bindings": {
                        "prompt_messages": {
                            "kind": "prompt_messages",
                            "value": [{
                                "id": "user-selected",
                                "role": "user",
                                "content": {
                                    "kind": "templated_text",
                                    "value": "{{node-start.query}}"
                                }
                            }]
                        }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-answer-selected",
                    "type": "answer",
                    "alias": "Selected Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "{{node-llm-selected.text}}"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                },
                {
                    "id": "node-llm-inactive",
                    "type": "llm",
                    "alias": "Inactive LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 160 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        }
                    },
                    "bindings": {
                        "prompt_messages": {
                            "kind": "prompt_messages",
                            "value": [{
                                "id": "user-inactive",
                                "role": "user",
                                "content": {
                                    "kind": "templated_text",
                                    "value": "inactive"
                                }
                            }]
                        }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-answer-inactive",
                    "type": "answer",
                    "alias": "Inactive Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": 160 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "{{node-llm-inactive.text}}"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
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
                    "id": "edge-if-selected",
                    "source": "node-if",
                    "target": "node-llm-selected",
                    "sourceHandle": "if",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-else-inactive",
                    "source": "node-if",
                    "target": "node-llm-inactive",
                    "sourceHandle": "else",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-selected-answer",
                    "source": "node-llm-selected",
                    "target": "node-answer-selected",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-inactive-answer",
                    "source": "node-llm-inactive",
                    "target": "node-answer-inactive",
                    "sourceHandle": null,
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
    })
}

#[tokio::test]
async fn selected_answer_branch_projects_reasoning_before_text_exactly_once() {
    use plugin_framework::provider_contract::ProviderStreamEvent;

    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::ReasoningDelta {
            delta: "先分析".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "最终".to_string(),
        },
        ProviderStreamEvent::TextDelta {
            delta: "回答".to_string(),
        },
    ]);
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Branching Answer Agent")
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "use selected branch" } }),
            document_snapshot: Some(branching_answer_flow_document(seeded.flow_id)),
            debug_session_id: None,
        })
        .await
        .expect("branching flow should start");
    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .expect("selected branch should complete");

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert!(completed
        .node_runs
        .iter()
        .any(|node_run| node_run.node_id == "node-answer-selected"));
    assert!(completed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-answer-inactive"));

    let presentation_events = stream
        .events()
        .into_iter()
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter(|event| matches!(event.event_type.as_str(), "reasoning_delta" | "text_delta"))
        .collect::<Vec<_>>();
    assert_eq!(
        presentation_events
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec!["reasoning_delta", "text_delta", "text_delta"]
    );
    assert_eq!(presentation_events[0].payload["text"], json!("先分析"));
    assert_eq!(
        presentation_events[1..]
            .iter()
            .filter_map(|event| event.payload["text"].as_str())
            .collect::<String>(),
        "最终回答"
    );
}

#[tokio::test]
async fn live_failed_llm_with_inactive_later_branch_keeps_answer_unmaterialized() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_results(vec![
        ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
    ]);
    let seeded = service
        .seed_application_with_flow("Branch Failover Agent")
        .await;
    let document = json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": seeded.flow_id.to_string(),
            "name": "Branch Failover Agent",
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
                                                "left": ["node-start", "query"],
                                                "comparator": "exists"
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
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "model_id": "gpt-5.4-mini"
                        }
                    },
                    "bindings": {
                        "prompt_messages": {
                            "kind": "prompt_messages",
                            "value": [{
                                "id": "user-1",
                                "role": "user",
                                "content": {
                                    "kind": "templated_text",
                                    "value": "{{node-start.query}}"
                                }
                            }]
                        }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": {
                            "kind": "templated_text",
                            "value": "{{ node-llm.text }}"
                        }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                },
                {
                    "id": "node-inactive",
                    "type": "variable_assigner",
                    "alias": "Inactive Assign",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 120 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
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
                    "id": "edge-if-llm",
                    "source": "node-if",
                    "target": "node-llm",
                    "sourceHandle": "if",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-else-inactive",
                    "source": "node-if",
                    "target": "node-inactive",
                    "sourceHandle": "else",
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                },
                {
                    "id": "edge-llm-answer",
                    "source": "node-llm",
                    "target": "node-answer",
                    "sourceHandle": null,
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
            input_payload: json!({ "node-start": { "query": "hi" } }),
            document_snapshot: Some(document),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let failed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed.flow_run.status, domain::FlowRunStatus::Failed);
    assert_eq!(
        node_run(&failed, "node-if").debug_payload["selected_source_handle"],
        json!("if")
    );
    assert_eq!(
        node_run(&failed, "node-llm").status,
        domain::NodeRunStatus::Failed
    );
    assert!(failed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-answer"));
    assert!(failed.flow_run.output_payload.get("answer").is_none());
    assert!(failed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-inactive"));
}
