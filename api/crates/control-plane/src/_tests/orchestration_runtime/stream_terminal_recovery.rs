use std::sync::Arc;

use control_plane::{
    orchestration_runtime::{
        FinalizePublishedRunMissingStreamTerminalCommand, OrchestrationRuntimeService,
    },
    ports::{
        AppendTerminalIfMissingAndCloseOutcome, RuntimeEventCloseReason, RuntimeEventStream,
        RuntimeEventStreamPolicy,
    },
};
use domain::FlowRunStatus;
use serde_json::json;

#[tokio::test]
async fn d2_ac_008_missing_terminal_fails_once_and_preserves_partial_output() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Stream terminal recovery")
        .await;
    let partial_output = json!({ "answer": "partial answer", "segments": ["partial answer"] });
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, partial_output.clone())
        .await;

    let first = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("missing terminal should finalize the published run");
    let second = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("repeat EOF finalization should return the existing terminal winner");

    assert_eq!(first.status, FlowRunStatus::Failed);
    assert_eq!(first.output_payload, partial_output);
    assert_eq!(
        first.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(second, first);
    assert_eq!(
        service
            .list_runtime_events(flow_run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)]
    );
}

#[tokio::test]
async fn d2_ac_008_eof_cas_reloads_a_concurrent_succeeded_winner() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Stream terminal race winner")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial answer" }))
        .await;
    service
        .force_flow_run_status_before_next_flow_update(flow_run.id, FlowRunStatus::Succeeded)
        .await;

    let winner = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("CAS miss should reload the legitimate terminal winner");

    assert_eq!(winner.status, FlowRunStatus::Succeeded);
    assert!(winner.error_payload.is_none());
    assert!(service
        .list_runtime_events(flow_run.id, 0)
        .await
        .iter()
        .all(|event| event.event_type != "flow_failed"));
    assert!(stream
        .events()
        .iter()
        .any(|event| event.event_type == "flow_finished"));
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Finished)]
    );
}

#[tokio::test]
async fn d2_ac_008_eof_finalizes_a_queued_published_run_once() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Queued stream terminal recovery")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "queued partial" }))
        .await;
    service
        .force_flow_run_status(flow_run.id, FlowRunStatus::Queued)
        .await;

    let recovered = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("queued published run should become a durable EOF failure");

    assert_eq!(recovered.status, FlowRunStatus::Failed);
    assert_eq!(
        recovered.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(
        service
            .list_runtime_events(flow_run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        service
            .list_run_events(flow_run.id)
            .iter()
            .filter(|event| event.event_type == "flow_run_failed")
            .count(),
        1
    );
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
}

#[tokio::test]
async fn d2_ac_008_eof_keeps_existing_terminal_or_waiting_winners() {
    for status in [
        FlowRunStatus::Succeeded,
        FlowRunStatus::Incomplete,
        FlowRunStatus::Failed,
        FlowRunStatus::Cancelled,
        FlowRunStatus::WaitingCallback,
        FlowRunStatus::WaitingHuman,
    ] {
        let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
        let service =
            OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
        let seeded = service.seed_application_with_flow("Terminal winner").await;
        let flow_run = service
            .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
            .await;
        service.force_flow_run_status(flow_run.id, status).await;

        let winner = service
            .finalize_published_run_missing_stream_terminal(
                FinalizePublishedRunMissingStreamTerminalCommand {
                    application_id: seeded.application_id,
                    flow_run_id: flow_run.id,
                },
            )
            .await
            .expect("an existing terminal or waiting winner must remain readable");

        assert_eq!(winner.status, status);
        assert!(
            service
                .list_runtime_events(flow_run.id, 0)
                .await
                .iter()
                .all(|event| event.event_type != "flow_failed"),
            "unexpected EOF failure for {status:?}"
        );
        let expected_terminal = match status {
            FlowRunStatus::Succeeded => Some(("flow_finished", RuntimeEventCloseReason::Finished)),
            FlowRunStatus::Incomplete => {
                Some(("flow_incomplete", RuntimeEventCloseReason::Incomplete))
            }
            FlowRunStatus::Failed => Some(("flow_failed", RuntimeEventCloseReason::Failed)),
            FlowRunStatus::Cancelled => {
                Some(("flow_cancelled", RuntimeEventCloseReason::Cancelled))
            }
            FlowRunStatus::WaitingCallback | FlowRunStatus::WaitingHuman => None,
            other => panic!("unexpected fixture status: {other:?}"),
        };
        match expected_terminal {
            Some((event_type, reason)) => {
                assert_eq!(
                    stream
                        .events()
                        .last()
                        .map(|event| event.event_type.as_str()),
                    Some(event_type)
                );
                assert_eq!(stream.close_calls(), vec![(flow_run.id, reason)]);
            }
            None => {
                assert!(stream.events().is_empty());
                assert!(stream.close_calls().is_empty());
            }
        }
    }
}

#[tokio::test]
async fn d2_ac_008_eof_rejects_paused_and_non_published_runs() {
    let service = OrchestrationRuntimeService::for_tests();
    let seeded = service
        .seed_application_with_flow("Invalid stream terminal recovery")
        .await;
    let paused = service
        .seed_published_running_run_with_output(&seeded, json!({}))
        .await;
    service
        .force_flow_run_status(paused.id, FlowRunStatus::Paused)
        .await;
    let paused_error = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: paused.id,
            },
        )
        .await
        .expect_err("paused run is not a legal EOF recovery source");
    assert!(paused_error
        .to_string()
        .contains("cannot finalize a paused published API run"));

    let debug_run = service
        .seed_published_running_run_with_output(&seeded, json!({}))
        .await;
    service
        .force_flow_run_mode(debug_run.id, domain::FlowRunMode::DebugFlowRun)
        .await;
    let mode_error = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: debug_run.id,
            },
        )
        .await
        .expect_err("non-published run is outside the recovery boundary");
    assert!(mode_error
        .to_string()
        .contains("only accepts published API runs"));
}

#[tokio::test]
async fn d2_ac_008_eof_persistence_failure_rolls_back_and_retry_writes_one_terminal() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Atomic stream terminal recovery")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    service.fail_next_runtime_event_append().await;

    let first_error = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect_err("a durable terminal persistence failure must reach the caller");
    assert!(first_error
        .to_string()
        .contains("simulated runtime event append failure"));

    let recovered = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("retry must atomically write the previously missing terminal");

    assert_eq!(recovered.status, FlowRunStatus::Failed);
    assert_eq!(
        recovered.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(
        service
            .list_runtime_events(flow_run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        service
            .list_run_events(flow_run.id)
            .iter()
            .filter(|event| event.event_type == "flow_run_failed")
            .count(),
        1
    );
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)]
    );
}

#[tokio::test]
async fn d2_ac_008_eof_post_commit_projection_warning_keeps_terminal_and_live_publish() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Post-commit EOF projection warning")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    service
        .fail_next_published_stream_terminal_projection()
        .await;

    let recovered = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("a post-commit projection warning must not suppress the EOF terminal");

    assert_eq!(recovered.status, FlowRunStatus::Failed);
    assert_eq!(
        recovered.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(
        service
            .list_run_events(flow_run.id)
            .iter()
            .filter(|event| event.event_type == "flow_run_failed")
            .count(),
        1
    );
    assert_eq!(
        service
            .list_runtime_events(flow_run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)]
    );
}

#[tokio::test]
async fn d2_ac_008_eof_retries_transient_live_append_without_duplicate_terminal() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Retry transient EOF live append")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    stream.fail_next_append();

    let first_error = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect_err("a transient live append failure must remain retryable");
    assert!(first_error
        .to_string()
        .contains("simulated runtime event stream append failure"));
    assert!(
        !stream.is_closed(flow_run.id),
        "append failure must leave the stream open for retry"
    );

    let recovered = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("stable EOF failure should retry the missing live terminal");

    assert_eq!(recovered.status, FlowRunStatus::Failed);
    assert_eq!(
        service
            .list_runtime_events(flow_run.id, 0)
            .await
            .iter()
            .filter(|event| event.event_type == "flow_failed")
            .count(),
        1
    );
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.run_id == flow_run.id && event.event_type == "flow_failed")
            .count(),
        1
    );
    assert!(stream.is_closed(flow_run.id));
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)]
    );
}

#[tokio::test]
async fn d2_ac_008_eof_cas_miss_reensures_a_stable_failure_live_terminal() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("CAS miss EOF terminal retry")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    service
        .force_stream_terminal_failure_before_next_flow_update(flow_run.id)
        .await;

    let winner = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("CAS miss should reensure another recovery's stable EOF failure terminal");

    assert_eq!(winner.status, FlowRunStatus::Failed);
    assert_eq!(
        winner.error_payload,
        Some(json!({
            "code": "stream_terminal_missing",
            "message": "runtime event stream ended without a terminal event"
        }))
    );
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.run_id == flow_run.id && event.event_type == "flow_failed")
            .count(),
        1
    );
    assert!(stream.is_closed(flow_run.id));
}

#[tokio::test]
async fn d2_ac_008_concurrent_stable_eof_retries_append_one_live_terminal_and_close_once() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Concurrent stable EOF terminal recovery")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    stream.fail_next_append();
    let initial_error = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect_err("seed a stable durable failure without a live terminal");
    assert!(initial_error
        .to_string()
        .contains("simulated runtime event stream append failure"));
    stream.synchronize_next_appends(2);

    let left = service.finalize_published_run_missing_stream_terminal(
        FinalizePublishedRunMissingStreamTerminalCommand {
            application_id: seeded.application_id,
            flow_run_id: flow_run.id,
        },
    );
    let right = service.finalize_published_run_missing_stream_terminal(
        FinalizePublishedRunMissingStreamTerminalCommand {
            application_id: seeded.application_id,
            flow_run_id: flow_run.id,
        },
    );
    let (left, right) = tokio::join!(left, right);

    assert!(left.is_ok(), "left recovery failed: {left:?}");
    assert!(right.is_ok(), "right recovery failed: {right:?}");
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.run_id == flow_run.id && event.event_type == "flow_failed")
            .count(),
        1,
        "concurrent retries must claim one live terminal"
    );
    assert!(stream.is_closed(flow_run.id));
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)],
        "concurrent retries must close the stream once"
    );
    let durable_terminals = service
        .list_runtime_events(flow_run.id, 0)
        .await
        .into_iter()
        .filter(|event| event.event_type == "flow_failed")
        .collect::<Vec<_>>();
    let live_terminals = stream
        .events()
        .into_iter()
        .filter(|event| event.run_id == flow_run.id && event.event_type == "flow_failed")
        .collect::<Vec<_>>();
    assert_eq!(durable_terminals.len(), 1);
    assert_eq!(live_terminals.len(), 1);
    assert_eq!(
        live_terminals[0].payload, durable_terminals[0].payload,
        "live retry and replay must project the same durable winner payload"
    );
}

#[tokio::test]
async fn d2_ac_008_existing_terminal_closure_uses_retained_terminal_semantics() {
    let stream = crate::_tests::support::RecordingRuntimeEventStream::default();
    let flow_run_id = uuid::Uuid::now_v7();
    stream
        .open_run(flow_run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .expect("open the recording stream");
    stream
        .append(
            flow_run_id,
            control_plane::orchestration_runtime::debug_stream_events::flow_finished(
                flow_run_id,
                json!({ "answer": "already complete" }),
            ),
        )
        .await
        .expect("seed the retained terminal");

    let outcome = stream
        .append_terminal_if_missing_and_close(
            flow_run_id,
            control_plane::orchestration_runtime::debug_stream_events::flow_failed(
                flow_run_id,
                json!({ "message": "incoming recovery failure must not rewrite terminal" }),
            ),
        )
        .await
        .expect("an existing terminal should only be closed");

    assert_eq!(
        outcome,
        AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal
    );
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run_id, RuntimeEventCloseReason::Finished)],
        "closure must derive from the retained terminal, not the incoming recovery event"
    );
}

#[tokio::test]
async fn d2_ac_008_stable_eof_recovery_closes_an_existing_unclosed_live_terminal() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Existing unclosed EOF terminal")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    stream.fail_next_append();
    let _ = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect_err("seed a stable durable failure without a live terminal");
    stream
        .append(
            flow_run.id,
            control_plane::orchestration_runtime::debug_stream_events::flow_failed(
                flow_run.id,
                json!({
                    "code": "stream_terminal_missing",
                    "message": "runtime event stream ended without a terminal event"
                }),
            ),
        )
        .await
        .expect("seed an already published but unclosed live terminal");

    let recovered = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("recovery must close an already published terminal");

    assert_eq!(recovered.status, FlowRunStatus::Failed);
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.run_id == flow_run.id && event.event_type == "flow_failed")
            .count(),
        1
    );
    assert!(stream.is_closed(flow_run.id));
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)]
    );
}

#[tokio::test]
async fn d2_ac_008_eof_retries_close_after_live_terminal_was_already_published() {
    let stream = Arc::new(crate::_tests::support::RecordingRuntimeEventStream::default());
    let service =
        OrchestrationRuntimeService::for_tests().with_runtime_event_stream(stream.clone());
    let seeded = service
        .seed_application_with_flow("Retry transient EOF live close")
        .await;
    let flow_run = service
        .seed_published_running_run_with_output(&seeded, json!({ "answer": "partial" }))
        .await;
    stream.fail_next_close();

    let first_error = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect_err("a transient live close failure must reach the caller");
    assert!(first_error
        .to_string()
        .contains("simulated runtime event stream close failure"));

    let recovered = service
        .finalize_published_run_missing_stream_terminal(
            FinalizePublishedRunMissingStreamTerminalCommand {
                application_id: seeded.application_id,
                flow_run_id: flow_run.id,
            },
        )
        .await
        .expect("retry must close the already published live terminal");

    assert_eq!(recovered.status, FlowRunStatus::Failed);
    assert_eq!(
        stream
            .events()
            .iter()
            .filter(|event| event.run_id == flow_run.id && event.event_type == "flow_failed")
            .count(),
        1
    );
    assert!(stream.is_closed(flow_run.id));
    assert_eq!(
        stream.close_calls(),
        vec![(flow_run.id, RuntimeEventCloseReason::Failed)]
    );
}
