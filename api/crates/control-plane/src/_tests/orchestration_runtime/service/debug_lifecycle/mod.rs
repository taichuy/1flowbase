use super::*;

#[tokio::test]
async fn start_node_debug_preview_creates_run_node_run_and_events() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let outcome = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-llm".to_string(),
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    assert_eq!(outcome.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(outcome.node_run.status, domain::NodeRunStatus::Succeeded);
    assert!(outcome
        .events
        .iter()
        .any(|event| event.event_type == "node_preview_completed"));
}

#[tokio::test]
async fn live_run_failure_keeps_answer_contract_unmaterialized() {
    use plugin_framework::provider_contract::{ProviderFinishReason, ProviderInvocationResult};

    let service = OrchestrationRuntimeService::for_tests_with_provider_results(vec![
        ProviderInvocationResult {
            final_content: Some("first answer".to_string()),
            finish_reason: Some(ProviderFinishReason::Stop),
            ..ProviderInvocationResult::default()
        },
        ProviderInvocationResult {
            finish_reason: Some(ProviderFinishReason::Error),
            ..ProviderInvocationResult::default()
        },
    ]);
    let seeded = service
        .seed_application_with_second_llm_failure_flow("Support Agent")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "hi" }
            }),
            document_snapshot: None,
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
    assert!(failed.flow_run.output_payload.get("answer").is_none());
    assert_eq!(
        failed.flow_run.error_payload.as_ref().unwrap()["error_code"],
        json!("provider_invalid_response")
    );
    assert_eq!(
        node_run(&failed, "node-llm").status,
        domain::NodeRunStatus::Succeeded
    );
    assert_eq!(
        node_run(&failed, "node-llm-2").status,
        domain::NodeRunStatus::Failed
    );
    assert!(failed
        .node_runs
        .iter()
        .all(|node_run| node_run.node_id != "node-answer"));
}

#[tokio::test]
async fn start_node_debug_preview_uses_failover_queue_for_multi_instance_provider_model_binding() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_multi_instance_provider_flow("Support Agent")
        .await;

    let outcome = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-llm".to_string(),
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    assert_eq!(outcome.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(outcome.node_run.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(
        outcome.preview_payload["metrics_payload"]["route"]["routing_mode"],
        serde_json::json!("failover_queue")
    );
    assert!(outcome.preview_payload["metrics_payload"]["attempts"]
        .as_array()
        .is_some_and(|attempts| !attempts.is_empty()));
}

#[tokio::test]
async fn start_node_debug_preview_uses_request_document_snapshot() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let outcome = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-llm".to_string(),
            input_payload: serde_json::json!({
                "node-start": { "query": "draft prompt" }
            }),
            document_snapshot: Some(serde_json::json!({
                "schemaVersion": "1flowbase.flow/v2",
                "meta": {
                    "flowId": seeded.flow_id.to_string(),
                    "name": "Support Agent",
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
                            "id": "node-llm",
                            "type": "llm",
                            "alias": "LLM",
                            "description": "",
                            "containerId": null,
                            "position": { "x": 240, "y": 0 },
                            "configVersion": 1,
                            "config": {
                                "model_provider": {
                                    "provider_code": "fixture_provider",
                                    "model_id": "gpt-5.4-mini"
                                }
                            },
                            "bindings": {
                                "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "snapshot {{node-start.query}}" } }] }
                            },
                            "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                        }
                    ],
                    "edges": [
                        {
                            "id": "edge-start-llm",
                            "source": "node-start",
                            "target": "node-llm",
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
            })),
            debug_session_id: None,
        })
        .await
        .unwrap();

    assert_eq!(
        outcome.preview_payload["node_output"]["text"],
        serde_json::json!("echo:gpt-5.4-mini:snapshot draft prompt")
    );
}

#[tokio::test]
async fn start_node_debug_preview_records_provider_invocation_duration() {
    let service = OrchestrationRuntimeService::for_tests_with_provider_delay(
        std::time::Duration::from_millis(50),
    );
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let outcome = service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-llm".to_string(),
            input_payload: serde_json::json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let elapsed = outcome
        .node_run
        .finished_at
        .expect("node preview should finish")
        - outcome.node_run.started_at;

    assert!(elapsed >= Duration::milliseconds(45));
}

#[tokio::test]
async fn start_flow_debug_run_returns_running_detail_before_background_continuation() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_plugin_node_flow("Capability Agent")
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: serde_json::json!({
                "node-start": { "query": "world" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    assert_eq!(started.flow_run.status, domain::FlowRunStatus::Running);
    assert!(started.node_runs.is_empty());
    assert_eq!(started.events[0].event_type, "flow_run_started");
}

#[tokio::test]
async fn flow_debug_run_resolves_system_variables_from_run_context() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let editor_state = service
        .editor_state_for_tests(seeded.application_id, seeded.actor_user_id)
        .await;
    let mut document = editor_state.draft.document.clone();

    document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]["value"] =
        json!("{{sys.user_id}}/{{sys.application_id}}/{{sys.workflow_id}}/{{sys.workflow_run_id}}");

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: Some(document),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    let llm_run = completed
        .node_runs
        .iter()
        .find(|run| run.node_id == "node-llm")
        .expect("llm node run");
    let content = llm_run.input_payload["prompt_messages"][0]["content"]
        .as_str()
        .expect("rendered prompt content");

    assert_eq!(
        content,
        format!(
            "{}/{}/{}/{}",
            seeded.actor_user_id, seeded.application_id, seeded.flow_id, detail.flow_run.id
        )
    );
}

#[tokio::test]
async fn flow_debug_run_counts_prior_native_user_turns_in_system_variables() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "__native_model_prompt_context": {
                    "system": [],
                    "messages": [
                        { "role": "user", "content": "first turn" },
                        { "role": "assistant", "content": "first answer" }
                    ]
                },
                "node-start": {
                    "query": "second turn",
                    "history": [
                        { "role": "user", "content": "first turn" },
                        { "role": "assistant", "content": "first answer" }
                    ]
                }
            }),
            document_snapshot: None,
            debug_session_id: Some("conversation-1".to_string()),
        })
        .await
        .expect("second conversation turn should start");

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .expect("second conversation turn should complete");

    assert_eq!(
        node_run(&completed, "node-start").input_payload["sys"]["dialog_count"],
        json!(1)
    );
    assert_eq!(
        completed.flow_run.output_payload["sys"]["dialog_count"],
        json!(1)
    );
}

#[tokio::test]
async fn flow_debug_run_resolves_application_environment_variables() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    service
        .replace_application_environment_variables_for_tests(
            seeded.actor_user_id,
            seeded.application_id,
            vec![control_plane::ports::ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".to_string(),
                value_type: "string".to_string(),
                value: json!("https://api.example.com"),
                description: "当前应用 API 地址".to_string(),
            }],
        )
        .await;
    let editor_state = service
        .editor_state_for_tests(seeded.application_id, seeded.actor_user_id)
        .await;
    let mut document = editor_state.draft.document.clone();

    document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]["value"] =
        json!("call {{env.ApiBaseUrl}}");

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "hello" } }),
            document_snapshot: Some(document),
            debug_session_id: None,
        })
        .await
        .unwrap();

    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    let llm_run = completed
        .node_runs
        .iter()
        .find(|run| run.node_id == "node-llm")
        .expect("llm node run");

    assert_eq!(
        llm_run.input_payload["prompt_messages"][0]["content"].as_str(),
        Some("call https://api.example.com")
    );
}

#[tokio::test]
async fn live_debug_run_persists_start_context_and_answer_final_variables() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Runtime Context Agent")
        .await;
    service
        .replace_application_environment_variables_for_tests(
            seeded.actor_user_id,
            seeded.application_id,
            vec![control_plane::ports::ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".to_string(),
                value_type: "string".to_string(),
                value: json!("https://api.example.com"),
                description: "当前应用 API 地址".to_string(),
            }],
        )
        .await;
    let mut document = code_to_answer_flow_document(
        seeded.flow_id,
        "function main(inputs) { return { result: inputs.query + ' via ' + inputs.base_url }; }",
    );
    document["graph"]["nodes"][1]["bindings"]["base_url"] = json!({
        "kind": "selector",
        "value": ["env", "ApiBaseUrl"]
    });

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "conversation": { "Locale": "zh-CN" },
                "node-start": { "query": "hello" }
            }),
            document_snapshot: Some(document),
            debug_session_id: Some("debug-session-1".to_string()),
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

    let start_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-start")
        .expect("start node should be persisted");
    assert_eq!(start_node.input_payload["query"], json!("hello"));
    assert_eq!(
        start_node.input_payload["env"]["ApiBaseUrl"],
        json!("https://api.example.com")
    );
    assert_eq!(
        start_node.input_payload["sys"]["conversation_id"],
        json!("debug-session-1")
    );
    assert_eq!(
        start_node.input_payload["sys"]["application_id"],
        json!(seeded.application_id.to_string())
    );
    assert!(start_node.input_payload["sys"].get("app_id").is_none());
    assert_eq!(
        start_node.input_payload["sys"]["workflow_run_id"],
        json!(started.flow_run.id.to_string())
    );
    assert_eq!(start_node.output_payload, json!({}));

    let code_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-code")
        .expect("code node should be persisted");
    assert_eq!(
        code_node.input_payload,
        json!({
            "query": "hello",
            "base_url": "https://api.example.com"
        })
    );

    let answer_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-answer")
        .expect("answer node should be persisted");
    assert_eq!(
        answer_node.output_payload["answer"],
        json!("Code said: hello via https://api.example.com")
    );
    assert_eq!(
        answer_node.output_payload["env"]["ApiBaseUrl"],
        json!("https://api.example.com")
    );
    assert_eq!(
        answer_node.output_payload["sys"]["workflow_run_id"],
        json!(started.flow_run.id.to_string())
    );
    assert_eq!(
        answer_node.output_payload["sys"]["application_id"],
        json!(seeded.application_id.to_string())
    );
    assert!(answer_node.output_payload["sys"].get("app_id").is_none());
    assert_eq!(
        answer_node.output_payload["conversation"]["Locale"],
        json!("zh-CN")
    );
    assert_eq!(
        completed.flow_run.output_payload,
        answer_node.output_payload
    );

    service
        .replace_application_environment_variables_for_tests(
            seeded.actor_user_id,
            seeded.application_id,
            vec![control_plane::ports::ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".to_string(),
                value_type: "string".to_string(),
                value: json!("https://api.changed.example.com"),
                description: "更新后的应用 API 地址".to_string(),
            }],
        )
        .await;
    let reloaded = service
        .application_run_detail(seeded.application_id, completed.flow_run.id)
        .await;
    let reloaded_start = reloaded
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-start")
        .expect("reloaded start node should exist");
    assert_eq!(
        reloaded_start.input_payload["env"]["ApiBaseUrl"],
        json!("https://api.example.com")
    );
    assert_eq!(
        reloaded.flow_run.output_payload["env"]["ApiBaseUrl"],
        json!("https://api.example.com")
    );
}

#[tokio::test]
async fn first_execution_uses_runtime_variable_assigner_semantics() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Conversation State Agent")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "conversation": { "ApiBaseUrl": "https://old.example.com" },
                "node-start": { "query": "new.example.com" }
            }),
            document_snapshot: Some(variable_assigner_to_answer_flow_document(seeded.flow_id)),
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
        node_run(&completed, "node-variable").output_payload["ApiBaseUrl"],
        json!("https://new.example.com/v1")
    );
    assert_eq!(
        completed.flow_run.output_payload["conversation"]["ApiBaseUrl"],
        json!("https://new.example.com/v1")
    );
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("https://new.example.com/v1")
    );
}

#[tokio::test]
async fn live_debug_run_uses_environment_snapshot_from_opened_shell() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Runtime Shell Context Agent")
        .await;
    service
        .replace_application_environment_variables_for_tests(
            seeded.actor_user_id,
            seeded.application_id,
            vec![control_plane::ports::ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".to_string(),
                value_type: "string".to_string(),
                value: json!("https://api.at-open.example.com"),
                description: "打开运行时的 API 地址".to_string(),
            }],
        )
        .await;
    let mut document = code_to_answer_flow_document(
        seeded.flow_id,
        "function main(inputs) { return { result: inputs.base_url }; }",
    );
    document["graph"]["nodes"][1]["bindings"]["base_url"] = json!({
        "kind": "selector",
        "value": ["env", "ApiBaseUrl"]
    });
    let input_payload = json!({ "node-start": { "query": "hello" } });
    let debug_session_id = "debug-session-shell".to_string();

    let shell = service
        .open_flow_debug_run_shell(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: input_payload.clone(),
            document_snapshot: Some(document.clone()),
            debug_session_id: Some(debug_session_id.clone()),
        })
        .await
        .unwrap();

    service
        .replace_application_environment_variables_for_tests(
            seeded.actor_user_id,
            seeded.application_id,
            vec![control_plane::ports::ApplicationEnvironmentVariableInput {
                name: "ApiBaseUrl".to_string(),
                value_type: "string".to_string(),
                value: json!("https://api.changed-before-continue.example.com"),
                description: "继续运行前修改后的 API 地址".to_string(),
            }],
        )
        .await;

    service
        .prepare_flow_debug_run_from_shell(PrepareFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: shell.id,
            input_payload,
            document_snapshot: Some(document),
            debug_session_id,
        })
        .await
        .unwrap();
    let completed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: shell.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let start_node = completed
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-start")
        .expect("start node should be persisted");
    assert_eq!(
        start_node.input_payload["env"]["ApiBaseUrl"],
        json!("https://api.at-open.example.com")
    );
    assert_eq!(
        completed.flow_run.output_payload["env"]["ApiBaseUrl"],
        json!("https://api.at-open.example.com")
    );
}

#[tokio::test]
async fn flow_debug_run_fails_before_provider_when_prompt_template_selector_is_missing() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "different-start": { "query": "hello" } }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let failed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(failed.flow_run.status, domain::FlowRunStatus::Failed);
    let message = failed
        .flow_run
        .error_payload
        .as_ref()
        .expect("flow error payload should be persisted")["message"]
        .as_str()
        .expect("error message should be persisted");
    assert!(message.contains("unresolved template selector node-start.query"));
    let llm_node = failed
        .node_runs
        .iter()
        .find(|run| run.node_id == "node-llm")
        .expect("binding failure should be recorded on the attempted LLM node");
    assert_eq!(llm_node.status, domain::NodeRunStatus::Failed);
    assert!(llm_node
        .error_payload
        .as_ref()
        .and_then(|payload| payload.get("message"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|message| message.contains("unresolved template selector")));
}

#[tokio::test]
async fn flow_compile_rejects_unknown_node_type_before_execution() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Unknown Node Agent")
        .await;

    let document = serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": seeded.flow_id.to_string(),
            "name": "Unknown Node Agent",
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
                    "id": "node-unknown",
                    "type": "x_unknown",
                    "alias": "Unknown",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": [{ "key": "result", "title": "结果", "valueType": "json" }]
                }
            ],
            "edges": [{
                "id": "edge-start-unknown",
                "source": "node-start",
                "target": "node-unknown",
                "sourceHandle": null,
                "targetHandle": null,
                "containerId": null,
                "points": []
            }]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    });

    let error = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({}),
            document_snapshot: Some(document),
            debug_session_id: None,
        })
        .await
        .expect_err("unsupported node types must fail before a run is executed");

    assert!(error.to_string().contains("invalid input: node_type"));
}

mod branching;
mod shell_preparation;
mod workflow;
