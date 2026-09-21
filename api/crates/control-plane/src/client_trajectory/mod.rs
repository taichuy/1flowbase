//! Best-effort bounded sidecar for actual client Responses bytes. Never handles credentials.
#[cfg(test)]
mod _tests;
mod classify;
mod decode;
mod schemas;

use crate::ports::{
    AppendClientTrajectoryInput, ClientTrajectoryFact, OrchestrationRuntimeRepository,
};
pub use crate::ports::{ClientTrajectoryFrameKind, ClientTrajectoryTransport};
use base64::Engine;
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use time::OffsetDateTime;
use tokio::sync::{mpsc, Notify, OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

const FRAME_BYTES: usize = 64 * 1024;
const QUEUE_BYTES: usize = 2 * 1024 * 1024;
const QUEUE_RECORDS: usize = 128;
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
#[derive(Clone, Copy, PartialEq, Eq)]
struct Scope {
    flow: Uuid,
    node: Option<Uuid>,
}
struct Frame {
    kind: ClientTrajectoryFrameKind,
    bytes: Vec<u8>,
    at: String,
    _permit: OwnedSemaphorePermit,
}
struct Shared {
    scope: Mutex<Option<Scope>>,
    notify: Notify,
    finished: AtomicBool,
    dropped: AtomicU64,
    stopped: AtomicBool,
    stopped_notify: Notify,
}
struct Owner {
    state: Arc<Shared>,
    sender: mpsc::Sender<Frame>,
    bytes: Arc<Semaphore>,
    id: Uuid,
}
impl Drop for Owner {
    fn drop(&mut self) {
        if !self.state.finished.swap(true, Ordering::AcqRel) {
            self.state.dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.state.notify.notify_one();
    }
}
#[derive(Clone)]
pub struct ClientTrajectoryRecorder {
    owner: Arc<Owner>,
}
#[async_trait::async_trait]
trait FactWriter: Send + Sync {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()>;
}
struct RepositoryWriter(Arc<dyn OrchestrationRuntimeRepository>);
#[async_trait::async_trait]
impl FactWriter for RepositoryWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        self.0.append_client_trajectory(input).await
    }
}
impl ClientTrajectoryRecorder {
    pub fn new(
        repository: Arc<dyn OrchestrationRuntimeRepository>,
        transport: ClientTrajectoryTransport,
    ) -> Self {
        Self::with_writer(Arc::new(RepositoryWriter(repository)), transport)
    }
    fn with_writer(repository: Arc<dyn FactWriter>, transport: ClientTrajectoryTransport) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE_RECORDS);
        let state = Arc::new(Shared {
            scope: Mutex::new(None),
            notify: Notify::new(),
            finished: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            stopped_notify: Notify::new(),
        });
        let id = Uuid::now_v7();
        tokio::spawn(worker(
            repository,
            transport,
            id,
            Arc::clone(&state),
            receiver,
        ));
        Self {
            owner: Arc::new(Owner {
                state,
                sender,
                bytes: Arc::new(Semaphore::new(QUEUE_BYTES)),
                id,
            }),
        }
    }
    pub fn capture_id(&self) -> Uuid {
        self.owner.id
    }
    pub fn bind_run(&self, flow_run_id: Uuid, node_run_id: Option<Uuid>) -> bool {
        if self.owner.state.stopped.load(Ordering::Acquire) {
            return false;
        }
        let target = Scope {
            flow: flow_run_id,
            node: node_run_id,
        };
        let Ok(mut scope) = self.owner.state.scope.lock() else {
            self.mark_incomplete();
            return false;
        };
        if let Some(existing) = *scope {
            if existing != target {
                self.mark_incomplete();
                return false;
            }
        } else {
            *scope = Some(target);
        }
        self.owner.state.notify.notify_one();
        true
    }
    /// Copies at most the available byte/record budget. Neither queue admission nor
    /// database persistence waits on the business forwarding path.
    pub fn record(&self, kind: ClientTrajectoryFrameKind, bytes: &[u8]) {
        if self.owner.state.finished.load(Ordering::Acquire) {
            return;
        }
        let at = observed_at();
        for chunk in bytes.chunks(FRAME_BYTES) {
            let Ok(permit) = self
                .owner
                .bytes
                .clone()
                .try_acquire_many_owned(chunk.len() as u32)
            else {
                self.mark_incomplete();
                return;
            };
            let frame = Frame {
                kind,
                bytes: chunk.to_vec(),
                at: at.clone(),
                _permit: permit,
            };
            if self.owner.sender.try_send(frame).is_err() {
                self.mark_incomplete();
                return;
            }
        }
    }
    pub fn mark_incomplete(&self) {
        self.owner.state.dropped.fetch_add(1, Ordering::Relaxed);
    }
    /// Call after the last observed byte; this does not wait for persistence.
    pub fn finish(&self) {
        self.owner.state.finished.store(true, Ordering::Release);
        self.owner.state.notify.notify_one();
    }
    pub async fn wait_finished(&self) {
        loop {
            let wake = self.owner.state.stopped_notify.notified();
            tokio::pin!(wake);
            wake.as_mut().enable();
            if self.owner.state.stopped.load(Ordering::Acquire) {
                return;
            }
            wake.await;
        }
    }
}
fn observed_at() -> String {
    OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("UTC timestamp")
}

async fn persist(
    repository: &dyn FactWriter,
    scope: Scope,
    id: Uuid,
    at: String,
    fact: ClientTrajectoryFact,
    failed: &mut u64,
) {
    let input = AppendClientTrajectoryInput {
        flow_run_id: scope.flow,
        node_run_id: scope.node,
        request_id: id,
        observed_at: at,
        fact,
    };
    if !matches!(
        tokio::time::timeout(WRITE_TIMEOUT, repository.append(&input)).await,
        Ok(Ok(()))
    ) {
        *failed = failed.saturating_add(1);
    }
}
async fn worker(
    repository: Arc<dyn FactWriter>,
    transport: ClientTrajectoryTransport,
    id: Uuid,
    state: Arc<Shared>,
    mut receiver: mpsc::Receiver<Frame>,
) {
    // Permits stay with pre-bind frames; draining the channel cannot defeat the byte budget.
    let mut pending = VecDeque::new();
    let scope = loop {
        let scope = state.scope.lock().ok().and_then(|scope| *scope);
        if let Some(scope) = scope {
            break Some(scope);
        }
        if state.finished.load(Ordering::Acquire) {
            break None;
        }
        tokio::select! {
            _=state.notify.notified()=>{},
            frame=receiver.recv()=>match frame {Some(frame)=>{if pending.len()<QUEUE_RECORDS {pending.push_back(frame);}else{state.dropped.fetch_add(1,Ordering::Relaxed);}},None=>break None},
            _=tokio::time::sleep(IDLE_TIMEOUT)=>{state.dropped.fetch_add(1,Ordering::Relaxed);break None;}
        }
    };
    if let Some(scope) = scope {
        let mut failed = 0;
        persist(
            repository.as_ref(),
            scope,
            id,
            observed_at(),
            ClientTrajectoryFact::Integrity {
                status: "pending".into(),
                dropped_count: 0,
                persist_failed_count: 0,
            },
            &mut failed,
        )
        .await;
        let mut classifier = classify::Classifier::new(id, scope.flow, scope.node, transport);
        let mut decoder = decode::Decoder::default();
        loop {
            if state.finished.load(Ordering::Acquire) {
                receiver.close();
            }
            let frame = if let Some(frame) = pending.pop_front() {
                Some(frame)
            } else {
                tokio::select! {
                    frame=receiver.recv()=>frame,
                    _=state.notify.notified()=>continue,
                    _=tokio::time::sleep(IDLE_TIMEOUT)=>{state.dropped.fetch_add(1,Ordering::Relaxed);receiver.close();continue;}
                }
            };
            let Some(frame) = frame else {
                break;
            };
            let direction = if frame.kind == ClientTrajectoryFrameKind::Request {
                "submitted"
            } else {
                "emitted"
            };
            let (encoding, body) = match std::str::from_utf8(&frame.bytes) {
                Ok(text) => ("utf8", text.to_owned()),
                Err(_) => (
                    "base64",
                    base64::engine::general_purpose::STANDARD.encode(&frame.bytes),
                ),
            };
            persist(repository.as_ref(),scope,id,frame.at.clone(),ClientTrajectoryFact::Section {step_id:id,section:"raw".into(),value:json!({"direction":direction,"encoding":encoding,"body":body,"frame_kind":frame.kind})},&mut failed).await;
            if frame.kind == ClientTrajectoryFrameKind::Request {
                for fact in classifier.begin_request(&frame.at) {
                    persist(
                        repository.as_ref(),
                        scope,
                        id,
                        frame.at.clone(),
                        fact,
                        &mut failed,
                    )
                    .await;
                }
            }
            let values = decoder.feed(frame.kind, &frame.bytes);
            for value in values {
                let facts = classifier.observe(frame.kind, value, &frame.at);
                for fact in facts {
                    persist(
                        repository.as_ref(),
                        scope,
                        id,
                        frame.at.clone(),
                        fact,
                        &mut failed,
                    )
                    .await;
                }
            }
            // Frame permit released each iteration, so arbitrarily long streams drain.
        }
        decoder.finish();
        let dropped = state.dropped.load(Ordering::Acquire)
            + u64::from(
                decoder.incomplete
                    || classifier.incomplete
                    || !classifier.completed
                    || !classifier.request_seen,
            );
        let status = if dropped == 0 && failed == 0 {
            "complete"
        } else {
            "incomplete"
        };
        persist(
            repository.as_ref(),
            scope,
            id,
            observed_at(),
            ClientTrajectoryFact::Integrity {
                status: status.into(),
                dropped_count: dropped,
                persist_failed_count: failed,
            },
            &mut failed,
        )
        .await;
    }
    state.stopped.store(true, Ordering::Release);
    state.stopped_notify.notify_waiters();
}
