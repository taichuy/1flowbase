//! Lossless client capture: bounded transport backpressure and durable immutable parts.
#[cfg(test)]
mod _tests;
mod batch;
mod classify;
mod decode;
mod schemas;
use crate::ports::{
    AppendClientTrajectoryArchiveInput, AppendClientTrajectoryInput, ClientTrajectoryArchiveFrame,
    ClientTrajectoryArchiveReceipt, ClientTrajectoryFact, OrchestrationRuntimeRepository,
};
pub use crate::ports::{ClientTrajectoryFrameKind, ClientTrajectoryTransport};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use time::OffsetDateTime;
use tokio::sync::{mpsc, Notify};
use uuid::Uuid;
// Physical batching parameters, never limits on captured task length or frame size.
const FRAME_BYTES: usize = 64 * 1024;
const QUEUE_RECORDS: usize = 8;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Scope {
    flow: Uuid,
    node: Option<Uuid>,
}
struct Frame {
    kind: ClientTrajectoryFrameKind,
    bytes: Vec<u8>,
    at: String,
}
struct Shared {
    flush_delay: std::time::Duration,
    scope: Mutex<Option<Scope>>,
    binding_closed: AtomicBool,
    node_links: Mutex<BTreeMap<Uuid, String>>,
    notify: Notify,
    finished: AtomicBool,
    dropped: AtomicU64,
    stopped: AtomicBool,
    stopped_notify: Notify,
    result: Mutex<Option<Result<ClientTrajectoryArchiveReceipt, String>>>,
}
struct Owner {
    state: Arc<Shared>,
    sender: mpsc::Sender<Frame>,
    id: Uuid,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.state.finished.store(true, Ordering::Release);
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
    async fn append_batch(&self, _inputs: &[AppendClientTrajectoryInput]) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn archive(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> anyhow::Result<ClientTrajectoryArchiveReceipt>;
    async fn discard_unbound(&self, request: Uuid) -> anyhow::Result<()>;
    async fn replay(
        &self,
        request: Uuid,
        cursor: i64,
    ) -> anyhow::Result<Vec<ClientTrajectoryArchiveFrame>>;
}
struct RepositoryWriter(Arc<dyn OrchestrationRuntimeRepository>);
#[async_trait::async_trait]
impl FactWriter for RepositoryWriter {
    async fn append(&self, input: &AppendClientTrajectoryInput) -> anyhow::Result<()> {
        self.0.append_client_trajectory(input).await
    }
    async fn append_batch(&self, inputs: &[AppendClientTrajectoryInput]) -> anyhow::Result<bool> {
        self.0.append_client_trajectory_batch(inputs).await
    }
    async fn archive(
        &self,
        input: &AppendClientTrajectoryArchiveInput,
    ) -> anyhow::Result<ClientTrajectoryArchiveReceipt> {
        self.0.append_client_trajectory_archive(input).await
    }
    async fn discard_unbound(&self, request: Uuid) -> anyhow::Result<()> {
        self.0
            .discard_unbound_client_trajectory_archive(request)
            .await
    }
    async fn replay(
        &self,
        request: Uuid,
        cursor: i64,
    ) -> anyhow::Result<Vec<ClientTrajectoryArchiveFrame>> {
        self.0
            .read_client_trajectory_archive(request, cursor, 32)
            .await
    }
}
impl ClientTrajectoryRecorder {
    pub fn new(
        repository: Arc<dyn OrchestrationRuntimeRepository>,
        transport: ClientTrajectoryTransport,
    ) -> Self {
        Self::new_with_flush_delay(repository, transport, batch::MAX_DELAY)
    }
    /// Host-selected physical scheduling budget; zero requests immediate commits.
    /// This changes no task/frame capacity or durable-completion contract.
    pub fn new_with_flush_delay(
        repository: Arc<dyn OrchestrationRuntimeRepository>,
        transport: ClientTrajectoryTransport,
        flush_delay: std::time::Duration,
    ) -> Self {
        Self::with_writer_and_flush_delay(
            Arc::new(RepositoryWriter(repository)),
            transport,
            flush_delay,
        )
    }
    #[cfg(test)]
    fn with_writer(repository: Arc<dyn FactWriter>, transport: ClientTrajectoryTransport) -> Self {
        Self::with_writer_and_flush_delay(repository, transport, batch::MAX_DELAY)
    }
    fn with_writer_and_flush_delay(
        repository: Arc<dyn FactWriter>,
        transport: ClientTrajectoryTransport,
        flush_delay: std::time::Duration,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(QUEUE_RECORDS);
        let state = Arc::new(Shared {
            flush_delay,
            scope: Mutex::new(None),
            binding_closed: AtomicBool::new(false),
            node_links: Mutex::new(BTreeMap::new()),
            notify: Notify::new(),
            finished: AtomicBool::new(false),
            dropped: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            stopped_notify: Notify::new(),
            result: Mutex::new(None),
        });
        let id = Uuid::now_v7();
        tokio::spawn(worker(repository, transport, id, state.clone(), receiver));
        Self {
            owner: Arc::new(Owner { state, sender, id }),
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
        // EOF freezes binding while holding this same mutex. A queued frame
        // may still finish and bind late before that final ownership boundary.
        if self.owner.state.binding_closed.load(Ordering::Acquire) {
            return false;
        }
        if scope.is_some_and(|s| s != target) {
            self.mark_incomplete();
            return false;
        }
        *scope = Some(target);
        self.owner.state.notify.notify_one();
        true
    }
    pub fn link_llm_node(&self, flow_run_id: Uuid, node_run_id: Uuid) {
        if !self.bind_run(flow_run_id, None) {
            return;
        }
        if let Ok(mut links) = self.owner.state.node_links.lock() {
            links.entry(node_run_id).or_insert_with(observed_at);
        } else {
            self.mark_incomplete();
        }
        self.owner.state.notify.notify_one();
    }
    /// Backpressure preserves original frame boundaries; awaiting admission never truncates.
    pub async fn record(
        &self,
        kind: ClientTrajectoryFrameKind,
        bytes: &[u8],
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.owner.state.finished.load(Ordering::Acquire),
            "client capture already finished"
        );
        self.owner
            .sender
            .send(Frame {
                kind,
                bytes: bytes.to_vec(),
                at: observed_at(),
            })
            .await
            .map_err(|_| anyhow::anyhow!("client archive writer stopped"))
    }
    pub fn mark_incomplete(&self) {
        self.owner.state.dropped.fetch_add(1, Ordering::Relaxed);
    }
    /// Signals EOF. Owners must await complete() before reporting durable completion.
    pub fn finish(&self) {
        self.owner.state.finished.store(true, Ordering::Release);
        self.owner.state.notify.notify_one();
    }
    pub async fn complete(&self) -> anyhow::Result<ClientTrajectoryArchiveReceipt> {
        self.finish();
        self.wait_finished().await;
        match self
            .owner
            .state
            .result
            .lock()
            .expect("capture result lock")
            .clone()
        {
            Some(Ok(receipt)) => Ok(receipt),
            Some(Err(error)) => Err(anyhow::anyhow!(error)),
            None => Err(anyhow::anyhow!("client archive completion missing")),
        }
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
        ClientTrajectoryFact::ResponseLink { .. } => "response_link",
        ClientTrajectoryFact::Step { .. } => "step",
        ClientTrajectoryFact::Section { section, .. } => section.as_str(),
    };
    let Err(error) = repository.append(&input).await else {
        return;
    };
    let reason = match error.to_string().as_str() {
        "client trajectory capture scope mismatch" => "capture_scope",
        "client trajectory capture missing" => "capture_missing",
        "client trajectory step scope mismatch" => "step_scope",
        "client trajectory section scope mismatch" => "section_scope",
        "client trajectory node scope mismatch" => "node_scope",
        _ => "repository",
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
    async fn push_many(&mut self, facts: Vec<ClientTrajectoryFact>) {
        let inputs: Vec<_> = facts
            .into_iter()
            .map(|fact| AppendClientTrajectoryInput {
                flow_run_id: self.scope.flow,
                node_run_id: self.scope.node,
                request_id: self.id,
                observed_at: self.at.to_owned(),
                fact,
            })
            .collect();
        match self.repository.append_batch(&inputs).await {
            Ok(true) => {}
            Ok(false) => {
                // Adapters without atomic batching retain individual ACK/failure accounting.
                for input in inputs {
                    self.push(input.fact).await;
                }
            }
            Err(_) => {
                *self.failed = self.failed.saturating_add(inputs.len() as u64);
                tracing::warn!(request_id = %self.id, flow_run_id = %self.scope.flow,
                    fact_count = inputs.len(), "client trajectory atomic batch persistence failed");
            }
        }
    }
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
    let result = archive_worker(repository.as_ref(), transport, id, &state, &mut receiver)
        .await
        .map_err(|e| e.to_string());
    if let Err(error) = &result {
        // Drop-only transport owners have no caller left to inspect complete().
        tracing::warn!(request_id=%id, %error, "client trajectory archive owner completion failed");
    }
    *state.result.lock().expect("capture result lock") = Some(result);
    state.stopped.store(true, Ordering::Release);
    state.stopped_notify.notify_waiters();
}
async fn archive_worker(
    repository: &dyn FactWriter,
    transport: ClientTrajectoryTransport,
    id: Uuid,
    state: &Shared,
    receiver: &mut mpsc::Receiver<Frame>,
) -> anyhow::Result<ClientTrajectoryArchiveReceipt> {
    let mut receipt = ClientTrajectoryArchiveReceipt {
        request_id: id,
        persisted_through: 0,
    };
    let mut classifier = None;
    let mut decoder = decode::Decoder::default();
    let mut replay_cursor = 0;
    let mut failed = 0;
    let mut linked = BTreeSet::new();
    loop {
        if state.finished.load(Ordering::Acquire) {
            receiver.close();
        }
        let (next, mut eof) = tokio::select! {frame=receiver.recv()=>{let eof=frame.is_none();(frame,eof)},_=state.notify.notified()=>(None,false)};
        if let Some(frame) = next {
            let (frames, drained) = batch::collect(frame, state, receiver).await;
            eof |= drained;
            receipt = repository
                .archive(&AppendClientTrajectoryArchiveInput {
                    request_id: id,
                    part_id: Uuid::now_v7(),
                    transport,
                    frames,
                })
                .await?;
        }
        let scope = {
            let scope = state
                .scope
                .lock()
                .map_err(|_| anyhow::anyhow!("client binding scope lock poisoned"))?;
            if eof {
                // Owner EOF and drained queue are the terminal binding boundary.
                // Freeze before checking scope so a bind cannot race deletion.
                state.binding_closed.store(true, Ordering::Release);
            }
            *scope
        };
        if let Some(scope) = scope {
            if classifier.is_none() {
                persist(
                    repository,
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
                classifier = Some(classify::Classifier::new(
                    id, scope.flow, scope.node, transport,
                ));
            }
            persist_node_links(repository, scope, id, state, &mut linked, &mut failed).await;
            let classifier = classifier.as_mut().expect("classifier initialized");
            // This owner is the sole archive writer; its receipt is the committed watermark.
            while replay_cursor < receipt.persisted_through {
                let frames = repository.replay(id, replay_cursor).await?;
                if frames.is_empty() {
                    break;
                }
                for frame in frames {
                    let mut sink = PersistenceSink {
                        repository,
                        scope,
                        id,
                        at: &frame.observed_at,
                        failed: &mut failed,
                    };
                    if frame.kind == ClientTrajectoryFrameKind::Request {
                        classifier
                            .begin_request_into(&frame.observed_at, &mut sink)
                            .await;
                    }
                    for value in decoder.feed(frame.kind, &frame.bytes) {
                        classifier
                            .observe_into(frame.kind, value, &frame.observed_at, &mut sink)
                            .await;
                    }
                    replay_cursor = frame.sequence;
                }
            }
        }
        if eof {
            break;
        }
    }
    let final_scope = state.scope.lock().ok().and_then(|s| *s);
    if final_scope.is_none() {
        // No run can acquire this capture after the final owner boundary. Release
        // pre-bind bytes explicitly; a failed cleanup makes complete() fail.
        repository.discard_unbound(id).await?;
        receipt.persisted_through = 0;
    }
    if let (Some(scope), Some(classifier)) = (final_scope, classifier) {
        decoder.finish();
        let dropped = state.dropped.load(Ordering::Acquire)
            + u64::from(
                decoder.incomplete
                    || classifier.incomplete
                    || !classifier.completed
                    || !classifier.request_seen,
            );
        persist_node_links(repository, scope, id, state, &mut linked, &mut failed).await;
        persist(
            repository,
            scope,
            id,
            observed_at(),
            ClientTrajectoryFact::Integrity {
                status: if dropped == 0 && failed == 0 {
                    "complete"
                } else {
                    "incomplete"
                }
                .into(),
                dropped_count: dropped,
                persist_failed_count: failed,
            },
            &mut failed,
        )
        .await;
    }
    anyhow::ensure!(
        failed == 0,
        "client trajectory classification persistence failed"
    );
    Ok(receipt)
}
