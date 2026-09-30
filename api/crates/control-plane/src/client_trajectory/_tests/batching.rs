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
