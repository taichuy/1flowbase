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
