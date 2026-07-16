use control_plane::errors::ControlPlaneError;
use control_plane::orchestration_runtime::{
    CompleteCallbackTaskCommand, ContinueFlowDebugRunCommand, OrchestrationRuntimeService,
    ResumeFlowRunCommand, StartFlowDebugRunCommand,
};
use domain::FlowRunStatus;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn continue_flow_debug_run_stops_at_human_input_and_persists_waiting_state() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_human_input_flow("Support Agent")
        .await;

    let started = service
        .start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({ "node-start": { "query": "请总结退款政策" } }),
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

    assert_eq!(detail.flow_run.run_mode.as_str(), "debug_flow_run");
    assert_eq!(detail.flow_run.status.as_str(), "waiting_human");
    let human_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-human")
        .expect("human node run should exist");
    assert_eq!(human_node.status.as_str(), "waiting_human");
    let answer_node = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-answer")
        .expect("answer node should be materialized while waiting");
    assert_eq!(answer_node.status.as_str(), "succeeded");
    assert_eq!(detail.checkpoints.len(), 1);
}

#[tokio::test]
async fn resume_flow_run_with_human_input_finishes_downstream_answer_node() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_human_run("Support Agent").await;

    let detail = service
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: seeded.flow_run_id,
            checkpoint_id: seeded.checkpoint_id,
            input_payload: json!({ "node-human": { "input": "已审核通过" } }),
        })
        .await
        .unwrap();

    assert_eq!(detail.flow_run.status.as_str(), "succeeded");
    let resumed_answer = detail
        .node_runs
        .iter()
        .find(|node_run| {
            node_run.node_id == "node-answer"
                && node_run.output_payload["answer"] == json!("已审核通过")
        })
        .expect("resume should execute the final answer node");
    assert_eq!(resumed_answer.node_id, "node-answer");
    assert_eq!(
        detail.flow_run.output_payload["answer"],
        json!("已审核通过")
    );
    let runtime_events = service.list_runtime_events(detail.flow_run.id, 0).await;
    let resumed_answer_started = runtime_events.iter().find(|event| {
        event.event_type == "node_started"
            && event.payload["node_run_id"] == json!(resumed_answer.id)
    });
    assert!(
        resumed_answer_started.is_some(),
        "resumed answer node {} must be observable before execution; node runs: {:?}; events: {:?}",
        resumed_answer.id,
        detail
            .node_runs
            .iter()
            .map(|node| (&node.node_id, node.id, node.started_at))
            .collect::<Vec<_>>(),
        runtime_events
            .iter()
            .map(|event| (&event.event_type, event.payload.get("node_run_id")))
            .collect::<Vec<_>>()
    );
    let waiting_answer = detail
        .node_runs
        .iter()
        .find(|node_run| node_run.node_id == "node-answer" && node_run.id != resumed_answer.id)
        .expect("waiting state should materialize a partial answer node");
    assert!(
        waiting_answer.started_at <= resumed_answer.started_at,
        "waiting answer timestamp must precede resumed execution"
    );
}

#[tokio::test]
async fn complete_callback_task_updates_task_and_requeues_waiting_run() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_callback_run("Support Agent").await;

    let detail = service
        .complete_callback_task(CompleteCallbackTaskCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            callback_task_id: seeded.callback_task_id,
            response_payload: json!({ "result": { "status": "ok" } }),
        })
        .await
        .unwrap();

    assert_eq!(detail.callback_tasks[0].status.as_str(), "completed");
    assert_eq!(detail.flow_run.status.as_str(), "succeeded");
    let runtime_events = service.list_runtime_events(detail.flow_run.id, 0).await;
    let runtime_event_types = runtime_events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(
        runtime_event_types.contains(&"flow_finished"),
        "callback resume completion should be durable: {runtime_event_types:?}"
    );
    let resumed_answer = detail
        .node_runs
        .iter()
        .find(|node_run| {
            node_run.node_id == "node-answer"
                && node_run.output_payload == detail.flow_run.output_payload
        })
        .expect("callback resume should execute answer node");
    assert!(
        runtime_events.iter().any(|event| {
            event.event_type == "node_started"
                && event.payload["node_run_id"] == json!(resumed_answer.id)
        }),
        "callback-resumed answer node must be observable before execution"
    );
}

#[tokio::test]
async fn resume_flow_run_rejects_terminal_flow_status_transition() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service.seed_waiting_human_run("Support Agent").await;
    service
        .force_flow_run_status(seeded.flow_run_id, FlowRunStatus::Succeeded)
        .await;

    let error = service
        .resume_flow_run(ResumeFlowRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_run_id: seeded.flow_run_id,
            checkpoint_id: seeded.checkpoint_id,
            input_payload: json!({ "node-human": { "input": "已审核通过" } }),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidStateTransition { resource, from, to, .. })
            if *resource == "flow_run" && from == "succeeded" && to == "succeeded"
    ));
}
