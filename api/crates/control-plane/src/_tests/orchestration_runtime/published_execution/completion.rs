use super::*;
use crate::{
    orchestration_runtime::{
        OrchestrationRuntimeService, StartFlowDebugRunCommand, StartPublishedFlowRunCommand,
    },
    ports::{CreateFlowRunInput, RuntimeEventStream},
};
use domain::{FlowRunMode, FlowRunStatus};
use serde_json::json;
use std::time::Duration;

// Compile through the real service, then admit a distinct published business root.
macro_rules! published_fixture {
    () => {{
        let stream = Arc::new(crate::_tests::RecordingRuntimeEventStream::default());
        let service = Arc::new(
            OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone()),
        );
        let seeded = service.seed_application_with_flow("Published root completion").await;
        let debug = service.start_flow_debug_run(StartFlowDebugRunCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            input_payload: json!({"node-start": {"query": "hello"}}),
            document_snapshot: None,
            debug_session_id: None,
        }).await.unwrap().flow_run;
        let run = service.repository.create_flow_run(&CreateFlowRunInput {
            application_run_log_context: None,
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            flow_id: seeded.flow_id,
            flow_draft_id: debug.draft_id,
            compiled_plan_id: debug.compiled_plan_id.unwrap(),
            debug_session_id: String::new(),
            flow_schema_version: debug.flow_schema_version,
            document_hash: debug.document_hash,
            run_mode: FlowRunMode::PublishedApiRun,
            target_node_id: None,
            title: "Published root completion".into(),
            status: FlowRunStatus::Queued,
            input_payload: debug.input_payload,
            started_at: time::OffsetDateTime::now_utc(),
            api_key_id: Some(Uuid::now_v7()),
            publication_version_id: Some(Uuid::now_v7()),
            assistant_conversation_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        }).await.unwrap();
        (service, run, stream)
    }};
}

fn command(run: &domain::FlowRunRecord) -> StartPublishedFlowRunCommand {
    StartPublishedFlowRunCommand {
        application_id: run.application_id,
        flow_run_id: run.id,
        provider_transport_slot: None,
        transport_connection_scope: None,
        observation_context: None,
    }
}

async fn entered(gate: &tokio::sync::Notify) {
    tokio::time::timeout(Duration::from_secs(5), gate.notified())
        .await
        .unwrap();
}

#[tokio::test]
async fn published_execution_completion_dropped_admitted_root_fails_once() {
    let (service, run, stream) = published_fixture!();
    let (gate, _release) = service.repository.gate_next_runtime_event_append();
    let root = tokio::spawn({
        let service = service.clone();
        let command = command(&run);
        async move { service.start_published_flow_run(command).await }
    });
    entered(&gate).await;
    root.abort();
    assert!(root.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(5), async {
        while !stream.is_closed(run.id) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let failed = service
        .repository
        .get_flow_run(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, FlowRunStatus::Failed);
    assert_eq!(
        failed.error_payload.unwrap()["code"],
        "stream_terminal_missing"
    );
    assert_eq!(
        service
            .list_runtime_events(run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
}

#[tokio::test]
async fn published_execution_completion_running_reentry_does_not_own_cleanup() {
    let (service, run, stream) = published_fixture!();
    service
        .force_flow_run_status(run.id, FlowRunStatus::Running)
        .await;
    let (gate, _release) = service.repository.gate_next_runtime_event_append();
    let root = tokio::spawn({
        let service = service.clone();
        let command = command(&run);
        async move { service.start_published_flow_run(command).await }
    });
    entered(&gate).await;
    root.abort();
    assert!(root.await.unwrap_err().is_cancelled());
    // Give an erroneously spawned cleanup task an opportunity to run.
    tokio::task::yield_now().await;
    assert_eq!(
        service
            .repository
            .get_flow_run(run.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Running
    );
    assert!(!stream.is_closed(run.id));
    assert!(service
        .list_runtime_events(run.id, 0)
        .await
        .iter()
        .all(|event| event.event_type != "flow_failed"));
}

#[tokio::test]
async fn published_execution_completion_observer_disconnect_keeps_root_alive() {
    let (service, run, stream) = published_fixture!();
    let subscription = stream.subscribe(run.id, None).await.unwrap();
    let (gate, release) = service.repository.gate_llm_selection_append(false);
    let root = tokio::spawn({
        let service = service.clone();
        let command = command(&run);
        async move { service.start_published_flow_run(command).await }
    });
    entered(&gate).await;
    drop(subscription);
    assert!(!root.is_finished());
    assert_eq!(
        service
            .repository
            .get_flow_run(run.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Running
    );
    release.notify_one();
    let detail = tokio::time::timeout(Duration::from_secs(5), root)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
    assert!(service
        .list_runtime_events(run.id, 0)
        .await
        .iter()
        .all(|event| event.event_type != "flow_failed"));
}

#[tokio::test]
async fn published_execution_completion_preserves_terminal_and_waiting_winners() {
    for status in [
        FlowRunStatus::Succeeded,
        FlowRunStatus::Incomplete,
        FlowRunStatus::Failed,
        FlowRunStatus::Cancelled,
        FlowRunStatus::WaitingCallback,
        FlowRunStatus::WaitingHuman,
    ] {
        let (service, run, stream) = published_fixture!();
        service.force_flow_run_status(run.id, status).await;
        let before = service
            .repository
            .get_flow_run(run.application_id, run.id)
            .await
            .unwrap()
            .unwrap();
        let mut guard = PublishedExecutionCompletionGuard::new(
            Handle::current(),
            service.repository.clone(),
            Some(stream.clone()),
            run.application_id,
            run.id,
        );
        // Await the exact independently owned cleanup to make no-op waiting winners deterministic.
        let winner = guard.completion.take().unwrap().await.unwrap();
        assert_eq!(winner, before);
        assert!(service
            .list_runtime_events(run.id, 0)
            .await
            .iter()
            .all(|event| event.event_type != "flow_failed"));
        if matches!(
            status,
            FlowRunStatus::WaitingCallback | FlowRunStatus::WaitingHuman
        ) {
            assert!(!stream.is_closed(run.id));
        }
    }
}

#[tokio::test]
async fn published_execution_completion_losing_initial_cas_does_not_arm() {
    let (service, run, stream) = published_fixture!();
    service
        .force_flow_run_status_before_next_flow_update(run.id, FlowRunStatus::Running)
        .await;
    let detail = service
        .start_published_flow_run(command(&run))
        .await
        .unwrap();
    tokio::task::yield_now().await;
    assert_eq!(detail.flow_run.status, FlowRunStatus::Running);
    assert_eq!(
        service
            .repository
            .get_flow_run(run.application_id, run.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        FlowRunStatus::Running
    );
    assert!(!stream.is_closed(run.id));
    assert!(service
        .list_runtime_events(run.id, 0)
        .await
        .iter()
        .all(|event| event.event_type != "flow_failed"));
}

#[tokio::test]
async fn published_execution_completion_panicking_root_fails_once() {
    let (service, run, stream) = published_fixture!();
    let (gate, _release) = service.repository.gate_next_runtime_event_append();
    let (panic_sender, panic_receiver) = tokio::sync::oneshot::channel::<()>();
    let root = tokio::spawn({
        let service = service.clone();
        let command = command(&run);
        async move {
            let execution = service.start_published_flow_run(command);
            tokio::pin!(execution);
            tokio::select! {
                result = &mut execution => { result.unwrap(); }
                _ = panic_receiver => panic!("injected business root panic"),
            }
        }
    });
    entered(&gate).await;
    panic_sender.send(()).unwrap();
    assert!(root.await.unwrap_err().is_panic());
    tokio::time::timeout(Duration::from_secs(5), async {
        while !stream.is_closed(run.id) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let failed = service
        .repository
        .get_flow_run(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, FlowRunStatus::Failed);
    assert_eq!(
        failed.error_payload.unwrap()["code"],
        "stream_terminal_missing"
    );
    assert_eq!(
        service
            .list_runtime_events(run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
}

#[tokio::test]
async fn published_execution_completion_active_llm_abort_finishes_running_node() {
    let (service, run, stream) = published_fixture!();
    let (gate, _release) = service.repository.gate_llm_selection_append(false);
    let root = tokio::spawn({
        let service = service.clone();
        let command = command(&run);
        async move { service.start_published_flow_run(command).await }
    });
    entered(&gate).await;
    let active = service
        .repository
        .get_application_run_detail(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.flow_run.status, FlowRunStatus::Running);
    let running_nodes: Vec<_> = active
        .node_runs
        .iter()
        .filter(|node| node.status == domain::NodeRunStatus::Running)
        .map(|node| node.id)
        .collect();
    assert!(
        !running_nodes.is_empty(),
        "fixture must interrupt a persisted Running node"
    );
    root.abort();
    assert!(root.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(5), async {
        while !stream.is_closed(run.id) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let failed = service
        .repository
        .get_application_run_detail(run.application_id, run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.flow_run.status, FlowRunStatus::Failed);
    assert_eq!(
        failed.flow_run.error_payload.unwrap()["code"],
        "stream_terminal_missing"
    );
    assert_eq!(
        service
            .list_runtime_events(run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    for node_id in running_nodes {
        let node = failed
            .node_runs
            .iter()
            .find(|node| node.id == node_id)
            .expect("interrupted node remains in durable detail");
        assert!(
            matches!(
                node.status,
                domain::NodeRunStatus::Failed | domain::NodeRunStatus::Cancelled
            ),
            "interrupted Running node must reach a failure terminal, got {:?}",
            node.status
        );
        assert!(
            node.finished_at.is_some(),
            "interrupted node must record completion time"
        );
    }
}
