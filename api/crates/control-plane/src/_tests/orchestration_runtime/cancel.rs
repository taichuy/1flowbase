use control_plane::errors::ControlPlaneError;
use control_plane::orchestration_runtime::{
    CancelFlowRunCommand, ContinueFlowDebugRunCommand, OrchestrationRuntimeService,
    StartFlowDebugRunCommand,
};
use control_plane::ports::RuntimeEventCloseReason;
use domain::FlowRunStatus;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn cancel_flow_run_marks_running_debug_run_as_cancelled() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let started = service
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
    service
        .force_flow_run_status(started.flow_run.id, FlowRunStatus::Running)
        .await;

    let detail = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
        })
        .await
        .unwrap();

    assert_eq!(detail.flow_run.status, FlowRunStatus::Cancelled);
    assert!(detail
        .events
        .iter()
        .any(|event| event.event_type == "flow_run_cancelled"));
}

#[tokio::test]
async fn cancel_flow_run_emits_cancelled_runtime_terminal_event_and_closes_stream() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let started = service
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
    service
        .force_flow_run_status(started.flow_run.id, FlowRunStatus::Running)
        .await;

    service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
        })
        .await
        .unwrap();

    let events = stream.events();
    assert!(events.iter().any(|event| {
        event.event_type == "flow_cancelled"
            && event.payload["type"] == "flow_cancelled"
            && event.payload["status"] == "cancelled"
    }));
    assert!(!events.iter().any(|event| event.event_type == "flow_failed"));
    assert_eq!(
        stream.close_calls(),
        vec![(started.flow_run.id, RuntimeEventCloseReason::Cancelled)]
    );
}

#[tokio::test]
async fn cancel_flow_run_projects_succeeded_winner_after_stale_read() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service.seed_application_with_flow("Support Agent").await;
    let started = service
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
    service
        .force_flow_run_status(started.flow_run.id, FlowRunStatus::Running)
        .await;
    service
        .force_flow_run_status_after_next_get(started.flow_run.id, FlowRunStatus::Succeeded)
        .await;

    let detail = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
        })
        .await
        .unwrap();

    assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
    assert!(!detail
        .events
        .iter()
        .any(|event| event.event_type == "flow_run_cancelled"));
    assert!(!stream
        .events()
        .iter()
        .any(|event| event.event_type == "flow_cancelled"));
    assert!(stream
        .events()
        .iter()
        .any(|event| event.event_type == "flow_finished"));
    assert_eq!(
        stream.close_calls(),
        vec![(started.flow_run.id, RuntimeEventCloseReason::Finished)]
    );
}

#[tokio::test]
async fn continue_flow_debug_run_does_not_overwrite_cancelled_run_before_waiting_callback_write() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_callback_flow("Support Agent")
        .await;
    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({
                "node-start": { "query": "order_123" }
            }),
            document_snapshot: None,
            debug_session_id: None,
        })
        .await
        .unwrap();
    service
        .force_flow_run_status_before_next_flow_update(
            started.flow_run.id,
            FlowRunStatus::Cancelled,
        )
        .await;

    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();

    assert_eq!(detail.flow_run.status, FlowRunStatus::Cancelled);
    assert!(!stream
        .events()
        .iter()
        .any(|event| event.event_type == "waiting_callback"));
    assert!(stream.close_calls().is_empty());
}

#[tokio::test]
async fn cancel_flow_run_rejects_terminal_status() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_human_input_flow("Support Agent")
        .await;
    let started = service
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
    let detail = service
        .continue_flow_debug_run(ContinueFlowDebugRunCommand {
            application_id: seeded.application_id,
            flow_run_id: started.flow_run.id,
            workspace_id: Uuid::nil(),
        })
        .await
        .unwrap();
    service
        .force_flow_run_status(detail.flow_run.id, FlowRunStatus::Succeeded)
        .await;

    let error = service
        .cancel_flow_run(CancelFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: detail.flow_run.id,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidStateTransition { resource, from, to, .. })
            if *resource == "flow_run" && from == "succeeded" && to == "cancelled"
    ));
}
