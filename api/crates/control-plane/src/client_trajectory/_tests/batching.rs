use super::*;
use std::time::{Duration, Instant};

async fn burst(
    delay: Duration,
    expect_first_commit: bool,
) -> (u64, Vec<ClientTrajectoryArchiveFrame>) {
    let writer = Arc::new(MemoryWriter::default());
    let recorder = ClientTrajectoryRecorder::with_writer_and_flush_delay(
        writer.clone(),
        ClientTrajectoryTransport::Http,
        delay,
    );
    assert!(recorder.bind_run(Uuid::now_v7(), None));
    assert_eq!(batch::MAX_DELAY, Duration::from_millis(20));
    let started = tokio::time::Instant::now();
    let tail_arrival = started + batch::MAX_DELAY / 2;
    recorder
        .record(ClientTrajectoryFrameKind::ResponseSse, &0u32.to_be_bytes())
        .await
        .unwrap();
    // A paused-clock sleep advances only once runnable tasks have made progress.
    // The archive notification proves a commit; yield_now alone cannot do so.
    let first_committed = tokio::select! {
        biased;
        _ = writer.archived.notified() => true,
        _ = tokio::time::sleep_until(tail_arrival) => false,
    };
    if expect_first_commit {
        assert!(
            first_committed,
            "immediate first frame must commit before the tail"
        );
        assert_eq!(writer.archive_calls.load(Ordering::Relaxed), 1);
        assert_eq!(writer.frames.lock().unwrap().len(), 1);
        tokio::time::sleep_until(tail_arrival).await;
    } else {
        assert!(
            !first_committed,
            "delayed first frame must stay uncommitted before the deadline"
        );
        assert_eq!(writer.archive_calls.load(Ordering::Relaxed), 0);
        assert!(writer.frames.lock().unwrap().is_empty());
    }
    let tail_offered = tokio::time::Instant::now();
    assert!(tail_offered >= tail_arrival);
    assert!(started.elapsed() < batch::MAX_DELAY);
    // All remaining frames arrive at the same virtual time, inside MAX_DELAY.
    for index in 1u32..256 {
        recorder
            .record(ClientTrajectoryFrameKind::ResponseSse, &index.to_be_bytes())
            .await
            .unwrap();
    }
    assert_eq!(tokio::time::Instant::now(), tail_offered);
    let receipt = recorder.complete().await.unwrap();
    assert_eq!(receipt.request_id, recorder.capture_id());
    assert_eq!(receipt.persisted_through, 256);
    assert!(recorder.owner.state.stopped.load(Ordering::Acquire));
    assert!(started.elapsed() <= batch::MAX_DELAY);
    let frames = writer.frames.lock().unwrap().clone();
    assert_eq!(frames.len(), 256);
    (writer.archive_calls.load(Ordering::Relaxed), frames)
}

#[tokio::test(start_paused = true)]
async fn burst_batching_preserves_every_frame_and_avoids_scheduler_sized_parts() {
    let (immediate, before) = burst(Duration::ZERO, true).await;
    let (batched, after) = burst(batch::MAX_DELAY, false).await;
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
        assert_eq!(before.kind, ClientTrajectoryFrameKind::ResponseSse);
        assert_eq!(after.kind, before.kind);
        assert_eq!(before.bytes, (index as u32).to_be_bytes());
        assert_eq!(after.bytes, (index as u32).to_be_bytes());
    }
    eprintln!("capture burst commits: {immediate} -> {batched}");
}

#[tokio::test(start_paused = true)]
#[should_panic(expected = "delayed first frame must stay uncommitted before the deadline")]
async fn predeadline_commit_oracle_rejects_zero_flush_delay() {
    // Controlled negative: the delayed oracle must reject immediate persistence,
    // even when the tail would otherwise fit in a single archive commit.
    burst(Duration::ZERO, false).await;
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
