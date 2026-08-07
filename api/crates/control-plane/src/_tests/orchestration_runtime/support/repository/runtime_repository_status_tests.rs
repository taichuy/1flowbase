use super::*;

#[tokio::test]
async fn fail_queued_flow_run_shell_does_not_fail_attached_run() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![
        "application.view.all",
        "application.create.all",
    ]);
    let now = OffsetDateTime::now_utc();
    let flow_run = repository
        .create_flow_run(&CreateFlowRunInput {
            actor_user_id: Uuid::now_v7(),
            application_id: Uuid::now_v7(),
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: "test-debug-session".to_string(),
            flow_schema_version: "1flowbase.flow/v2".to_string(),
            document_hash: "test-document-hash".to_string(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "Untitled run".to_string(),
            status: domain::FlowRunStatus::Running,
            input_payload: json!({}),
            started_at: now,
            api_key_id: None,
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        })
        .await
        .unwrap();

    let failed = repository
        .fail_queued_flow_run_shell(&crate::ports::FailQueuedFlowRunShellInput {
            flow_run_id: flow_run.id,
            output_payload: json!({}),
            error_payload: json!({ "message": "prepare failed" }),
            finished_at: now,
        })
        .await
        .unwrap();

    assert!(failed.is_none());
    let unchanged = repository
        .get_flow_run(flow_run.application_id, flow_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.status, domain::FlowRunStatus::Running);
    assert_eq!(unchanged.compiled_plan_id, flow_run.compiled_plan_id);
    assert!(unchanged.error_payload.is_none());
}

#[tokio::test]
async fn update_flow_run_if_status_does_not_overwrite_cancelled_run() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![
        "application.view.all",
        "application.create.all",
    ]);
    let now = OffsetDateTime::now_utc();
    let flow_run = repository
        .create_flow_run(&CreateFlowRunInput {
            actor_user_id: Uuid::now_v7(),
            application_id: Uuid::now_v7(),
            flow_id: Uuid::now_v7(),
            flow_draft_id: Uuid::now_v7(),
            compiled_plan_id: Uuid::now_v7(),
            debug_session_id: "test-debug-session".to_string(),
            flow_schema_version: "1flowbase.flow/v2".to_string(),
            document_hash: "test-document-hash".to_string(),
            run_mode: domain::FlowRunMode::DebugFlowRun,
            target_node_id: None,
            title: "Untitled run".to_string(),
            status: domain::FlowRunStatus::Running,
            input_payload: json!({}),
            started_at: now,
            api_key_id: None,
            publication_version_id: None,
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        })
        .await
        .unwrap();

    repository.force_flow_run_status(flow_run.id, domain::FlowRunStatus::Cancelled);

    let updated = repository
        .update_flow_run_if_status(
            &UpdateFlowRunInput {
                flow_run_id: flow_run.id,
                status: domain::FlowRunStatus::Succeeded,
                output_payload: json!({ "answer": "done" }),
                error_payload: None,
                finished_at: Some(now),
            },
            domain::FlowRunStatus::Running,
        )
        .await
        .unwrap();

    assert!(updated.is_none());
    let unchanged = repository
        .get_flow_run(flow_run.application_id, flow_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.status, domain::FlowRunStatus::Cancelled);
    assert_eq!(unchanged.output_payload, json!({}));
}

#[tokio::test]
async fn update_flow_run_if_status_returns_not_found_for_missing_run() {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![
        "application.view.all",
        "application.create.all",
    ]);

    let error = repository
        .update_flow_run_if_status(
            &UpdateFlowRunInput {
                flow_run_id: Uuid::now_v7(),
                status: domain::FlowRunStatus::Succeeded,
                output_payload: json!({ "answer": "done" }),
                error_payload: None,
                finished_at: Some(OffsetDateTime::now_utc()),
            },
            domain::FlowRunStatus::Running,
        )
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::NotFound("flow_run"))
    ));
}
