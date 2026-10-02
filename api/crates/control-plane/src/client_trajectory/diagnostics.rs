//! Opt-in cumulative elapsed-time diagnostics. No frame/value/content is retained.
//! Nested classifier/worker spans are inclusive; these are not CPU measurements.
use super::*;
use std::{sync::OnceLock, time::Instant};

const ENABLE_ENV: &str = "FLOWBASE_CLIENT_TRAJECTORY_DIAGNOSTICS";
pub(super) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var(ENABLE_ENV).as_deref() == Ok("1"))
}

#[derive(Clone, Copy)]
#[repr(usize)]
pub(super) enum Stage {
    RequestAdmission,
    ResponseSseAdmission,
    ResponseJsonAdmission,
    ArchiveCollect,
    ArchiveCommit,
    ReplayRead,
    Decode,
    RequestClassifyInclusive,
    ResponseClassifyInclusive,
    FactAppend,
    UnboundDiscard,
    CompleteWait,
    WorkerTotal,
}
impl Stage {
    pub(super) const COUNT: usize = 13;
    const ALL: [(Self, &'static str); Self::COUNT] = [
        (Self::RequestAdmission, "request_admission"),
        (Self::ResponseSseAdmission, "response_sse_admission"),
        (Self::ResponseJsonAdmission, "response_json_admission"),
        (Self::ArchiveCollect, "archive_collect"),
        (Self::ArchiveCommit, "archive_commit"),
        (Self::ReplayRead, "replay_read"),
        (Self::Decode, "decode"),
        (Self::RequestClassifyInclusive, "request_classify_inclusive"),
        (Self::ResponseClassifyInclusive, "response_classify_inclusive"),
        (Self::FactAppend, "fact_append"),
        (Self::UnboundDiscard, "unbound_discard"),
        (Self::CompleteWait, "complete_wait"),
        (Self::WorkerTotal, "worker_total"),
    ];
    pub(super) fn admission(kind: ClientTrajectoryFrameKind) -> Self {
        match kind {
            ClientTrajectoryFrameKind::Request => Self::RequestAdmission,
            ClientTrajectoryFrameKind::ResponseSse => Self::ResponseSseAdmission,
            ClientTrajectoryFrameKind::ResponseJson => Self::ResponseJsonAdmission,
        }
    }
    pub(super) fn classification(kind: ClientTrajectoryFrameKind) -> Self {
        if kind == ClientTrajectoryFrameKind::Request {
            Self::RequestClassifyInclusive
        } else {
            Self::ResponseClassifyInclusive
        }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Outcome {
    Success,
    Failure,
    Cancelled,
}
#[derive(Default)]
struct Counter {
    count: AtomicU64,
    total_ns: AtomicU64,
    max_ns: AtomicU64,
    failures: AtomicU64,
    cancelled: AtomicU64,
}
#[derive(serde::Serialize)]
pub(super) struct Snapshot {
    pub count: u64,
    pub total_ns: u64,
    pub max_ns: u64,
    pub failures: u64,
    pub cancelled: u64,
}
fn add_saturating(counter: &AtomicU64, amount: u64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
        Some(n.saturating_add(amount))
    });
}
pub(super) struct Diagnostics {
    stages: [Counter; Stage::COUNT],
    clock_reads: AtomicU64,
    complete_reported: AtomicBool,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            stages: std::array::from_fn(|_| Counter::default()),
            clock_reads: AtomicU64::new(0),
            complete_reported: AtomicBool::new(false),
        }
    }
}
impl Diagnostics {
    pub(super) fn start(&self, stage: Stage) -> Span<'_> {
        let start = Instant::now();
        add_saturating(&self.clock_reads, 1);
        Span {
            metrics: self,
            stage,
            start,
            outcome: Outcome::Cancelled,
        }
    }
    pub(super) fn record_ns(&self, stage: Stage, ns: u64, outcome: Outcome) {
        let counter = &self.stages[stage as usize];
        add_saturating(&counter.count, 1);
        add_saturating(&counter.total_ns, ns);
        counter.max_ns.fetch_max(ns, Ordering::Release);
        match outcome {
            Outcome::Success => {}
            Outcome::Failure => add_saturating(&counter.failures, 1),
            Outcome::Cancelled => add_saturating(&counter.cancelled, 1),
        }
    }
    pub(super) fn stage_snapshot(&self, stage: Stage) -> Snapshot {
        let counter = &self.stages[stage as usize];
        // Read the monotone maximum before the sum and the outcomes before count.
        // Concurrent completion may make the snapshot conservative, never imply
        // max > sum or outcomes > count. Stages are not a jointly atomic snapshot.
        let max_ns = counter.max_ns.load(Ordering::Acquire);
        let total_ns = counter.total_ns.load(Ordering::Acquire);
        let failures = counter.failures.load(Ordering::Acquire);
        let cancelled = counter.cancelled.load(Ordering::Acquire);
        let count = counter.count.load(Ordering::Acquire);
        Snapshot {
            count,
            total_ns,
            max_ns,
            failures,
            cancelled,
        }
    }
    pub(super) fn clock_reads(&self) -> u64 {
        self.clock_reads.load(Ordering::Relaxed)
    }
    pub(super) fn payload(
        &self,
        event: &'static str,
        id: Uuid,
        flow: Option<Uuid>,
        ok: bool,
    ) -> serde_json::Value {
        let stages: BTreeMap<_, _> = Stage::ALL
            .into_iter()
            .map(|(stage, name)| (name, self.stage_snapshot(stage)))
            .collect();
        serde_json::json!({
            "version": 1, "event": event, "capture_id": id, "flow_run_id": flow,
            "ok": ok, "clock_reads": self.clock_reads(), "stages": stages,
        })
    }
    pub(super) fn emit(&self, event: &'static str, id: Uuid, state: &Shared, ok: bool) {
        let flow = state
            .scope
            .lock()
            .ok()
            .and_then(|scope| (*scope).map(|scope| scope.flow));
        let payload = self.payload(event, id, flow, ok);
        tracing::info!(target: "gateway_capture_diagnostic", diagnostic = %payload, "gateway capture cumulative timing");
    }
    pub(super) fn emit_first_complete(&self, id: Uuid, state: &Shared, ok: bool) {
        if !self.complete_reported.swap(true, Ordering::Relaxed) {
            self.emit("complete_end", id, state, ok);
        }
    }
}
pub(super) struct Span<'a> {
    metrics: &'a Diagnostics,
    stage: Stage,
    start: Instant,
    outcome: Outcome,
}
impl Span<'_> {
    pub(super) fn finish(mut self, ok: bool) {
        self.outcome = if ok {
            Outcome::Success
        } else {
            Outcome::Failure
        };
        // Drop accounts for both completed and cancelled futures without altering them.
    }
}
impl Drop for Span<'_> {
    fn drop(&mut self) {
        let ns = self.start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        add_saturating(&self.metrics.clock_reads, 1);
        self.metrics.record_ns(self.stage, ns, self.outcome);
    }
}
pub(super) fn start(state: &Shared, stage: Stage) -> Option<Span<'_>> {
    state.diagnostics.as_ref().map(|metrics| metrics.start(stage))
}
pub(super) fn finish(span: Option<Span<'_>>, ok: bool) {
    if let Some(span) = span {
        span.finish(ok);
    }
}
