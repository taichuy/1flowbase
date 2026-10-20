use super::*;

#[tokio::test]
async fn complete_callback_task_preserves_nul_in_response_and_execution() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_callback_run("Support Agent").await;

    let payload = json!({
        "result": "STDERR:\n\0after\\u0000"
    });
    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: seeded.callback_task_id,
            response_payload: payload.clone(),
        })
        .await
        .unwrap();

    let callback_task = service
        .callback_task_for_tests(seeded.callback_task_id)
        .await;
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Completed);
    assert_eq!(callback_task.response_payload.as_ref().unwrap(), &payload);
    assert_eq!(
        completed.flow_run.output_payload["answer"],
        payload["result"]
    );
}

#[tokio::test]
async fn complete_llm_tool_callback_accepts_partial_replay_conflict_and_single_advancement() {
    use plugin_framework::provider_contract::{
        ProviderFinishReason, ProviderInvocationResult, ProviderToolCall, ProviderUsage,
    };

    let service = OrchestrationRuntimeService::for_tests_with_provider_results(vec![
        ProviderInvocationResult {
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
        },
        ProviderInvocationResult {
            final_content: Some("done".to_string()),
            usage: ProviderUsage {
                total_tokens: Some(8),
                ..ProviderUsage::default()
            },
            finish_reason: Some(ProviderFinishReason::Stop),
            ..ProviderInvocationResult::default()
        },
    ]);
    let service = std::sync::Arc::new(service);
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

    let empty_results = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id,
            response_payload: json!({ "tool_results": [] }),
        })
        .await
        .unwrap_err();
    assert_eq!(
        empty_results.downcast_ref::<crate::errors::ControlPlaneError>(),
        Some(&crate::errors::ControlPlaneError::InvalidInput(
            "tool_results"
        ))
    );
    assert_eq!(
        service
            .callback_task_for_tests(callback_task_id)
            .await
            .status,
        domain::CallbackTaskStatus::Pending
    );

    let partial_payload = json!({
        "tool_results": [
            {
                "tool_call_id": "call_weather",
                "content": "{\"temperature\":21}"
            }
        ]
    });
    let partial = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id,
            response_payload: partial_payload.clone(),
        })
        .await
        .unwrap();

    assert_eq!(
        partial.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );
    let callback_task = service.callback_task_for_tests(callback_task_id).await;
    assert_eq!(callback_task.status, domain::CallbackTaskStatus::Pending);

    let replay = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id,
            response_payload: partial_payload,
        })
        .await
        .unwrap();
    assert_eq!(
        replay.flow_run.status,
        domain::FlowRunStatus::WaitingCallback
    );

    let conflict = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id,
            response_payload: json!({
                "tool_results": [{
                    "tool_call_id": "call_weather",
                    "content": "different"
                }]
            }),
        })
        .await
        .unwrap_err();
    assert!(conflict
        .to_string()
        .contains("tool_callback_result_conflict"));

    let final_payload = json!({
        "tool_results": [{
            "tool_call_id": "call_time",
            "content": "12:00"
        }]
    });
    let command = || CompleteCallbackTaskCommand {
        responses_continuation: None,
        transport_connection_scope: None,
        observation_context: None,
        native_transport: None,
        actor_user_id: seeded.actor_user_id,
        application_id: seeded.application_id,
        callback_task_id,
        response_payload: final_payload.clone(),
    };
    let left_service = std::sync::Arc::clone(&service);
    let right_service = std::sync::Arc::clone(&service);
    let left_command = command();
    let right_command = command();
    let left = tokio::spawn(async move { left_service.complete_callback_task(left_command).await });
    let right =
        tokio::spawn(async move { right_service.complete_callback_task(right_command).await });
    let left = left.await.unwrap().unwrap();
    let right = right.await.unwrap().unwrap();
    assert!(matches!(
        (left.flow_run.status, right.flow_run.status),
        (
            domain::FlowRunStatus::Succeeded,
            domain::FlowRunStatus::WaitingCallback
        ) | (
            domain::FlowRunStatus::WaitingCallback,
            domain::FlowRunStatus::Succeeded
        ) | (
            domain::FlowRunStatus::Succeeded,
            domain::FlowRunStatus::Succeeded
        )
    ));

    let completed = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id,
            response_payload: final_payload,
        })
        .await
        .unwrap();
    assert_eq!(completed.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(
        completed
            .node_runs
            .iter()
            .filter(|run| run.node_id == "node-llm")
            .count(),
        2
    );
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
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
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
                cache_write_by_ttl_seconds: None,
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

    let checkpoint = waiting_detail
        .checkpoints
        .last()
        .expect("waiting human checkpoint should be stored");
    let snapshot = service.checkpoint_snapshot_for_tests(checkpoint).await;
    let llm_snapshot = snapshot
        .variable_pool
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
