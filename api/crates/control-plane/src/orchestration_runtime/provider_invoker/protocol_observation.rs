use super::*;

/// Invocation-local capture identity and completeness; independent of business outcome.
pub(super) struct Capture {
    node: Option<(String, Uuid)>,
    invocation_id: Uuid,
    provider_attempt_index: u64,
    observed_count: u64,
    pub(super) persist_failed_count: u64,
    ended: bool,
    dropped_count: u64,
    capture_failed: bool,
}

impl Capture {
    pub(super) fn new(node: Option<(String, Uuid)>, input: &ProviderInvocationInput) -> Self {
        Self {
            node,
            invocation_id: input
                .trace_context
                .get("provider_invocation_id")
                .and_then(|id| Uuid::parse_str(id).ok())
                .unwrap_or_else(Uuid::now_v7),
            provider_attempt_index: input
                .trace_context
                .get("provider_attempt_index")
                .and_then(|index| index.parse().ok())
                .unwrap_or(0),
            observed_count: 0,
            persist_failed_count: 0,
            ended: false,
            dropped_count: 0,
            capture_failed: false,
        }
    }

    fn payload(
        &self,
        flow_run_id: Uuid,
        event_type: &str,
        mut payload: Value,
    ) -> Option<crate::ports::RuntimeEventPayload> {
        let (node_id, node_run_id) = self.node.as_ref()?;
        let object = payload.as_object_mut()?;
        object.insert("type".into(), json!(event_type));
        object.insert("flow_run_id".into(), json!(flow_run_id));
        object.insert("node_id".into(), json!(node_id));
        object.insert("node_run_id".into(), json!(node_run_id));
        object.insert("invocation_id".into(), json!(self.invocation_id));
        object.insert(
            "provider_attempt_index".into(),
            json!(self.provider_attempt_index),
        );
        Some(crate::ports::RuntimeEventPayload {
            event_type: event_type.into(),
            source: crate::ports::RuntimeEventSource::Provider,
            durability: RuntimeEventDurability::DurableRequired,
            persist_required: true,
            trace_visible: false,
            payload,
        })
    }

    pub(super) fn observe(
        &mut self,
        flow_run_id: Option<Uuid>,
        event: &ProviderStreamEvent,
    ) -> Option<crate::ports::RuntimeEventPayload> {
        let ProviderStreamEvent::ProtocolObservation { kind, .. } = event else {
            return None;
        };
        self.observed_count += 1;
        if kind == "capture_integrity" {
            self.capture_failed = true;
            if let ProviderStreamEvent::ProtocolObservation { body, .. } = event {
                let count = serde_json::from_str::<Value>(body)
                    .ok()
                    .and_then(|v| v.get("dropped_count").and_then(Value::as_u64));
                self.dropped_count = self.dropped_count.saturating_add(count.unwrap_or(1));
            }
        }
        // A new transport request after a completed exchange requires fresh end evidence.
        if kind == "request_prepared" {
            self.ended = false;
        }
        if kind == "stream_end" {
            self.ended = true;
        }
        let mut payload = serde_json::to_value(event).ok()?;
        payload["sequence"] = json!(self.observed_count);
        self.payload(flow_run_id?, "provider_protocol_observation", payload)
    }

    pub(super) fn integrity(
        &self,
        flow_run_id: Uuid,
        invocation_ok: bool,
    ) -> Option<crate::ports::RuntimeEventPayload> {
        self.payload(flow_run_id, "provider_protocol_integrity", json!({
            "observed_count": self.observed_count,
            "persist_failed_count": self.persist_failed_count,
            "dropped_count": self.dropped_count,
            "status": if self.dropped_count > 0 || self.capture_failed || self.persist_failed_count > 0 || !invocation_ok { "incomplete" }
                else if self.observed_count == 0 { "unavailable" }
                else if self.ended { "complete" }
                else { "incomplete" },
        }))
    }
}

// At most 32 writers, 32 queued observations and 1 MiB retained per invocation.
// These limits include the event currently being persisted; admission never waits.
const QUEUE_CAPACITY: usize = 32;
const BYTE_CAPACITY: usize = 1024 * 1024;
const WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
const DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
static WRITERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(32);

#[derive(Debug)]
struct Observation {
    event: ProviderStreamEvent,
    sequence: u64,
    _bytes: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Debug)]
struct Sink {
    sender: mpsc::Sender<Observation>,
    bytes: Arc<tokio::sync::Semaphore>,
    dropped: Arc<std::sync::atomic::AtomicU64>,
    sequence: std::sync::atomic::AtomicU64,
}

impl runtime_core::runtime_backend::RuntimeProtocolObservationSink for Sink {
    fn observe(&self, event: ProviderStreamEvent) {
        use std::sync::atomic::Ordering::Relaxed;
        let ProviderStreamEvent::ProtocolObservation {
            protocol,
            transport,
            direction,
            kind,
            body,
            encoding,
            ..
        } = &event
        else {
            return;
        };
        let size = [protocol, transport, direction, kind, body, encoding]
            .iter()
            .fold(512usize, |total, value| total.saturating_add(value.len()));
        let sequence = self.sequence.fetch_add(1, Relaxed) + 1;
        let permit = u32::try_from(size)
            .ok()
            .filter(|size| *size as usize <= BYTE_CAPACITY)
            .and_then(|size| self.bytes.clone().try_acquire_many_owned(size).ok());
        let Some(permit) = permit else {
            self.dropped.fetch_add(1, Relaxed);
            return;
        };
        if self
            .sender
            .try_send(Observation {
                event,
                sequence,
                _bytes: permit,
            })
            .is_err()
        {
            self.dropped.fetch_add(1, Relaxed);
        }
    }
}

/// Dropping the invocation future is cancellation evidence, not a successful end.
#[derive(Default)]
pub(super) struct Completion(Option<tokio::sync::oneshot::Sender<bool>>);
impl Completion {
    pub(super) fn finish(mut self, success: bool) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(success);
        }
    }
}
impl Drop for Completion {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(false);
        }
    }
}

async fn persist<F, Fut>(writer: &mut F, payload: &crate::ports::RuntimeEventPayload) -> bool
where
    F: FnMut(crate::ports::RuntimeEventPayload) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    matches!(
        tokio::time::timeout(WRITE_TIMEOUT, writer(payload.clone())).await,
        Ok(true)
    )
}

pub(super) fn start<
    R: crate::ports::OrchestrationRuntimeRepository + Clone + Send + Sync + 'static,
>(
    repository: R,
    run: Option<Uuid>,
    capture: Capture,
) -> (
    Option<Arc<dyn runtime_core::runtime_backend::RuntimeProtocolObservationSink>>,
    Completion,
) {
    let Some(run) = run.filter(|_| capture.node.is_some()) else {
        return (None, Completion::default());
    };
    let Ok(permit) = WRITERS.try_acquire() else {
        tracing::warn!(%run, reason = "writer_capacity_exceeded", "provider protocol capture not recorded");
        return (None, Completion::default());
    };
    let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
    let dropped = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let sink = Arc::new(Sink {
        sender,
        bytes: Arc::new(tokio::sync::Semaphore::new(BYTE_CAPACITY)),
        dropped: dropped.clone(),
        sequence: std::sync::atomic::AtomicU64::new(0),
    });
    let (done, completion) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _permit = permit;
        write(
            run,
            capture,
            receiver,
            dropped,
            completion,
            move |payload| {
                let repository = repository.clone();
                async move {
                    runtime_event_persister::persist_runtime_event_payload(
                        &repository,
                        run,
                        &payload,
                    )
                    .await
                    .is_ok()
                }
            },
        )
        .await;
    });
    (Some(sink), Completion(Some(done)))
}

async fn write<F, Fut>(
    run: Uuid,
    mut capture: Capture,
    mut receiver: mpsc::Receiver<Observation>,
    dropped: Arc<std::sync::atomic::AtomicU64>,
    mut completion: tokio::sync::oneshot::Receiver<bool>,
    mut writer: F,
) where
    F: FnMut(crate::ports::RuntimeEventPayload) -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    use std::sync::atomic::Ordering::Relaxed;
    let mut projector = super::semantic_trajectory::SemanticProjector::default();
    let mut host_dropped = 0;
    let mut success = false;
    let mut done = false;
    let mut drain_deadline = None;
    // A durable pending marker makes cancellation/crash honest even before the first raw event.
    if let Some(integrity) = capture.integrity(run, false) {
        if !persist(&mut writer, &integrity).await {
            capture.persist_failed_count += 1;
        }
    }
    loop {
        let observation = tokio::select! {
            result = &mut completion, if !done => {
                success = result.unwrap_or(false);
                done = true;
                receiver.close();
                drain_deadline = Some(tokio::time::Instant::now() + DRAIN_TIMEOUT);
                continue;
            }
            _ = async { if let Some(deadline) = drain_deadline { tokio::time::sleep_until(deadline).await } else { std::future::pending::<()>().await } } => {
                capture.capture_failed = true;
                capture.dropped_count += receiver.len() as u64;
                break;
            }
            observation = receiver.recv() => observation,
        };
        let Some(observation) = observation else {
            if !done {
                success = completion.await.unwrap_or(false);
            }
            break;
        };
        let count = dropped.load(Relaxed);
        if count != host_dropped {
            capture.dropped_count += count - host_dropped;
            host_dropped = count;
            if let Some(integrity) = capture.integrity(run, false) {
                projector.observe(&integrity);
            }
        }
        if let Some(mut payload) = capture.observe(Some(run), &observation.event) {
            payload.payload["sequence"] = json!(observation.sequence);
            // Bound the entire projection/write batch, not just each individual row.
            // A single raw frame may project multiple semantic steps.
            let deadline = drain_deadline
                .unwrap_or_else(|| tokio::time::Instant::now() + WRITE_TIMEOUT)
                .min(tokio::time::Instant::now() + WRITE_TIMEOUT);
            let batch = async {
                if persist(&mut writer, &payload).await {
                    for semantic in projector.observe(&payload) {
                        if !persist(&mut writer, &semantic).await {
                            capture.persist_failed_count += 1;
                        }
                    }
                } else {
                    capture.persist_failed_count += 1;
                }
            };
            if tokio::time::timeout_at(deadline, batch).await.is_err() {
                capture.persist_failed_count += 1;
            }
            if capture.persist_failed_count > 0 || capture.capture_failed {
                if let Some(integrity) = capture.integrity(run, false) {
                    projector.observe(&integrity);
                }
            }
        }
    }
    capture.dropped_count += dropped.load(Relaxed).saturating_sub(host_dropped);
    if let Some(integrity) = capture.integrity(run, success) {
        let batch = async {
            for semantic in projector.observe(&integrity) {
                if !persist(&mut writer, &semantic).await {
                    capture.persist_failed_count += 1;
                }
            }
        };
        if tokio::time::timeout(WRITE_TIMEOUT, batch).await.is_err() {
            capture.persist_failed_count += 1;
        }
        // Rebuild after semantic failures: never publish complete after a failed write.
        if let Some(integrity) = capture.integrity(run, success) {
            if !persist(&mut writer, &integrity).await {
                tracing::warn!(%run, "provider protocol integrity persistence failed");
            }
        }
    }
}

#[cfg(test)]
#[path = "_tests/protocol_observation.rs"]
mod tests;
