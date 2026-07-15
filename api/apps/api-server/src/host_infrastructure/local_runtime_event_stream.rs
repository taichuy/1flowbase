use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        Arc, Mutex,
    },
};

use anyhow::{anyhow, Result};
use control_plane::ports::{
    ensure_ephemeral_payload_size, ephemeral_metadata_size_bytes, EphemeralEntrySnapshot,
    EphemeralEntryValueSnapshot, EphemeralInspectionCapabilities, EphemeralValueRevealMode,
    RuntimeEventCloseReason, RuntimeEventDurability, RuntimeEventEnvelope,
    RuntimeEventOverflowBehavior, RuntimeEventPayload, RuntimeEventStream,
    RuntimeEventStreamPolicy, RuntimeEventSubscription, RuntimeEventTrimPolicy,
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::{broadcast, mpsc, watch};
use uuid::Uuid;

const DEFAULT_BROADCAST_CAPACITY: usize = 1024;
const WAITING_RUN_RETENTION: TimeDuration = TimeDuration::hours(24);
const ORPHAN_RUN_RETENTION: TimeDuration = TimeDuration::hours(72);

#[derive(Clone)]
pub struct LocalRuntimeEventStream {
    runs: Arc<Mutex<HashMap<Uuid, Arc<LocalRunEventStream>>>>,
    broadcast_capacity: usize,
}

struct LocalRunEventStream {
    next_sequence: AtomicI64,
    ring: Mutex<VecDeque<RuntimeEventEnvelope>>,
    broadcaster: broadcast::Sender<RuntimeEventEnvelope>,
    closed_sender: watch::Sender<bool>,
    policy: RuntimeEventStreamPolicy,
    closed: AtomicBool,
    close_reason: Mutex<Option<RuntimeEventCloseReason>>,
    closed_at: Mutex<Option<OffsetDateTime>>,
    last_event_at: Mutex<OffsetDateTime>,
}

impl Default for LocalRuntimeEventStream {
    fn default() -> Self {
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
            broadcast_capacity: DEFAULT_BROADCAST_CAPACITY,
        }
    }
}

impl LocalRuntimeEventStream {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn with_broadcast_capacity_for_tests(broadcast_capacity: usize) -> Self {
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
            broadcast_capacity: broadcast_capacity.max(1),
        }
    }

    fn run(&self, run_id: Uuid) -> Result<Arc<LocalRunEventStream>> {
        self.purge_expired_runs();
        self.runs
            .lock()
            .expect("runtime event stream runs lock poisoned")
            .get(&run_id)
            .cloned()
            .ok_or_else(|| anyhow!("runtime event stream is not open"))
    }

    fn purge_expired_runs(&self) {
        let now = OffsetDateTime::now_utc();
        self.runs
            .lock()
            .expect("runtime event stream runs lock poisoned")
            .retain(|_, run| !run.expired_at(now));
    }

    #[cfg(test)]
    pub(crate) fn set_run_timestamps_for_tests(
        &self,
        run_id: Uuid,
        last_event_at: OffsetDateTime,
        closed_at: Option<OffsetDateTime>,
    ) -> Result<()> {
        let run = self.run(run_id)?;
        *run.last_event_at
            .lock()
            .expect("runtime event last event lock poisoned") = last_event_at;
        *run.closed_at
            .lock()
            .expect("runtime event closed_at lock poisoned") = closed_at;
        Ok(())
    }

    fn entry_key(run_id: Uuid, sequence: i64) -> String {
        format!("{run_id}:{sequence}")
    }

    fn parse_entry_key(key: &str) -> Option<(Uuid, i64)> {
        let (run_id, sequence) = key.rsplit_once(':')?;
        Some((Uuid::parse_str(run_id).ok()?, sequence.parse().ok()?))
    }

    fn event_value_size_bytes(event: &RuntimeEventEnvelope) -> u64 {
        serde_json::to_vec(&event.payload)
            .map(|bytes| bytes.len() as u64)
            .unwrap_or(0)
    }

    fn event_snapshot(
        event: &RuntimeEventEnvelope,
        run: &LocalRunEventStream,
        now: OffsetDateTime,
    ) -> EphemeralEntrySnapshot {
        let key = Self::entry_key(event.run_id, event.sequence);
        let expires_at = run.retention_deadline();
        let ttl_seconds = Some((expires_at - now).whole_seconds().max(0));
        let metadata = serde_json::json!({
            "run_id": event.run_id,
            "node_run_id": event.node_run_id,
            "sequence": event.sequence,
            "event_id": event.event_id,
            "event_type": event.event_type,
            "source": event.source,
            "durability": event.durability,
            "persist_required": event.persist_required,
            "trace_visible": event.trace_visible,
            "delta_index": event.delta_index,
            "content_type": event.content_type,
            "text_size_bytes": event.text.as_ref().map(|value| value.len()),
            "retention_expires_at_unix": expires_at.unix_timestamp(),
        });
        EphemeralEntrySnapshot {
            contract_code: "runtime-event-stream".to_string(),
            group_code: Some(event.run_id.to_string()),
            entry_ref: key.clone(),
            key,
            inspection_path: vec![event.run_id.to_string(), event.sequence.to_string()],
            entry_kind: "runtime_event".to_string(),
            status: if run.closed.load(Ordering::SeqCst) {
                "closed".to_string()
            } else {
                "open".to_string()
            },
            owner: event.node_run_id.map(|value| value.to_string()),
            value_size_bytes: Self::event_value_size_bytes(event),
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds,
            created_at_unix: Some(event.occurred_at.unix_timestamp()),
            expires_at_unix: Some(expires_at.unix_timestamp()),
            sensitive: true,
            metadata,
        }
    }
}

impl LocalRunEventStream {
    fn new(policy: RuntimeEventStreamPolicy, broadcast_capacity: usize) -> Self {
        let (broadcaster, _) = broadcast::channel(broadcast_capacity);
        let (closed_sender, _) = watch::channel(false);
        let now = OffsetDateTime::now_utc();
        Self {
            next_sequence: AtomicI64::new(1),
            ring: Mutex::new(VecDeque::new()),
            broadcaster,
            closed_sender,
            policy,
            closed: AtomicBool::new(false),
            close_reason: Mutex::new(None),
            closed_at: Mutex::new(None),
            last_event_at: Mutex::new(now),
        }
    }

    fn retention_duration(&self) -> TimeDuration {
        match *self
            .close_reason
            .lock()
            .expect("runtime event close reason lock poisoned")
        {
            Some(RuntimeEventCloseReason::WaitingHuman)
            | Some(RuntimeEventCloseReason::WaitingCallback) => WAITING_RUN_RETENTION,
            Some(_) => self.policy.ttl,
            None => ORPHAN_RUN_RETENTION,
        }
    }

    fn retention_deadline(&self) -> OffsetDateTime {
        let start = self
            .closed_at
            .lock()
            .expect("runtime event closed_at lock poisoned")
            .unwrap_or_else(|| {
                *self
                    .last_event_at
                    .lock()
                    .expect("runtime event last event lock poisoned")
            });
        start + self.retention_duration()
    }

    fn expired_at(&self, now: OffsetDateTime) -> bool {
        now >= self.retention_deadline()
    }

    fn replay_from_ring(
        &self,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        let requested_sequence = from_sequence.unwrap_or(0);
        let ring = self.ring.lock().expect("runtime event ring lock poisoned");

        if let Some(front) = ring.front() {
            if requested_sequence < front.sequence - 1 {
                return Err(anyhow!("runtime event replay expired"));
            }
        } else if requested_sequence < self.next_sequence.load(Ordering::SeqCst) - 1 {
            return Err(anyhow!("runtime event replay expired"));
        }

        Ok(ring
            .iter()
            .filter(|event| event.sequence > requested_sequence)
            .take(limit)
            .cloned()
            .collect())
    }

    fn events_after_sequence(&self, sequence: i64, limit: usize) -> Vec<RuntimeEventEnvelope> {
        let ring = self.ring.lock().expect("runtime event ring lock poisoned");
        ring.iter()
            .filter(|event| event.sequence > sequence)
            .take(limit)
            .cloned()
            .collect()
    }

    fn trim_overflow(&self, ring: &mut VecDeque<RuntimeEventEnvelope>) {
        match self.policy.overflow_behavior {
            RuntimeEventOverflowBehavior::DropOldEphemeralKeepRequired => {
                while ring.len() > self.policy.max_events {
                    if let Some(index) = ring.iter().position(|event| !is_required_event(event)) {
                        ring.remove(index);
                    } else {
                        break;
                    }
                }
            }
        }
    }
}

fn is_required_event(event: &RuntimeEventEnvelope) -> bool {
    event.persist_required
        || matches!(
            event.durability,
            RuntimeEventDurability::DurableRequired | RuntimeEventDurability::AuditRequired
        )
}

fn send_retained_after_sequence(
    run: &LocalRunEventStream,
    sender: &mpsc::UnboundedSender<RuntimeEventEnvelope>,
    last_sent_sequence: &mut i64,
) -> bool {
    for event in run.events_after_sequence(*last_sent_sequence, usize::MAX) {
        let sequence = event.sequence;
        if sender.send(event).is_err() {
            return false;
        }
        *last_sent_sequence = sequence;
    }
    true
}

#[async_trait::async_trait]
impl RuntimeEventStream for LocalRuntimeEventStream {
    async fn open_run(&self, run_id: Uuid, policy: RuntimeEventStreamPolicy) -> Result<()> {
        self.purge_expired_runs();
        let mut runs = self
            .runs
            .lock()
            .expect("runtime event stream runs lock poisoned");
        match runs.get(&run_id) {
            Some(run) if run.closed.load(Ordering::SeqCst) => {
                runs.insert(
                    run_id,
                    Arc::new(LocalRunEventStream::new(policy, self.broadcast_capacity)),
                );
            }
            Some(_) => {}
            None => {
                runs.insert(
                    run_id,
                    Arc::new(LocalRunEventStream::new(policy, self.broadcast_capacity)),
                );
            }
        }
        Ok(())
    }

    async fn append(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<RuntimeEventEnvelope> {
        ensure_ephemeral_payload_size(&event.payload)?;
        let run = self.run(run_id)?;

        let envelope = {
            let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
            if run.closed.load(Ordering::SeqCst) {
                return Err(anyhow!("runtime event stream is closed"));
            }

            let sequence = run.next_sequence.fetch_add(1, Ordering::SeqCst);
            let envelope = RuntimeEventEnvelope::new(run_id, sequence, event);
            *run.last_event_at
                .lock()
                .expect("runtime event last event lock poisoned") = envelope.occurred_at;
            ring.push_back(envelope.clone());
            run.trim_overflow(&mut ring);
            envelope
        };

        let _ = run.broadcaster.send(envelope.clone());
        Ok(envelope)
    }

    async fn subscribe(
        &self,
        run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> Result<RuntimeEventSubscription> {
        let run = self.run(run_id)?;
        let mut live_receiver = run.broadcaster.subscribe();
        let replay = run.replay_from_ring(from_sequence, usize::MAX)?;
        let mut last_sent_sequence = replay
            .last()
            .map(|event| event.sequence)
            .unwrap_or_else(|| from_sequence.unwrap_or(0));
        let (sender, live_events) = mpsc::unbounded_channel();

        if run.closed.load(Ordering::SeqCst) {
            drop(sender);
            return Ok(RuntimeEventSubscription {
                replay,
                live_events,
            });
        }

        let live_run = Arc::clone(&run);
        let mut closed_receiver = run.closed_sender.subscribe();
        if *closed_receiver.borrow() {
            drop(sender);
            return Ok(RuntimeEventSubscription {
                replay,
                live_events,
            });
        }

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = closed_receiver.changed() => {
                        if changed.is_err() || *closed_receiver.borrow() {
                            let _ = send_retained_after_sequence(
                                &live_run,
                                &sender,
                                &mut last_sent_sequence,
                            );
                            break;
                        }
                    }
                    received = live_receiver.recv() => {
                        match received {
                            Ok(event) if event.sequence <= last_sent_sequence => {}
                            Ok(event) => {
                                if !send_retained_after_sequence(
                                    &live_run,
                                    &sender,
                                    &mut last_sent_sequence,
                                ) {
                                    break;
                                }
                                if event.sequence <= last_sent_sequence {
                                    continue;
                                }
                                let sequence = event.sequence;
                                if sender.send(event).is_err() {
                                    break;
                                }
                                last_sent_sequence = sequence;
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                if !send_retained_after_sequence(
                                    &live_run,
                                    &sender,
                                    &mut last_sent_sequence,
                                ) {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });

        Ok(RuntimeEventSubscription {
            replay,
            live_events,
        })
    }

    async fn replay(
        &self,
        run_id: Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        self.run(run_id)?.replay_from_ring(from_sequence, limit)
    }

    async fn close_run(&self, run_id: Uuid, reason: RuntimeEventCloseReason) -> Result<()> {
        let run = self.run(run_id)?;
        let _ring = run.ring.lock().expect("runtime event ring lock poisoned");
        if !run.closed.swap(true, Ordering::SeqCst) {
            *run.close_reason
                .lock()
                .expect("runtime event close reason lock poisoned") = Some(reason);
            *run.closed_at
                .lock()
                .expect("runtime event closed_at lock poisoned") = Some(OffsetDateTime::now_utc());
            let _ = run.closed_sender.send(true);
        }
        Ok(())
    }

    async fn trim(&self, run_id: Uuid, policy: RuntimeEventTrimPolicy) -> Result<()> {
        let run = self.run(run_id)?;
        if let Some(before_sequence) = policy.before_sequence {
            let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
            ring.retain(|event| {
                event.sequence >= before_sequence
                    || (policy.keep_required && is_required_event(event))
            });
            if ring.len() > run.policy.max_events {
                run.trim_overflow(&mut ring);
            }
        }
        Ok(())
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::supported()
    }

    async fn list_ephemeral_entries(&self) -> Result<Vec<EphemeralEntrySnapshot>> {
        self.purge_expired_runs();
        let runs = self
            .runs
            .lock()
            .expect("runtime event stream runs lock poisoned")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut entries = Vec::new();
        let now = OffsetDateTime::now_utc();
        for run in runs {
            let ring = run.ring.lock().expect("runtime event ring lock poisoned");
            entries.extend(
                ring.iter()
                    .map(|event| Self::event_snapshot(event, &run, now))
                    .collect::<Vec<_>>(),
            );
        }
        entries.sort_by(|left, right| {
            left.group_code
                .cmp(&right.group_code)
                .then(left.key.cmp(&right.key))
        });
        Ok(entries)
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> Result<Option<EphemeralEntryValueSnapshot>> {
        self.purge_expired_runs();
        let Some((run_id, sequence)) = Self::parse_entry_key(entry_ref) else {
            return Ok(None);
        };
        let Some(run) = self
            .runs
            .lock()
            .expect("runtime event stream runs lock poisoned")
            .get(&run_id)
            .cloned()
        else {
            return Ok(None);
        };
        let ring = run.ring.lock().expect("runtime event ring lock poisoned");
        let Some(event) = ring
            .iter()
            .find(|event| event.sequence == sequence)
            .cloned()
        else {
            return Ok(None);
        };
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            Self::event_snapshot(&event, &run, OffsetDateTime::now_utc()),
            event.payload,
            reveal_mode,
        )))
    }
}
