use super::*;

#[derive(Default)]
struct SlowFactWriter {
    inner: MemoryWriter,
    entered: Notify,
    release: Notify,
    held: AtomicBool,
}
#[async_trait::async_trait]
impl FactWriter for SlowFactWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        self.inner.append(input).await
    }
    async fn archive(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> anyhow::Result<ClientTrajectoryArchiveReceipt> {
        if !self.held.swap(true, Ordering::AcqRel) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.inner.archive(input).await
    }
    async fn discard_unbound(&self, request: Uuid) -> anyhow::Result<()> {
        self.inner.discard_unbound(request).await
    }
    async fn replay(
        &self,
        request: Uuid,
        cursor: i64,
    ) -> anyhow::Result<Vec<ClientTrajectoryArchiveFrame>> {
        self.inner.replay(request, cursor).await
    }
}

#[tokio::test]
async fn tiny_fragment_burst_waits_for_slow_archive_and_preserves_every_frame() {
    let writer = Arc::new(SlowFactWriter::default());
    let recorder =
        ClientTrajectoryRecorder::with_writer(writer.clone(), ClientTrajectoryTransport::Websocket);
    recorder
        .record(ClientTrajectoryFrameKind::Request, b"{}")
        .await
        .unwrap();
    writer.entered.notified().await;
    assert!(recorder.bind_run(Uuid::now_v7(), None));
    let producer = recorder.clone();
    let task = tokio::spawn(async move {
        for byte in 0..64u8 {
            producer
                .record(ClientTrajectoryFrameKind::ResponseJson, &[byte])
                .await
                .unwrap();
        }
        producer.complete().await.unwrap()
    });
    tokio::task::yield_now().await;
    assert!(
        !task.is_finished(),
        "bounded queue must backpressure while durable writer is blocked"
    );
    writer.release.notify_one();
    let receipt = task.await.unwrap();
    assert_eq!(receipt.persisted_through, 65);
    let frames = writer.inner.frames.lock().unwrap();
    assert_eq!(frames.len(), 65);
    for (i, f) in frames.iter().skip(1).enumerate() {
        assert_eq!(f.bytes, vec![i as u8]);
        assert_eq!(f.sequence, i as i64 + 2);
    }
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn complete_waits_for_durable_commit() {
    let writer = Arc::new(SlowFactWriter::default());
    let recorder =
        ClientTrajectoryRecorder::with_writer(writer.clone(), ClientTrajectoryTransport::Http);
    recorder
        .record(ClientTrajectoryFrameKind::Request, b"{}")
        .await
        .unwrap();
    writer.entered.notified().await;
    assert!(recorder.bind_run(Uuid::now_v7(), None));
    let capture = recorder.clone();
    let completion = tokio::spawn(async move { capture.complete().await });
    tokio::task::yield_now().await;
    assert!(!completion.is_finished());
    writer.release.notify_one();
    assert_eq!(completion.await.unwrap().unwrap().persisted_through, 1);
}

#[tokio::test]
async fn complete_exposes_projection_failure_while_raw_remains_durable() {
    let writer = Arc::new(MemoryWriter::default());
    writer.fail_once.store(true, Ordering::Release);
    let recorder = capture(writer.clone());
    recorder.bind_run(Uuid::now_v7(), None);
    recorder
        .record(ClientTrajectoryFrameKind::Request, b"{}")
        .await
        .unwrap();
    assert!(recorder.complete().await.is_err());
    assert_eq!(writer.frames.lock().unwrap()[0].bytes, b"{}");
}

#[tokio::test]
async fn final_unbound_owner_completion_releases_prebind_bytes_and_closes_binding() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder
        .record(ClientTrajectoryFrameKind::Request, b"{\"rejected\":true}")
        .await
        .unwrap();
    let receipt = recorder.complete().await.unwrap();
    assert_eq!(
        receipt.persisted_through, 0,
        "cleaned archive cannot claim retained bytes"
    );
    assert!(writer.frames.lock().unwrap().is_empty());
    assert!(writer.records.lock().unwrap().is_empty());
    assert_eq!(writer.cleanup_count.load(Ordering::Relaxed), 1);
    assert!(!recorder.bind_run(Uuid::now_v7(), None));
}

#[tokio::test]
async fn final_unbound_cleanup_failure_reaches_complete_owner() {
    let writer = Arc::new(MemoryWriter::default());
    writer.fail_cleanup_once.store(true, Ordering::Release);
    let recorder = capture(writer.clone());
    recorder
        .record(ClientTrajectoryFrameKind::Request, b"{}")
        .await
        .unwrap();
    assert!(recorder
        .complete()
        .await
        .unwrap_err()
        .to_string()
        .contains("fixture cleanup failed"));
    assert_eq!(writer.frames.lock().unwrap()[0].bytes, b"{}");
    assert!(!recorder.bind_run(Uuid::now_v7(), None));
}

#[tokio::test]
async fn late_binding_during_admitted_frame_drain_preserves_exact_bound_archive() {
    let writer = Arc::new(SlowFactWriter::default());
    let recorder =
        ClientTrajectoryRecorder::with_writer(writer.clone(), ClientTrajectoryTransport::Http);
    let bytes = b" {\"input\":\"late bind\"} ";
    recorder
        .record(ClientTrajectoryFrameKind::Request, bytes)
        .await
        .unwrap();
    writer.entered.notified().await;
    recorder.finish();
    assert!(
        recorder.bind_run(Uuid::now_v7(), None),
        "binding remains open until admitted frames drain"
    );
    writer.release.notify_one();
    let receipt = recorder.complete().await.unwrap();
    assert_eq!(receipt.persisted_through, 1);
    assert_eq!(writer.inner.frames.lock().unwrap()[0].bytes, bytes);
    assert_eq!(writer.inner.cleanup_count.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn cumulative_diagnostics_keeps_admission_backpressure_and_durable_complete_wait() {
    use super::super::diagnostics::Stage;
    let writer = Arc::new(SlowFactWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, true,
    );
    let metrics = recorder.owner.state.diagnostics.clone().unwrap();
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}").await.unwrap();
    writer.entered.notified().await;
    let producer = recorder.clone();
    let task = tokio::spawn(async move {
        for byte in 0..64u8 {
            producer.record(ClientTrajectoryFrameKind::ResponseJson, &[byte]).await.unwrap();
        }
        producer.complete().await
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished(), "diagnostics must not bypass the original bounded admission");
    writer.release.notify_one();
    assert_eq!(task.await.unwrap().unwrap().persisted_through, 65);
    let frames = writer.inner.frames.lock().unwrap();
    assert_eq!(frames.len(), 65);
    for (index, frame) in frames.iter().skip(1).enumerate() {
        assert_eq!(frame.bytes, vec![index as u8]);
        assert_eq!(frame.sequence, index as i64 + 2);
    }
    assert_eq!(metrics.stage_snapshot(Stage::ResponseJsonAdmission).count, 64);
    assert_eq!(metrics.stage_snapshot(Stage::CompleteWait).count, 1);
    assert_eq!(metrics.stage_snapshot(Stage::ArchiveCommit).count, writer.inner.archive_calls.load(Ordering::Relaxed));
    assert_eq!(metrics.stage_snapshot(Stage::ArchiveCommit).failures, 0);
}

// Send the handshake only after the real inner future has been polled Pending.
// A scheduler yield alone would not prove that the diagnostic span exists.
async fn signal_first_pending<F: std::future::Future>(
    future: F,
    started: tokio::sync::oneshot::Sender<()>,
) -> F::Output {
    use std::future::Future;
    tokio::pin!(future);
    let mut started = Some(started);
    std::future::poll_fn(move |context| {
        let result = future.as_mut().poll(context);
        if result.is_pending() {
            if let Some(started) = started.take() {
                let _ = started.send(());
            }
        }
        result
    })
    .await
}

#[tokio::test]
async fn cumulative_diagnostics_cancelling_pending_record_keeps_only_admitted_frames() {
    use super::super::diagnostics::Stage;
    let writer = Arc::new(SlowFactWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, true,
    );
    let metrics = recorder.owner.state.diagnostics.clone().unwrap();
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}").await.unwrap();
    writer.entered.notified().await;
    for _ in 0..QUEUE_RECORDS {
        recorder.record(ClientTrajectoryFrameKind::ResponseSse, b"x").await.unwrap();
    }
    let producer = recorder.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let pending = tokio::spawn(async move {
        signal_first_pending(
            producer.record(ClientTrajectoryFrameKind::ResponseSse, b"not admitted"),
            started,
        )
        .await
    });
    ready.await.expect("record future was polled Pending");
    assert!(!pending.is_finished());
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert_eq!(metrics.stage_snapshot(Stage::ResponseSseAdmission).cancelled, 1);
    writer.release.notify_one();
    assert_eq!(recorder.complete().await.unwrap().persisted_through, QUEUE_RECORDS as i64 + 1);
    let frames = writer.inner.frames.lock().unwrap();
    assert_eq!(frames.len(), QUEUE_RECORDS + 1);
    assert!(frames.iter().all(|frame| frame.bytes != b"not admitted"));
}

#[tokio::test]
async fn cumulative_diagnostics_cancelling_complete_does_not_cancel_durable_worker() {
    use super::super::diagnostics::Stage;
    let writer = Arc::new(SlowFactWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, true,
    );
    let metrics = recorder.owner.state.diagnostics.clone().unwrap();
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}").await.unwrap();
    writer.entered.notified().await;
    let owner = recorder.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let pending = tokio::spawn(async move {
        signal_first_pending(owner.complete(), started).await
    });
    ready.await.expect("complete future was polled Pending");
    assert!(!pending.is_finished());
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    assert_eq!(metrics.stage_snapshot(Stage::CompleteWait).cancelled, 1);
    assert!(!recorder.owner.state.stopped.load(Ordering::Acquire));
    writer.release.notify_one();
    assert_eq!(recorder.complete().await.unwrap().persisted_through, 1);
    assert_eq!(writer.inner.frames.lock().unwrap()[0].bytes, b"{}");
    assert_eq!(metrics.stage_snapshot(Stage::CompleteWait).count, 2);
    assert_eq!(metrics.stage_snapshot(Stage::WorkerTotal).cancelled, 0);
}

#[tokio::test]
async fn cumulative_diagnostics_concurrent_complete_waiters_share_one_durable_receipt() {
    use super::super::diagnostics::Stage;
    let writer = Arc::new(SlowFactWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_diagnostics(
        writer.clone(), ClientTrajectoryTransport::Http, std::time::Duration::ZERO, true,
    );
    let metrics = recorder.owner.state.diagnostics.clone().unwrap();
    recorder.bind_run(Uuid::now_v7(), None);
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}").await.unwrap();
    writer.entered.notified().await;
    let mut waiters = Vec::new();
    for _ in 0..2 {
        let owner = recorder.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        waiters.push(tokio::spawn(async move {
            signal_first_pending(owner.complete(), started).await
        }));
        ready.await.expect("each completion waiter was polled Pending");
    }
    assert!(waiters.iter().all(|waiter| !waiter.is_finished()));
    writer.release.notify_one();
    for waiter in waiters {
        let receipt = waiter.await.unwrap().unwrap();
        assert_eq!(receipt.request_id, recorder.capture_id());
        assert_eq!(receipt.persisted_through, 1);
    }
    let completion = metrics.stage_snapshot(Stage::CompleteWait);
    assert_eq!(completion.count, 2);
    assert_eq!(completion.failures, 0);
    assert_eq!(completion.cancelled, 0);
    assert_eq!(metrics.stage_snapshot(Stage::WorkerTotal).count, 1);
}
