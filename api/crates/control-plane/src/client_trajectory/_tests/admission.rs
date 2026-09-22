use super::*;

#[derive(Default)]
struct SlowFactWriter {
    records: Mutex<Vec<AppendClientTrajectoryInput>>,
    entered: Notify,
    release: Notify,
    held: AtomicBool,
}
#[async_trait::async_trait]
impl FactWriter for SlowFactWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        if !self.held.swap(true, Ordering::AcqRel) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.records.lock().unwrap().push(input.clone());
        Ok(())
    }
}

#[tokio::test]
async fn tiny_fragment_bursts_survive_slow_writer_without_coalescing() {
    for transport in [
        ClientTrajectoryTransport::Http,
        ClientTrajectoryTransport::Websocket,
    ] {
        let writer = Arc::new(SlowFactWriter::default());
        let recorder = ClientTrajectoryRecorder::with_writer(writer.clone(), transport);
        recorder.bind_run(Uuid::now_v7(), None);
        writer.entered.notified().await;
        let (kind, wire) = if transport == ClientTrajectoryTransport::Http {
            (ClientTrajectoryFrameKind::ResponseSse, format!(":{}\n\ndata: {{\"type\":\"response.completed\",\"response\":{{\"output\":[]}}}}\n\n", "x".repeat(512)))
        } else {
            (ClientTrajectoryFrameKind::ResponseJson, format!("{{\"type\":\"response.completed\",\"response\":{{\"output\":[],\"padding\":\"{}\"}}}}", "x".repeat(512)))
        };
        assert!(wire.len() > 128);
        let charged = 2 + FRAME_OVERHEAD_BYTES + wire.len() * (1 + FRAME_OVERHEAD_BYTES);
        assert!(charged < QUEUE_BYTES);
        recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
        let before = OffsetDateTime::now_utc();
        for byte in wire.as_bytes() {
            recorder.record(kind, &[*byte]);
        }
        let after = OffsetDateTime::now_utc();
        assert_eq!(
            recorder.owner.bytes.available_permits(),
            QUEUE_BYTES - charged
        );
        assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
        recorder.finish();
        writer.release.notify_one();
        recorder.wait_finished().await;
        let records = writer.records.lock().unwrap();
        assert!(complete(&records));
        assert_eq!(raw(&records, "submitted"), b"{}");
        assert_eq!(raw(&records, "emitted"), wire.as_bytes());
        let fragments: Vec<_> = records
            .iter()
            .filter_map(|record| match &record.fact {
                ClientTrajectoryFact::Section { section, value, .. }
                    if section == "raw" && value["direction"] == "emitted" =>
                {
                    Some((record, value))
                }
                _ => None,
            })
            .collect();
        assert_eq!(fragments.len(), wire.len());
        for ((record, value), byte) in fragments.iter().zip(wire.as_bytes()) {
            assert_eq!(value["body"].as_str().unwrap().as_bytes(), &[*byte]);
            assert_eq!(value["frame_kind"], json!(kind));
            let at = OffsetDateTime::parse(
                &record.observed_at,
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap();
            assert!(at >= before && at <= after);
        }
        assert_eq!(recorder.owner.bytes.available_permits(), QUEUE_BYTES);
    }
}

#[tokio::test]
async fn prebind_frames_retain_weighted_budget_and_overflow_is_nonblocking() {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = capture(writer.clone());
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
    let frame = b": keepalive\n\n";
    let charge = frame.len() + FRAME_OVERHEAD_BYTES;
    let initial = 2 + FRAME_OVERHEAD_BYTES;
    let accepted = (QUEUE_BYTES - initial) / charge;
    assert!(accepted > 128);
    for _ in 0..accepted {
        recorder.record(ClientTrajectoryFrameKind::ResponseSse, frame);
        // Let the worker move frames to pre-bind pending; their permits must stay held.
        tokio::task::yield_now().await;
    }
    let remaining = QUEUE_BYTES - initial - accepted * charge;
    assert_eq!(recorder.owner.bytes.available_permits(), remaining);
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 0);
    // Synchronous admissions all return despite a full retained-memory budget.
    let started = std::time::Instant::now();
    for _ in 0..32 {
        recorder.record(ClientTrajectoryFrameKind::ResponseSse, frame);
    }
    assert!(started.elapsed() < WRITE_TIMEOUT);
    assert_eq!(recorder.owner.state.dropped.load(Ordering::Acquire), 32);
    assert_eq!(recorder.owner.bytes.available_permits(), remaining);
    recorder.bind_run(Uuid::now_v7(), None);
    while recorder.owner.bytes.available_permits() != QUEUE_BYTES {
        tokio::task::yield_now().await;
    }
    let tail = b"data: {\"type\":\"response.completed\",\"response\":{\"output\":[]}}\n\n";
    recorder.record(ClientTrajectoryFrameKind::ResponseSse, tail);
    recorder.finish();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert_eq!(
        raw(&records, "emitted"),
        [frame.repeat(accepted), tail.to_vec()].concat()
    );
    assert!(
        matches!(&records.last().unwrap().fact, ClientTrajectoryFact::Integrity {
        status, dropped_count: 32, persist_failed_count: 0,
    } if status == "incomplete")
    );
    assert_eq!(recorder.owner.bytes.available_permits(), QUEUE_BYTES);
}

#[tokio::test]
async fn normal_database_contention_does_not_block_forwarding_or_discard_capture() {
    let writer = Arc::new(SlowFactWriter::default());
    let recorder =
        ClientTrajectoryRecorder::with_writer(writer.clone(), ClientTrajectoryTransport::Http);
    recorder.bind_run(Uuid::now_v7(), None);
    writer.entered.notified().await;
    let started = std::time::Instant::now();
    recorder.record(ClientTrajectoryFrameKind::Request, b"{}");
    recorder.record(
        ClientTrajectoryFrameKind::ResponseJson,
        b"{\"object\":\"response\",\"output\":[]}",
    );
    recorder.finish();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "capture admission must not await the database"
    );
    // Real conversation projection held the sequence lock for ~2.4s. This
    // fixture rejects the former 2s observational deadline without sleeping on forwarding.
    tokio::time::sleep(Duration::from_secs(3)).await;
    writer.release.notify_one();
    recorder.wait_finished().await;
    let records = writer.records.lock().unwrap();
    assert!(complete(&records));
    assert_eq!(raw(&records, "submitted"), b"{}");
}
