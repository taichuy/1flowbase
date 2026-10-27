use crate::orchestration_runtime::{
    live_debug_run::fail_flow_run, CancelFlowRunCommand, CompleteCallbackTaskCommand,
    ContinueFlowDebugRunCommand, OrchestrationRuntimeService, StartFlowDebugRunCommand,
};
use crate::ports::OrchestrationRuntimeRepository;
use domain::{CallbackTaskStatus, FlowRunStatus, NodeRunStatus};
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationResult, ProviderToolCall,
};
use serde_json::json;
use uuid::Uuid;

// These tests invoke production persistence, continuation, exception cleanup and
// callback service methods. The repository fixture only injects read failures;
// the real PostgreSQL transaction boundary has a separate starvation fixture.
#[tokio::test]
async fn waiting_commit_receipt_survives_upper_run_and_detail_read_failures_and_resumes() {
    for detail_failure in [false, true] {
        let service = OrchestrationRuntimeService::for_tests_with_provider_results(vec![
            ProviderInvocationResult {
                tool_calls: vec![ProviderToolCall {
                    id: "call-recover".into(),
                    name: "lookup_weather".into(),
                    arguments: json!({"city":"Shanghai"}),
                    provider_metadata: json!({}),
                }],
                finish_reason: Some(ProviderFinishReason::ToolCall),
                ..Default::default()
            },
            ProviderInvocationResult {
                final_content: Some("Shanghai is sunny".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
        ]);
        let seeded = service
            .seed_application_with_flow("waiting commit receipt")
            .await;
        let started = service
            .start_flow_debug_run(StartFlowDebugRunCommand {
                actor_user_id: seeded.actor_user_id,
                application_id: seeded.application_id,
                input_payload: json!({"node-start":{"query":"weather"}}),
                document_snapshot: None,
                debug_session_id: None,
            })
            .await
            .unwrap();
        service
            .repository
            .inject_waiting_read_fault(!detail_failure, detail_failure);
        let result = service
            .continue_flow_debug_run(ContinueFlowDebugRunCommand {
                application_id: seeded.application_id,
                flow_run_id: started.flow_run.id,
                workspace_id: Uuid::nil(),
            })
            .await;
        service.repository.inject_waiting_read_fault(false, false);
        let waiting = result.expect("a committed wait must survive a projection read error");
        assert_eq!(waiting.flow_run.status, FlowRunStatus::WaitingCallback);
        assert!(waiting.flow_run.error_payload.is_none());
        assert_eq!(waiting.callback_tasks.len(), 1);
        assert_eq!(
            waiting.callback_tasks[0].status,
            CallbackTaskStatus::Pending
        );
        assert_eq!(
            waiting.checkpoints.last().unwrap().status,
            "waiting_callback"
        );
        assert!(waiting
            .node_runs
            .iter()
            .any(|node| node.status == NodeRunStatus::WaitingCallback));
        assert!(!waiting
            .events
            .iter()
            .any(|event| event.event_type == "flow_run_failed"));
        let completed = service.complete_callback_task(CompleteCallbackTaskCommand {
            responses_continuation: None,
            transport_connection_scope: None,
            observation_context: None,
            native_transport: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: waiting.callback_tasks[0].id,
            response_payload: json!({"tool_results":[{"tool_call_id":"call-recover","content":"sunny"}]}),
        }).await.unwrap();
        assert_eq!(completed.flow_run.status, FlowRunStatus::Succeeded);
        assert_eq!(
            completed.flow_run.output_payload["answer"],
            "Shanghai is sunny"
        );
        assert_eq!(
            completed.callback_tasks[0].status,
            CallbackTaskStatus::Completed
        );
        assert_eq!(
            completed
                .events
                .iter()
                .filter(|event| event.event_type == "flow_run_completed")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn waiting_commit_precommit_exception_still_fails_its_owned_running_revision() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("owned failure").await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"hello"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    let node = service
        .repository
        .create_node_run(&crate::ports::CreateNodeRunInput {
            flow_run_id: started.flow_run.id,
            node_id: "completed-node".into(),
            node_type: "start".into(),
            node_alias: "Start".into(),
            status: NodeRunStatus::Running,
            input_payload: json!({}),
            debug_payload: json!({}),
            started_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    service
        .repository
        .update_node_run(&crate::ports::UpdateNodeRunInput {
            node_run_id: node.id,
            status: NodeRunStatus::Succeeded,
            output_payload: json!({}),
            error_payload: None,
            metrics_payload: json!({}),
            debug_payload: json!({}),
            finished_at: Some(time::OffsetDateTime::now_utc()),
        })
        .await
        .unwrap();
    let current = service
        .repository
        .get_flow_run(seeded.application_id, started.flow_run.id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(current.updated_at, started.flow_run.updated_at);
    let failed = fail_flow_run(
        &service,
        &started.flow_run,
        &anyhow::anyhow!("precommit fault"),
    )
    .await
    .unwrap();
    assert_eq!(failed.flow_run.status, FlowRunStatus::Failed);
    assert_eq!(
        failed.flow_run.error_payload.unwrap()["message"],
        "precommit fault"
    );
    assert!(failed.callback_tasks.is_empty());
    assert!(failed.checkpoints.is_empty());
}

#[tokio::test]
async fn waiting_commit_stale_exception_preserves_independent_cancel() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("cancel fence").await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"hello"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
        })
        .await
        .unwrap();
    let detail = fail_flow_run(
        &service,
        &started.flow_run,
        &anyhow::anyhow!("late read failure"),
    )
    .await
    .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Cancelled);
    assert_eq!(
        detail
            .events
            .iter()
            .filter(|event| event.event_type == "flow_run_cancelled")
            .count(),
        1
    );
    assert!(!detail
        .events
        .iter()
        .any(|event| event.event_type == "flow_run_failed"));
    assert_eq!(
        service
            .repository
            .get_flow_run(seeded.application_id, started.flow_run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Cancelled
    );
}

#[tokio::test]
async fn waiting_commit_initial_read_failure_still_fails_only_the_unhanded_initial_execution() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("initial read failure")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start":{"query":"hello"}}),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    service.repository.fail_initial_run_read_once();
    let failed = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    assert_eq!(failed.flow_run.status, FlowRunStatus::Failed);
    assert_eq!(
        failed.flow_run.error_payload.unwrap()["message"],
        "injected initial run read failure"
    );
    assert!(failed.node_runs.is_empty());
    assert!(failed.callback_tasks.is_empty());
    assert!(failed.checkpoints.is_empty());
    assert_eq!(
        failed
            .events
            .iter()
            .filter(|event| event.event_type == "flow_run_failed")
            .count(),
        1
    );
}
