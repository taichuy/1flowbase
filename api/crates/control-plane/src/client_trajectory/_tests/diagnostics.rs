use super::*;
use super::super::diagnostics::{Diagnostics, Outcome, Stage};

#[test]
fn cumulative_diagnostics_count_sum_max_and_failure_are_independent() {
    let diagnostics = Diagnostics::default();
    diagnostics.record_ns(Stage::ArchiveCommit, 10, Outcome::Success);
    diagnostics.record_ns(Stage::ArchiveCommit, 30, Outcome::Failure);
    diagnostics.record_ns(Stage::ArchiveCommit, 20, Outcome::Cancelled);
    let stage = diagnostics.stage_snapshot(Stage::ArchiveCommit);
    assert_eq!(stage.count, 3);
    assert_eq!(stage.total_ns, 60);
    assert_eq!(stage.max_ns, 30);
    assert_eq!(stage.failures, 1);
    assert_eq!(stage.cancelled, 1);
    assert_eq!(diagnostics.stage_snapshot(Stage::FactAppend).count, 0);
}

#[test]
fn cumulative_diagnostics_saturate_instead_of_wrapping() {
    let diagnostics = Diagnostics::default();
    diagnostics.record_ns(Stage::ReplayRead, u64::MAX, Outcome::Success);
    diagnostics.record_ns(Stage::ReplayRead, 1, Outcome::Success);
    assert_eq!(diagnostics.stage_snapshot(Stage::ReplayRead).total_ns, u64::MAX);
}

#[test]
fn cumulative_diagnostics_cancelled_span_is_not_success() {
    let diagnostics = Diagnostics::default();
    drop(diagnostics.start(Stage::ResponseSseAdmission));
    let stage = diagnostics.stage_snapshot(Stage::ResponseSseAdmission);
    assert_eq!(stage.count, 1);
    assert_eq!(stage.cancelled, 1);
    assert_eq!(stage.failures, 0);
    assert_eq!(diagnostics.clock_reads(), 2);
}

#[test]
fn cumulative_diagnostics_concurrent_snapshots_keep_counter_invariants() {
    let diagnostics = Diagnostics::default();
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                for ns in 1..=2000 {
                    diagnostics.record_ns(Stage::CompleteWait, ns, Outcome::Failure);
                }
            });
        }
        for _ in 0..2000 {
            let snapshot = diagnostics.stage_snapshot(Stage::CompleteWait);
            assert!(snapshot.max_ns <= snapshot.total_ns);
            assert!(snapshot.failures + snapshot.cancelled <= snapshot.count);
            assert!(snapshot.count > 0 || snapshot.total_ns == 0);
        }
    });
    let snapshot = diagnostics.stage_snapshot(Stage::CompleteWait);
    assert_eq!(snapshot.count, 4000);
    assert_eq!(snapshot.failures, 4000);
    assert_eq!(snapshot.total_ns, 4_002_000);
    assert_eq!(snapshot.max_ns, 2000);
}

#[test]
fn cumulative_diagnostics_payload_is_ids_and_fixed_numeric_counters_only() {
    let diagnostics = Diagnostics::default();
    diagnostics.record_ns(Stage::RequestAdmission, 42, Outcome::Success);
    let value = diagnostics.payload("worker_end", Uuid::nil(), None, true);
    assert_eq!(value["version"], 1);
    assert_eq!(value["capture_id"], Uuid::nil().to_string());
    assert!(value["flow_run_id"].is_null());
    assert_eq!(value["stages"]["request_admission"]["total_ns"], 42);
    assert_eq!(value.as_object().unwrap().len(), 7);
    assert!(serde_json::to_vec(&value).unwrap().len() < 4096);
    assert_eq!(value["stages"].as_object().unwrap().len(), Stage::COUNT);
    for stage in value["stages"].as_object().unwrap().values() {
        assert_eq!(stage.as_object().unwrap().len(), 5);
        assert!(stage.as_object().unwrap().values().all(serde_json::Value::is_u64));
    }
}

#[tokio::test]
async fn cumulative_diagnostics_disabled_has_no_state_and_preserves_receipt() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, false,
    );
    assert!(recorder.owner.state.diagnostics.is_none());
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}").await.unwrap();
    assert_eq!(recorder.complete().await.unwrap().persisted_through, 1);
    assert_eq!(writer.frames.lock().unwrap()[0].bytes, b"{}");
}

#[tokio::test]
async fn cumulative_diagnostics_enabled_preserves_raw_and_unbound_cleanup() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, true,
    );
    let diagnostics = recorder.owner.state.diagnostics.clone().unwrap();
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}\0").await.unwrap();
    recorder.record(ClientTrajectoryFrameKind::ResponseSse, b"data: [DONE]\n\n").await.unwrap();
    assert_eq!(recorder.complete().await.unwrap().persisted_through, 0);
    assert_eq!(writer.cleanup_count.load(Ordering::Relaxed), 1);
    assert!(writer.frames.lock().unwrap().is_empty());
    assert_eq!(diagnostics.stage_snapshot(Stage::RequestAdmission).count, 1);
    assert_eq!(diagnostics.stage_snapshot(Stage::ResponseSseAdmission).count, 1);
    assert_eq!(diagnostics.stage_snapshot(Stage::CompleteWait).count, 1);
    assert_eq!(diagnostics.stage_snapshot(Stage::UnboundDiscard).count, 1);
}

#[tokio::test]
async fn cumulative_diagnostics_projection_error_still_fails_complete_and_keeps_raw() {
    let writer = Arc::new(MemoryWriter::default());
    writer.fail_once.store(true, Ordering::Release);
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, true,
    );
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}").await.unwrap();
    assert!(recorder.complete().await.is_err());
    assert_eq!(writer.frames.lock().unwrap()[0].bytes, b"{}");
    let metrics = recorder.owner.state.diagnostics.as_ref().unwrap();
    assert_eq!(metrics.stage_snapshot(Stage::FactAppend).failures, 1);
    assert_eq!(metrics.stage_snapshot(Stage::WorkerTotal).failures, 1);
    assert_eq!(metrics.stage_snapshot(Stage::CompleteWait).failures, 1);
    assert!(metrics.stage_snapshot(Stage::RequestClassifyInclusive).count > 0);
}
