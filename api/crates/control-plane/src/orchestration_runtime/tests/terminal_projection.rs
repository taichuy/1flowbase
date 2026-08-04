use super::*;

/// #1366 AC-001 / AC-005: a cancelled partial result remains the only durable terminal when a
/// stale successful execution finishes later.
#[tokio::test]
async fn late_success_projects_cancelled_winner_without_success_terminal_or_answer_history() {
    let repository = test_support::InMemoryOrchestrationRuntimeRepository::with_permissions(vec![]);
    let now = OffsetDateTime::now_utc();
    let running = OrchestrationRuntimeRepository::create_flow_run(
        &repository,
        &crate::ports::CreateFlowRunInput {
            actor_user_id: Uuid::nil(),
            application_id: Uuid::nil(),
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: "late-success".to_string(),
            flow_schema_version: "1".to_string(),
            document_hash: "hash".to_string(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "late success".to_string(),
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
    .expect("running flow should be created");
    let stale_partial = OrchestrationRuntimeRepository::update_flow_run(
        &repository,
        &crate::ports::UpdateFlowRunInput {
            flow_run_id: running.id,
            status: domain::FlowRunStatus::Running,
            output_payload: json!({ "answer": "partial" }),
            error_payload: None,
            finished_at: None,
        },
    )
    .await
    .expect("partial result should be durable before cancellation");
    let cancel_event = debug_stream_events::flow_cancelled(running.id);
    let cancelled = OrchestrationRuntimeRepository::commit_flow_run_terminal(
        &repository,
        &crate::ports::CommitFlowRunTerminalInput {
            flow_run_id: running.id,
            expected_status: domain::FlowRunStatus::Running,
            result: crate::ports::CommitFlowRunTerminalResult::Cancelled {
                output_payload: stale_partial.output_payload.clone(),
                error_payload: None,
            },
            flow_run_event_payload: json!({ "reason": "manual_stop" }),
            terminal_event_payload: cancel_event.payload,
            finished_at: now,
        },
    )
    .await
    .expect("cancellation commit should succeed");
    assert!(matches!(
        cancelled,
        crate::ports::CommitFlowRunTerminalReceipt::Winner(_)
            | crate::ports::CommitFlowRunTerminalReceipt::WinnerWithPostCommitProjectionWarning(_)
    ));

    let late_success = FlowDebugExecutionOutcome {
        stop_reason: ExecutionStopReason::Completed,
        variable_pool: Map::new(),
        checkpoint_snapshot: None,
        operation_terminal: None,
        node_traces: vec![NodeExecutionTrace {
            node_id: "node-answer".to_string(),
            node_type: "answer".to_string(),
            node_alias: "Answer".to_string(),
            input_payload: json!({}),
            output_payload: json!({ "answer": "late success must lose" }),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            provider_events: Vec::new(),
        }],
    };
    let projected = persist_flow_debug_outcome(
        &repository,
        PersistFlowDebugOutcomeInput {
            scope_id: Uuid::nil(),
            application_name: "fixture application",
            task_queue: None,
            application_id: running.application_id,
            flow_run: &stale_partial,
            compiled_plan: None,
            outcome: &late_success,
            prepared_node_runs: None,
            answer_presentation: None,
            trigger_event_type: "flow_run_execution_finished_late",
            trigger_event_payload: json!({}),
            base_started_at: now,
            waiting_node_resume: None,
        },
    )
    .await
    .expect("late completion should resolve the durable cancellation winner");

    assert_eq!(projected.flow_run.status, domain::FlowRunStatus::Cancelled);
    assert!(projected.stream_events.is_empty());
    assert_eq!(
        projected
            .terminal_event
            .as_ref()
            .map(|event| event.event_type.as_str()),
        Some("flow_cancelled")
    );
    let runtime_events =
        OrchestrationRuntimeRepository::list_runtime_events(&repository, running.id, 0)
            .await
            .expect("runtime replay should be readable");
    assert_eq!(
        runtime_events
            .iter()
            .filter(|event| event.event_type == "flow_cancelled")
            .count(),
        1
    );
    assert!(runtime_events
        .iter()
        .all(|event| event.event_type != "flow_finished" && event.event_type != "text_delta"));
    let detail = OrchestrationRuntimeRepository::get_application_run_detail(
        &repository,
        running.application_id,
        running.id,
    )
    .await
    .expect("run detail should load")
    .expect("run detail should exist");
    assert!(detail
        .events
        .iter()
        .all(|event| event.event_type != "flow_run_completed"));
}
