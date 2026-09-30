//! Physical write coalescing. Admission is queued; only repository commit advances ACK.
use super::*;
// Reuse the runtime persistence latency budget rather than invent a task limit.
pub(super) const MAX_DELAY: std::time::Duration =
    crate::orchestration_runtime::RUNTIME_EVENT_BATCH_MAX_DELAY;

pub(super) async fn collect(
    first: Frame,
    state: &Shared,
    receiver: &mut mpsc::Receiver<Frame>,
) -> (Vec<ClientTrajectoryArchiveFrame>, bool) {
    let deadline = tokio::time::Instant::now() + state.flush_delay;
    let mut frames = Vec::new();
    let mut bytes = 0usize;
    let mut next = Some(first);
    loop {
        if let Some(frame) = next.take() {
            // Include per-frame metadata so empty frames also have a bounded batch.
            bytes = bytes
                .saturating_add(frame.bytes.len())
                .saturating_add(frame.at.len())
                .saturating_add(std::mem::size_of::<ClientTrajectoryArchiveFrame>());
            frames.push(ClientTrajectoryArchiveFrame {
                sequence: 0,
                kind: frame.kind,
                observed_at: frame.at,
                bytes: frame.bytes,
            });
        }
        if bytes >= FRAME_BYTES {
            return (frames, false);
        }
        if state.finished.load(Ordering::Acquire) {
            receiver.close();
        }
        tokio::select! {
            frame = receiver.recv() => match frame {
                Some(frame) => next = Some(frame),
                None => return (frames, true),
            },
            _ = tokio::time::sleep_until(deadline) => return (frames, false),
            _ = state.notify.notified() => {},
        }
    }
}
