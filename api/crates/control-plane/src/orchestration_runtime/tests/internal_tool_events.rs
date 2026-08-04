use super::*;

#[tokio::test]
async fn orchestration_runtime_persists_visible_internal_llm_tool_route_events() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let now = OffsetDateTime::now_utc();
    let flow_run = OrchestrationRuntimeRepository::create_flow_run(
        &repository,
        &crate::ports::CreateFlowRunInput {
            actor_user_id: Uuid::nil(),
            application_id: Uuid::nil(),
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: "debug-session".to_string(),
            flow_schema_version: "1".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "debug flow".to_string(),
            status: domain::FlowRunStatus::Running,
            input_payload: json!({}),
            started_at: now,
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        },
    )
    .await
    .expect("flow run should be created");
    let outcome = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![NodeExecutionTrace {
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "Main LLM".to_string(),
            input_payload: json!({}),
            output_payload: json!({ "text": "done" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({
                "visible_internal_llm_tool_events": [
                    {
                        "event_type": "visible_internal_llm_tool_started",
                        "main_node_id": "node-llm",
                        "target_node_id": "node-mounted-llm",
                        "tool_name": "image_llm",
                        "tool_call_id": "call_visible",
                        "arguments": { "task": "describe image" }
                    },
                    {
                        "event_type": "visible_internal_llm_tool_completed",
                        "main_node_id": "node-llm",
                        "target_node_id": "node-mounted-llm",
                        "tool_name": "image_llm",
                        "tool_call_id": "call_visible",
                        "provider_route": { "model": "gpt-5.4-mini" }
                    }
                ]
            }),
            provider_events: Vec::new(),
        }],
    };

    persist_flow_debug_outcome(
        &repository,
        PersistFlowDebugOutcomeInput {
            scope_id: Uuid::nil(),
            application_name: "fixture application",
            task_queue: None,
            application_id: flow_run.application_id,
            flow_run: &flow_run,
            compiled_plan: None,
            outcome: &outcome,
            prepared_node_runs: None,
            answer_presentation: None,
            trigger_event_type: "flow_run_started",
            trigger_event_payload: json!({}),
            base_started_at: now,
            waiting_node_resume: None,
        },
    )
    .await
    .expect("debug outcome should persist");

    let runtime_events =
        OrchestrationRuntimeRepository::list_runtime_events(&repository, flow_run.id, 0)
            .await
            .expect("runtime events should be listed");
    assert!(runtime_events.iter().any(|event| {
        event.event_type == "visible_internal_llm_tool_started"
            && event.node_run_id.is_some()
            && event.payload["node_id"] == json!("node-llm")
            && event.payload["tool_name"] == json!("image_llm")
            && event.payload["arguments"]["task"] == json!("describe image")
    }));
    assert!(runtime_events.iter().any(|event| {
        event.event_type == "visible_internal_llm_tool_completed"
            && event.payload["target_node_id"] == json!("node-mounted-llm")
            && event.payload["provider_route"]["model"] == json!("gpt-5.4-mini")
    }));
}
