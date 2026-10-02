use super::*;
use std::time::{Duration, Instant};

async fn burst(delay: Duration) -> (u64, Vec<ClientTrajectoryArchiveFrame>) {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_flush_delay(
        writer.clone(),
        ClientTrajectoryTransport::Http,
        delay,
    );
    assert!(recorder.bind_run(Uuid::now_v7(), None));
    for index in 0u32..256 {
        recorder
            .record(ClientTrajectoryFrameKind::ResponseSse, &index.to_be_bytes())
            .await
            .unwrap();
        tokio::task::yield_now().await;
    }
    let receipt = recorder.complete().await.unwrap();
    assert_eq!(receipt.persisted_through, 256);
    let frames = writer.frames.lock().unwrap().clone();
    (writer.archive_calls.load(Ordering::Relaxed), frames)
}

#[tokio::test]
async fn burst_batching_preserves_every_frame_and_avoids_scheduler_sized_parts() {
    let (immediate, before) = burst(Duration::ZERO).await;
    let (batched, after) = burst(batch::MAX_DELAY).await;
    assert!(
        batched < immediate,
        "coalescing must reduce commits: {immediate} -> {batched}"
    );
    assert!(
        batched <= 4,
        "the small burst fits the declared byte/time budget"
    );
    for (index, (before, after)) in before.iter().zip(&after).enumerate() {
        assert_eq!(before.sequence, index as i64 + 1);
        assert_eq!(after.sequence, before.sequence);
        assert_eq!(after.kind, before.kind);
        assert_eq!(after.bytes, (index as u32).to_be_bytes());
    }
    eprintln!("capture burst commits: {immediate} -> {batched}");
}

#[tokio::test]
async fn idle_frames_are_committed_without_terminal_and_owner_drop_flushes_accepted_suffix() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    assert!(recorder.bind_run(Uuid::now_v7(), None));
    let started = Instant::now();
    recorder
        .record(ClientTrajectoryFrameKind::ResponseSse, b"idle")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), writer.archived.notified())
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(writer.frames.lock().unwrap()[0].bytes, b"idle");
    recorder
        .record(
            ClientTrajectoryFrameKind::ResponseSse,
            b"cancelled suffix\0\xff",
        )
        .await
        .unwrap();
    let state = recorder.owner.state.clone();
    drop(recorder);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let stopped = state.stopped_notify.notified();
            if state.stopped.load(Ordering::Acquire) {
                break;
            }
            stopped.await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        state
            .result
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .persisted_through,
        2
    );
    assert_eq!(
        writer.frames.lock().unwrap()[1].bytes,
        b"cancelled suffix\0\xff"
    );
    eprintln!(
        "idle archive visibility: {elapsed:?}, scheduling budget {:?}",
        batch::MAX_DELAY
    );
}

#[derive(Default)]
struct NaturalBatchSink {
    singles: Vec<ClientTrajectoryFact>,
    batches: Vec<Vec<ClientTrajectoryFact>>,
}
#[async_trait::async_trait]
impl classify::FactSink for NaturalBatchSink {
    async fn push(&mut self, fact: ClientTrajectoryFact) {
        self.singles.push(fact);
    }
    async fn push_many(&mut self, facts: Vec<ClientTrajectoryFact>) {
        self.batches.push(facts);
    }
}

#[tokio::test]
async fn natural_step_batches_preserve_exact_sections_and_timing_order() {
    let at = "2026-10-02T00:00:00Z";
    let request = Uuid::now_v7();
    let flow = Uuid::now_v7();
    let mut classifier =
        classify::Classifier::new(request, flow, None, ClientTrajectoryTransport::Http);
    let mut sink = NaturalBatchSink::default();
    classifier.begin_request_into(at, &mut sink).await;
    assert_eq!(sink.batches.len(), 1);
    assert!(
        matches!(&sink.batches[0][0], ClientTrajectoryFact::Step { step } if step.id == request)
    );
    assert!(
        matches!(&sink.batches[0][1], ClientTrajectoryFact::Section { step_id, section, value }
        if *step_id == request && section == "timing" && value == &json!({"observed_at":at}))
    );
    sink.batches.clear();
    let original = json!({"type":"function_call","id":"fc_exact","call_id":"call_exact",
        "name":"lookup","arguments":" {\"text\":\"北京\\u0000🌍\",\"enabled\":true,\"optional\":null} "});
    classifier
        .observe_into(
            ClientTrajectoryFrameKind::ResponseJson,
            json!({"object":"response","output":[original.clone()]}),
            at,
            &mut sink,
        )
        .await;
    let batch = sink
        .batches
        .iter()
        .find(|batch| {
            matches!(&batch[0], ClientTrajectoryFact::Step { step }
        if step.category == "tool_call")
        })
        .expect("one complete tool step");
    let ClientTrajectoryFact::Step { step } = &batch[0] else {
        unreachable!()
    };
    assert_eq!(step.item_id.as_deref(), Some("fc_exact"));
    assert_eq!(step.call_id.as_deref(), Some("call_exact"));
    let sections: Vec<_> = batch[1..]
        .iter()
        .map(|fact| match fact {
            ClientTrajectoryFact::Section {
                step_id,
                section,
                value,
            } => {
                assert_eq!(*step_id, step.id);
                (section.as_str(), value)
            }
            _ => panic!("sections must follow their step"),
        })
        .collect();
    assert_eq!(
        sections.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        vec!["overview", "parameters", "timing"]
    );
    assert_eq!(sections[0].1, &original);
    assert_eq!(sections[1].1, &original["arguments"]);
    assert_eq!(sections[2].1, &json!({"observed_at":at}));
    assert!(sink.singles.iter().all(|fact| !matches!(
        fact,
        ClientTrajectoryFact::Step { .. } | ClientTrajectoryFact::Section { .. }
    )));
}

#[tokio::test]
async fn unsupported_batch_keeps_individual_failure_accounting_and_suffix() {
    use classify::FactSink;
    let writer = MemoryWriter::default();
    writer.fail_once.store(true, Ordering::Release);
    let scope = Scope {
        flow: Uuid::now_v7(),
        node: None,
    };
    let request = Uuid::now_v7();
    let mut failed = 0;
    let mut sink = PersistenceSink {
        repository: &writer,
        scope,
        id: request,
        at: "exact-at",
        failed: &mut failed,
    };
    sink.push_many(vec![
        ClientTrajectoryFact::ResponseLink {
            response_id: "first".into(),
        },
        ClientTrajectoryFact::ResponseLink {
            response_id: "second".into(),
        },
    ])
    .await;
    assert_eq!(failed, 1);
    let records = writer.records.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert!(
        matches!(&records[0].fact, ClientTrajectoryFact::ResponseLink { response_id } if response_id == "second")
    );
}

#[derive(Default)]
struct FailingAtomicWriter(MemoryWriter);
#[async_trait::async_trait]
impl FactWriter for FailingAtomicWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        self.0.append(input).await
    }
    async fn append_batch(&self, _inputs: &[AppendClientTrajectoryInput]) -> anyhow::Result<bool> {
        anyhow::bail!("fixture atomic rollback")
    }
    async fn archive(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> anyhow::Result<ClientTrajectoryArchiveReceipt> {
        self.0.archive(input).await
    }
    async fn discard_unbound(&self, request: Uuid) -> anyhow::Result<()> {
        self.0.discard_unbound(request).await
    }
    async fn replay(
        &self,
        request: Uuid,
        cursor: i64,
    ) -> anyhow::Result<Vec<ClientTrajectoryArchiveFrame>> {
        self.0.replay(request, cursor).await
    }
}
#[tokio::test]
async fn atomic_failure_counts_all_unpersisted_facts_without_individual_retry() {
    use classify::FactSink;
    let writer = FailingAtomicWriter::default();
    let mut failed = 0;
    let mut sink = PersistenceSink {
        repository: &writer,
        scope: Scope {
            flow: Uuid::now_v7(),
            node: None,
        },
        id: Uuid::now_v7(),
        at: "exact-at",
        failed: &mut failed,
    };
    sink.push_many(vec![
        ClientTrajectoryFact::ResponseLink {
            response_id: "first".into(),
        },
        ClientTrajectoryFact::ResponseLink {
            response_id: "second".into(),
        },
    ])
    .await;
    assert_eq!(failed, 2);
    assert!(writer.0.records.lock().unwrap().is_empty());
}
