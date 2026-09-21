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
    collections::{BTreeMap, BTreeSet, VecDeque},
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
// Admit one maximum-sized HTTP request including per-frame overhead while
// retaining a bounded 2 MiB response backlog during incremental classification.
const QUEUE_BYTES: usize = decode::AGGREGATE_BYTES
    + decode::AGGREGATE_BYTES.div_ceil(FRAME_BYTES) * FRAME_OVERHEAD_BYTES
    + 2 * 1024 * 1024;
// Charge timestamp/Vec/permit storage, queue bookkeeping and allocation slack per
// resident frame, including frames retained before binding and during persistence.
const FRAME_OVERHEAD_BYTES: usize = 512;
const QUEUE_RECORDS: usize = QUEUE_BYTES / FRAME_OVERHEAD_BYTES;
// Persistence runs only in the bounded sidecar. Its total deadline must allow
// the PostgreSQL default 5s pool acquisition plus a bounded transaction budget.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
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
    node_links: Mutex<BTreeMap<Uuid, String>>,
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
            node_links: Mutex::new(BTreeMap::new()),
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
    /// Associate only an observed Native LLM event; a capture may traverse several nodes.
    pub fn link_llm_node(&self, flow_run_id: Uuid, node_run_id: Uuid) {
        if !self.bind_run(flow_run_id, None) {
            return;
        }
        let Ok(mut links) = self.owner.state.node_links.lock() else {
            self.mark_incomplete();
            return;
        };
        if links.contains_key(&node_run_id) {
            return;
        }
        if links.len() >= 1024 {
            self.mark_incomplete();
            return;
        }
        links.insert(node_run_id, observed_at());
        self.owner.state.notify.notify_one();
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
                .try_acquire_many_owned((chunk.len() + FRAME_OVERHEAD_BYTES) as u32)
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
    let kind = match &input.fact {
        ClientTrajectoryFact::Integrity { .. } => "integrity",
        ClientTrajectoryFact::NodeLink { .. } => "node_link",
        ClientTrajectoryFact::Step { .. } => "step",
        ClientTrajectoryFact::Section { section, .. } => section.as_str(),
    };
    let result = tokio::time::timeout(WRITE_TIMEOUT, repository.append(&input)).await;
    let reason = match &result {
        Ok(Ok(())) => return,
        Err(_) => "timeout",
        Ok(Err(error)) => match error.to_string().as_str() {
            "client trajectory record capacity" => "record_capacity",
            "client trajectory capture scope mismatch" => "capture_scope",
            "client trajectory capture missing" => "capture_missing",
            "client trajectory step scope mismatch" => "step_scope",
            "client trajectory section scope mismatch" => "section_scope",
            "client trajectory node scope mismatch" => "node_scope",
            _ => "repository",
        },
    };
    *failed = failed.saturating_add(1);
    tracing::warn!(request_id = %id, flow_run_id = %scope.flow, fact_kind = kind, reason, "client trajectory fact persistence failed");
}
struct PersistenceSink<'a> {
    repository: &'a dyn FactWriter,
    scope: Scope,
    id: Uuid,
    at: &'a str,
    failed: &'a mut u64,
}
#[async_trait::async_trait]
impl classify::FactSink for PersistenceSink<'_> {
    async fn push(&mut self, fact: ClientTrajectoryFact) {
        persist(
            self.repository,
            self.scope,
            self.id,
            self.at.to_owned(),
            fact,
            self.failed,
        )
        .await;
    }
}

async fn persist_node_links(
    repository: &dyn FactWriter,
    scope: Scope,
    id: Uuid,
    state: &Shared,
    linked: &mut BTreeSet<Uuid>,
    failed: &mut u64,
) {
    let pending: Vec<_> = state
        .node_links
        .lock()
        .map(|links| {
            links
                .iter()
                .filter(|(node, _)| !linked.contains(node))
                .map(|(node, at)| (*node, at.clone()))
                .collect()
        })
        .unwrap_or_default();
    for (node_run_id, at) in pending {
        persist(
            repository,
            scope,
            id,
            at,
            ClientTrajectoryFact::NodeLink { node_run_id },
            failed,
        )
        .await;
        linked.insert(node_run_id);
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
            frame=receiver.recv()=>match frame {Some(frame)=>pending.push_back(frame),None=>break None},
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
        let mut linked = BTreeSet::new();
        loop {
            persist_node_links(
                repository.as_ref(),
                scope,
                id,
                &state,
                &mut linked,
                &mut failed,
            )
            .await;
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
            let mut sink = PersistenceSink {
                repository: repository.as_ref(),
                scope,
                id,
                at: &frame.at,
                failed: &mut failed,
            };
            if frame.kind == ClientTrajectoryFrameKind::Request {
                classifier.begin_request_into(&frame.at, &mut sink).await;
            }
            for value in decoder.feed(frame.kind, &frame.bytes) {
                classifier
                    .observe_into(frame.kind, value, &frame.at, &mut sink)
                    .await;
            }
            // Frame permit released each iteration, so arbitrarily long streams drain.
        }
        persist_node_links(
            repository.as_ref(),
            scope,
            id,
            &state,
            &mut linked,
            &mut failed,
        )
        .await;
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
