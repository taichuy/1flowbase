use super::*;

#[tokio::test]
async fn live_llm_tool_calls_create_callback_task_and_pause_downstream() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderToolCall, ProviderUsage,
    };

    let service =
        OrchestrationRuntimeService::for_tests_with_provider_result(ProviderInvocationResult {
            final_content: Some("need tool".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_weather".to_string(),
                name: "lookup_weather".to_string(),
                arguments: json!({ "city": "Shanghai" }),
                provider_metadata: json!({}),
            }],
            usage: ProviderUsage {
                input_tokens: Some(8),
                output_tokens: Some(4),
                total_tokens: Some(12),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        });
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "天气？" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let waiting_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(
        waiting_detail.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    let llm_node = node_run(&waiting_detail, "node-llm");
    assert_eq!(llm_node.status, domain::NodeRunStatus::WaitingCallback);
    assert_eq!(
        llm_node.output_payload["tool_calls"][0]["id"],
        "call_weather"
    );
    let projections = service
        .list_context_projections(waiting_detail.flow_run.id)
        .await;
    let attempts = service
        .list_model_failover_attempt_ledger(waiting_detail.flow_run.id)
        .await;
    assert_resolved_llm_debug_refs(
        &llm_node.debug_payload,
        &projections,
        &attempts,
        llm_node.id,
    );
    let answer_node = node_run(&waiting_detail, "node-answer");
    assert_eq!(answer_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(answer_node.output_payload["answer"], json!(""));
    assert_eq!(waiting_detail.flow_run.output_payload, json!({}));
    assert_eq!(waiting_detail.callback_tasks.len(), 1);
    assert_eq!(
        waiting_detail.callback_tasks[0].callback_kind,
        "llm_tool_calls"
    );
    assert_eq!(
        waiting_detail.callback_tasks[0].request_payload["tool_calls"][0]["id"],
        "call_weather"
    );
    let checkpoint = waiting_detail
        .checkpoints
        .last()
        .expect("llm tool wait should store checkpoint");
    assert_eq!(checkpoint.locator_payload["node_id"], "node-llm");
    assert_eq!(checkpoint.locator_payload["next_node_index"], json!(1));
    assert_eq!(
        checkpoint.variable_snapshot["node-llm"]["__llm_tool_callback"]["pending_tool_calls"][0]
            ["id"],
        "call_weather"
    );
}

#[tokio::test]
async fn complete_llm_tool_callback_resolves_final_llm_debug_refs() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderToolCall, ProviderUsage,
    };

    let service = OrchestrationRuntimeService::for_tests_with_provider_results(vec![
        ProviderInvocationResult {
            final_content: Some("need tool".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_weather".to_string(),
                name: "lookup_weather".to_string(),
                arguments: json!({ "city": "Shanghai" }),
                provider_metadata: json!({}),
            }],
            usage: ProviderUsage {
                input_tokens: Some(8),
                output_tokens: Some(4),
                total_tokens: Some(12),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        ProviderInvocationResult {
            final_content: Some("Shanghai is sunny".to_string()),
            usage: ProviderUsage {
                input_tokens: Some(11),
                output_tokens: Some(5),
                total_tokens: Some(16),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::Stop),
            ..ProviderInvocationResult::default()
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "天气？" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let waiting_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting_detail.callback_tasks[0].id,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call_weather",
                        "content": "sunny"
                    }
                ]
            }),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("Shanghai is sunny")
    );
    let presentation_text = service
        .list_runtime_events(completed.flow_run.id, 0)
        .await
        .into_iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| {
            event
                .payload
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<String>();
    assert!(
        presentation_text.ends_with("Shanghai is sunny"),
        "callback resume should append final Answer Presentation suffix: {presentation_text}"
    );

    let projections = service
        .list_context_projections(completed.flow_run.id)
        .await;
    let attempts = service
        .list_model_failover_attempt_ledger(completed.flow_run.id)
        .await;
    let llm_nodes = completed
        .node_runs
        .iter()
        .filter(|node_run| node_run.node_id == "node-llm")
        .collect::<Vec<_>>();

    assert_eq!(llm_nodes.len(), 2);
    for llm_node in llm_nodes {
        assert_no_pending_debug_ref(&llm_node.debug_payload);
    }
    let final_llm_node = completed
        .node_runs
        .iter()
        .filter(|node_run| node_run.node_id == "node-llm")
        .find(|node_run| node_run.output_payload["finish_reason"] == json!("stop"))
        .expect("final llm node run should be persisted");
    assert_resolved_llm_debug_refs(
        &final_llm_node.debug_payload,
        &projections,
        &attempts,
        final_llm_node.id,
    );
}

#[tokio::test]
async fn callback_resume_does_not_repeat_final_live_answer_presentation() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderStreamEvent, ProviderToolCall,
    };

    let tool_call = ProviderToolCall {
        id: "call_weather".to_string(),
        name: "lookup_weather".to_string(),
        arguments: json!({ "city": "Shanghai" }),
        provider_metadata: json!({}),
    };
    let final_answer = "NATIVE-CALLBACK-7319|Cargo.toml=不存在|AGENTS=# 记忆";
    let service = OrchestrationRuntimeService::for_tests_with_provider_outputs(vec![
        crate::ports::ProviderRuntimeInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: "我先检查文件。".to_string(),
                },
                ProviderStreamEvent::ToolCallCommit {
                    call: tool_call.clone(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::ToolCall,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some("我先检查文件。".to_string()),
                tool_calls: vec![tool_call],
                finish_reason: Some(ProviderFinishReason::ToolCall),
                ..ProviderInvocationResult::default()
            },
        },
        crate::ports::ProviderRuntimeInvocationOutput {
            events: vec![
                ProviderStreamEvent::TextDelta {
                    delta: final_answer.to_string(),
                },
                ProviderStreamEvent::Finish {
                    reason: ProviderFinishReason::Stop,
                },
            ],
            result: ProviderInvocationResult {
                final_content: Some(final_answer.to_string()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..ProviderInvocationResult::default()
            },
        },
    ]);
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let stream =
        std::sync::Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service = service.with_runtime_event_stream(stream.clone());
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "检查 Cargo.toml 和 AGENTS.md" }
            }),
            document_snapshot: None,
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

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({
                "tool_results": [{
                    "tool_call_id": "call_weather",
                    "content": "file inspection complete"
                }]
            }),
        })
        .await
        .unwrap();

    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!(final_answer)
    );
    let presentation_events = service
        .list_runtime_events(completed.flow_run.id, 0)
        .await
        .into_iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .collect::<Vec<_>>();
    let presentation_text = presentation_events
        .iter()
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    let expected_presentation = format!("我先检查文件。{final_answer}");
    assert_eq!(presentation_text, expected_presentation);
    assert_eq!(
        presentation_text.matches(final_answer).count(),
        1,
        "the callback resume final answer must be presented exactly once: {presentation_text}"
    );
    let final_presentation_events = presentation_events
        .iter()
        .filter(|event| event.payload["text"].as_str() == Some(final_answer))
        .collect::<Vec<_>>();
    assert_eq!(final_presentation_events.len(), 1);
    assert!(final_presentation_events[0].payload["presentation"]["source_node_run_id"].is_string());
    let streamed_text = stream
        .events()
        .into_iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| event.payload["text"].as_str().map(str::to_string))
        .collect::<String>();
    assert_eq!(streamed_text, expected_presentation);
    assert_eq!(
        streamed_text.matches(final_answer).count(),
        1,
        "the compatible SSE source must receive the callback final answer exactly once: {streamed_text}"
    );
}

#[tokio::test]
async fn callback_resume_waiting_again_projects_completed_answer_prefix_before_tool_call() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderToolCall, ProviderUsage,
    };

    let service = OrchestrationRuntimeService::for_tests_with_provider_results(vec![
        ProviderInvocationResult {
            final_content: Some("need first tool".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_weather".to_string(),
                name: "lookup_weather".to_string(),
                arguments: json!({ "city": "Shanghai" }),
                provider_metadata: json!({}),
            }],
            usage: ProviderUsage {
                total_tokens: Some(12),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        },
        ProviderInvocationResult {
            final_content: Some("LLM1 final".to_string()),
            usage: ProviderUsage {
                total_tokens: Some(16),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::Stop),
            ..ProviderInvocationResult::default()
        },
        ProviderInvocationResult {
            final_content: Some("need second tool".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_policy".to_string(),
                name: "lookup_policy".to_string(),
                arguments: json!({ "topic": "refund" }),
                provider_metadata: json!({}),
            }],
            usage: ProviderUsage {
                total_tokens: Some(20),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::ToolCall),
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
                "node-start": { "query": "天气？" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let first_waiting = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    assert_eq!(
        first_waiting.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );

    let second_waiting = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: first_waiting.callback_tasks[0].id,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call_weather",
                        "content": "sunny"
                    }
                ]
            }),
        })
        .await
        .unwrap();

    assert_eq!(
        second_waiting.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    let second_callback_task_id = second_waiting.callback_tasks.last().unwrap().id;
    let runtime_events = service
        .list_runtime_events(second_waiting.flow_run.id, 0)
        .await;
    let presentation_text = runtime_events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .filter_map(|event| event.payload["text"].as_str())
        .collect::<String>();
    assert_eq!(presentation_text, "LLM1 final\n----\n");
    let answer_node = second_waiting
        .node_runs
        .iter()
        .rev()
        .find(|node_run| node_run.node_id == "node-answer")
        .expect("waiting run should materialize Answer node with visible prefix");
    assert_eq!(answer_node.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(
        answer_node.output_payload["answer"],
        json!("LLM1 final\n----\n")
    );
    assert_eq!(
        second_waiting.flow_run.output_payload["answer"],
        json!("LLM1 final\n----\n")
    );

    let waiting_callback_sequence = runtime_events
        .iter()
        .find(|event| {
            event.event_type == "waiting_callback"
                && event.payload["callback_task_id"].as_str()
                    == Some(second_callback_task_id.to_string().as_str())
        })
        .expect("second waiting callback runtime event should be persisted")
        .sequence;
    let last_answer_sequence = runtime_events
        .iter()
        .filter(|event| event.event_type == "text_delta")
        .filter(|event| event.payload["presentation"]["kind"].as_str() == Some("answer"))
        .map(|event| event.sequence)
        .max()
        .expect("answer presentation delta should be persisted");
    assert!(
        last_answer_sequence < waiting_callback_sequence,
        "answer presentation should be durable before waiting callback"
    );
}

#[tokio::test]
async fn complete_callback_task_escapes_nul_characters_before_persisting_response() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_callback_run("Support Agent").await;

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: seeded.callback_task_id,
            response_payload: json!({ "result": "STDERR:\n\0after" }),
        })
        .await
        .unwrap();

    let callback_task = service
        .callback_task_for_tests(seeded.callback_task_id)
        .await;
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Completed);
    assert_eq!(
        callback_task.response_payload.as_ref().unwrap()["result"],
        json!("STDERR:\n\\u0000after")
    );
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        json!("STDERR:\n\\u0000after")
    );
}

#[tokio::test]
async fn complete_llm_tool_callback_rejects_partial_results_without_consuming_task() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderToolCall, ProviderUsage,
    };

    let service =
        OrchestrationRuntimeService::for_tests_with_provider_result(ProviderInvocationResult {
            final_content: Some("need tools".to_string()),
            tool_calls: vec![
                ProviderToolCall {
                    id: "call_weather".to_string(),
                    name: "lookup_weather".to_string(),
                    arguments: json!({ "city": "Shanghai" }),
                    provider_metadata: json!({}),
                },
                ProviderToolCall {
                    id: "call_time".to_string(),
                    name: "lookup_time".to_string(),
                    arguments: json!({ "city": "Shanghai" }),
                    provider_metadata: json!({}),
                },
            ],
            usage: ProviderUsage {
                total_tokens: Some(12),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        });
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "天气和时间？" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let waiting_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    let callback_task_id = waiting_detail.callback_tasks[0].id;

    let error = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call_weather",
                        "content": "{\"temperature\":21}"
                    }
                ]
            }),
        })
        .await
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("missing tool result for call_time"));
    let callback_task = service.callback_task_for_tests(callback_task_id).await;
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
}

#[tokio::test]
async fn complete_llm_tool_callback_rejects_wrong_application_without_consuming_task() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderToolCall, ProviderUsage,
    };

    let service =
        OrchestrationRuntimeService::for_tests_with_provider_result(ProviderInvocationResult {
            final_content: Some("need tool".to_string()),
            tool_calls: vec![ProviderToolCall {
                id: "call_weather".to_string(),
                name: "lookup_weather".to_string(),
                arguments: json!({ "city": "Shanghai" }),
                provider_metadata: json!({}),
            }],
            usage: ProviderUsage {
                total_tokens: Some(12),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::ToolCall),
            ..ProviderInvocationResult::default()
        });
    let owner = service.seed_application_with_flow("Owner Agent").await;
    let intruder = service.seed_application_with_flow("Intruder Agent").await;
    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: owner.actor_user_id,
            application_id: owner.application_id,
            input_payload: json!({
                "node-start": { "query": "天气？" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let waiting_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: owner.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    let callback_task_id = waiting_detail.callback_tasks[0].id;

    let error = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: intruder.actor_user_id,
            application_id: intruder.application_id,
            callback_task_id,
            response_payload: json!({
                "tool_results": [
                    {
                        "tool_call_id": "call_weather",
                        "content": "{\"temperature\":21}"
                    }
                ]
            }),
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("flow run not found"));
    let callback_task = service.callback_task_for_tests(callback_task_id).await;
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);
}

#[tokio::test]
async fn live_debug_checkpoint_snapshot_stores_llm_output_metrics_without_process_events() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderStreamEvent, ProviderToolCall, ProviderUsage,
    };

    let service = OrchestrationRuntimeService::for_tests_with_provider_events(vec![
        ProviderStreamEvent::TextDelta {
            delta: "visible output".to_string(),
        },
        ProviderStreamEvent::ToolCallCommit {
            call: ProviderToolCall {
                id: "tool-call-1".to_string(),
                name: "lookup_policy".to_string(),
                arguments: json!({ "query": "refund" }),
                provider_metadata: json!({}),
            },
        },
        ProviderStreamEvent::UsageSnapshot {
            usage: ProviderUsage {
                input_tokens: Some(5),
                output_tokens: Some(7),
                reasoning_tokens: None,
                input_cache_hit_tokens: None,
                input_cache_miss_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                total_tokens: Some(12),
            },
        },
        ProviderStreamEvent::Finish {
            reason: ProviderFinishReason::Stop,
        },
    ]);
    let seeded = service
        .seed_application_with_human_input_flow("Support Agent")
        .await;

    let detail = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "请总结退款政策" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();

    let waiting_detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(
        waiting_detail.flow_run.status,
        domain::FlowRunStatus::WaitingHuman
    );
    assert_eq!(waiting_detail.flow_run.output_payload, json!({}));
    let llm_node = node_run(&waiting_detail, "node-llm");
    assert_eq!(
        llm_node.output_payload["text"],
        json!("echo:gpt-5.4-mini:请总结退款政策")
    );
    assert_eq!(
        llm_node.output_payload["usage"],
        llm_node.metrics_payload["usage"]
    );
    assert!(llm_node.output_payload.get("route").is_none());
    assert!(llm_node.output_payload.get("provider_route").is_some());
    assert!(llm_node.metrics_payload.get("usage").is_some());

    let snapshot = &waiting_detail
        .checkpoints
        .last()
        .expect("waiting human checkpoint should be stored")
        .variable_snapshot;
    let llm_snapshot = snapshot
        .get("node-llm")
        .expect("llm output should be available to waiting node");
    assert_eq!(
        llm_snapshot["text"],
        json!("echo:gpt-5.4-mini:请总结退款政策")
    );
    assert_eq!(llm_snapshot["usage"]["total_tokens"], json!(12));
    for hidden_key in [
        "tool_calls",
        "error",
        "__context_projection_id",
        "__attempt_ids",
    ] {
        assert!(
            llm_node.output_payload.get(hidden_key).is_none(),
            "{hidden_key} must not be persisted in node output"
        );
        assert!(
            llm_snapshot.get(hidden_key).is_none(),
            "{hidden_key} must not be persisted in checkpoint variables"
        );
    }
}
