use super::*;
use std::time::Duration;

#[derive(Default)]
struct CommittedOnlyWriter {
    inner: MemoryWriter,
    replay_calls: AtomicU64,
    replayed: Notify,
    node_linked: Notify,
}

#[async_trait::async_trait]
impl FactWriter for CommittedOnlyWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        self.inner.append(input).await?;
        if matches!(input.fact, ClientTrajectoryFact::NodeLink { .. }) {
            self.node_linked.notify_one();
        }
        Ok(())
    }
    async fn archive(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> anyhow::Result<ClientTrajectoryArchiveReceipt> {
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
        let committed = self.inner.frames.lock().unwrap().len() as i64;
        anyhow::ensure!(
            cursor < committed,
            "read attempted beyond committed watermark"
        );
        let page = self.inner.replay(request, cursor).await?;
        self.replay_calls.fetch_add(1, Ordering::SeqCst);
        self.replayed.notify_one();
        Ok(page)
    }
}

#[tokio::test]
async fn late_binding_replays_all_pages_without_empty_reads_and_still_links_new_nodes() {
    let writer = Arc::new(CommittedOnlyWriter::default());
    let recorder =
        ClientTrajectoryRecorder::with_writer(writer.clone(), ClientTrajectoryTransport::Http);
    let mut expected = vec![(
        ClientTrajectoryFrameKind::Request,
        br#"{"input":[{"role":"user","content":"late history"}]}"#.to_vec(),
    )];
    expected.extend((0..63).map(|_| {
        (
            ClientTrajectoryFrameKind::ResponseSse,
            b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"x\"}\n\n".to_vec(),
        )
    }));
    expected.push((
        ClientTrajectoryFrameKind::ResponseJson,
        br#"{"object":"response","status":"completed","output":[]}"#.to_vec(),
    ));
    for (kind, bytes) in &expected {
        recorder.record(*kind, bytes).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let committed = writer.inner.archived.notified();
            if writer.inner.frames.lock().unwrap().len() == expected.len() {
                break;
            }
            committed.await;
        }
    })
    .await
    .unwrap();
    let flow = Uuid::now_v7();
    assert!(recorder.bind_run(flow, None));
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let replayed = writer.replayed.notified();
            if writer.replay_calls.load(Ordering::SeqCst) == 3 {
                break;
            }
            replayed.await;
        }
    })
    .await
    .unwrap();
    tokio::task::yield_now().await;
    if recorder.owner.state.stopped.load(Ordering::Acquire) {
        recorder.complete().await.unwrap();
    }
    // With the archive caught up, an independent new link must still be persisted.
    let node = Uuid::now_v7();
    recorder.link_llm_node(flow, node);
    tokio::time::timeout(Duration::from_secs(2), writer.node_linked.notified())
        .await
        .unwrap();
    let receipt = recorder.complete().await.unwrap();
    assert_eq!(receipt.persisted_through, 65);
    assert_eq!(writer.replay_calls.load(Ordering::SeqCst), 3);
    let frames = writer.inner.frames.lock().unwrap();
    for (index, (frame, (kind, bytes))) in frames.iter().zip(&expected).enumerate() {
        assert_eq!(frame.sequence, index as i64 + 1);
        assert_eq!(frame.kind, *kind);
        assert_eq!(&frame.bytes, bytes);
    }
    let records = writer.inner.records.lock().unwrap();
    assert!(complete(&records));
    assert!(records.iter().any(|r| matches!(r.fact,
        ClientTrajectoryFact::NodeLink { node_run_id } if node_run_id == node)));
}
