use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicI64, Ordering},
        Arc, Mutex, Weak,
    },
};

use anyhow::{anyhow, Result};
use control_plane::ports::{
    ensure_ephemeral_payload_size, ephemeral_metadata_size_bytes,
    AppendTerminalIfMissingAndCloseOutcome, EphemeralEntrySnapshot, EphemeralEntryValueSnapshot,
    EphemeralInspectionCapabilities, EphemeralValueRevealMode, RuntimeEventCloseReason,
    RuntimeEventClosure, RuntimeEventDiagnosticLane, RuntimeEventDurability, RuntimeEventEnvelope,
    RuntimeEventOverflowBehavior, RuntimeEventPayload, RuntimeEventReceiver,
    RuntimeEventRequiredLane, RuntimeEventStream, RuntimeEventStreamPolicy,
    RuntimeEventSubscription, RuntimeEventTerminalWriter, RuntimeEventTrimPolicy,
};
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::sync::{broadcast, watch};
use uuid::Uuid;

const DEFAULT_BROADCAST_CAPACITY: usize = 1024;
// A transfer batch, independent of the stream retention capacity.
const LIVE_BACKFILL_PAGE_SIZE: usize = 64;
const WAITING_RUN_RETENTION: TimeDuration = TimeDuration::hours(24);
const ORPHAN_RUN_RETENTION: TimeDuration = TimeDuration::hours(72);

#[derive(Clone)]
pub struct LocalRuntimeEventStream {
    runs: Arc<Mutex<HashMap<Uuid, Arc<LocalRunEventStream>>>>,
    broadcast_capacity: usize,
    schedule_updates: watch::Sender<()>,
    scheduler_started: Arc<AtomicBool>,
}

struct LocalRuntimeEventTerminalWriter {
    run_id: Uuid,
    run: Arc<LocalRunEventStream>,
    schedule_updates: watch::Sender<()>,
}

#[async_trait::async_trait]
impl RuntimeEventTerminalWriter for LocalRuntimeEventTerminalWriter {
    async fn append_terminal_if_missing_and_close(
        &self,
        event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        let outcome = self
            .run
            .append_terminal_if_missing_and_close(self.run_id, event)?;
        self.schedule_updates.send_replace(());
        Ok(outcome)
    }
}

struct LocalRunEventStream {
    next_sequence: AtomicI64,
    ring: Mutex<RetainedRuntimeEvents>,
    broadcaster: Mutex<Option<broadcast::Sender<RuntimeEventEnvelope>>>,
    closed_sender: watch::Sender<Option<RuntimeEventClosure>>,
    policy: RuntimeEventStreamPolicy,
    closed_at: Mutex<Option<OffsetDateTime>>,
    last_event_at: Mutex<OffsetDateTime>,
    subscribers: Mutex<Vec<Weak<LiveBackfill>>>,
}

#[derive(Default)]
struct RetainedRuntimeEvents {
    events: VecDeque<Arc<RetainedRuntimeEvent>>,
    bytes: usize,
    terminal_reason: Option<RuntimeEventCloseReason>,
    retired: bool,
}

// Only forwarding tasks own this lease. Expiry can transfer pending data to
// live consumers without letting a generation-bound terminal writer pin it.
struct LiveBackfill {
    sequence: AtomicI64,
    retired_events: Mutex<Option<Arc<VecDeque<Arc<RetainedRuntimeEvent>>>>>,
}

// The replay ring owns one compact wire representation. Live delivery keeps
// using the typed envelope; replay and inspection decode only when requested.
struct RetainedRuntimeEvent {
    sequence: i64,
    durability: RuntimeEventDurability,
    terminal_reason: Option<RuntimeEventCloseReason>,
    wire: Box<[u8]>,
}

impl RetainedRuntimeEvent {
    fn encode(event: &RuntimeEventEnvelope) -> Result<Self> {
        Ok(Self {
            sequence: event.sequence,
            durability: event.durability,
            terminal_reason: RuntimeEventCloseReason::from_terminal_event_type(&event.event_type),
            wire: serde_json::to_vec(event)?.into_boxed_slice(),
        })
    }

    fn decode(&self) -> Result<RuntimeEventEnvelope> {
        Ok(serde_json::from_slice(&self.wire)?)
    }

    fn is_required(&self) -> bool {
        is_required_durability(self.durability)
    }
}

impl Default for LocalRuntimeEventStream {
    fn default() -> Self {
        let (schedule_updates, _) = watch::channel(());
        Self {
            runs: Arc::new(Mutex::new(HashMap::new())),
            broadcast_capacity: DEFAULT_BROADCAST_CAPACITY,
            schedule_updates,
            scheduler_started: Arc::new(AtomicBool::new(false)),
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
            broadcast_capacity: broadcast_capacity.max(1),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(crate) fn contains_run_without_purge_for_tests(&self, run_id: Uuid) -> bool {
        self.runs
            .lock()
            .expect("runtime event stream runs lock poisoned")
            .contains_key(&run_id)
    }

    #[cfg(test)]
    pub(crate) fn run_probe_for_tests(
        &self,
        run_id: Uuid,
    ) -> Result<impl Fn() -> (usize, usize, usize)> {
        let run = Arc::downgrade(&self.run(run_id)?);
        Ok(move || {
            let strong_count = run.strong_count();
            let Some(run) = run.upgrade() else {
                return (0, 0, 0);
            };
            let ring = run.ring.lock().expect("runtime event ring lock poisoned");
            (strong_count, ring.bytes, ring.events.capacity())
        })
    }

    fn start_expiry_scheduler(&self) {
        if self.scheduler_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let runs = Arc::downgrade(&self.runs);
        let updates = self.schedule_updates.subscribe();
        tokio::spawn(async move { Self::expire_idle_runs(runs, updates).await });
    }

    async fn expire_idle_runs(
        runs: Weak<Mutex<HashMap<Uuid, Arc<LocalRunEventStream>>>>,
        mut updates: watch::Receiver<()>,
    ) {
        loop {
            // Register the revision before inspecting deadlines, so a mutation during
            // the scan cannot be lost before the timer or wait is armed.
            updates.borrow_and_update();
            let Some(runs) = runs.upgrade() else { break };
            let next_deadline = Self::purge_and_next_deadline(&runs);
            drop(runs);
            match next_deadline {
                Some(deadline) => {
                    let delay = (deadline - OffsetDateTime::now_utc())
                        .try_into()
                        .unwrap_or(std::time::Duration::ZERO);
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {},
                        changed = updates.changed() => { if changed.is_err() { break; } }
                    }
                }
                None => {
                    if updates.changed().await.is_err() {
                        break;
                    }
                }
            }
        }
    }

    fn purge_and_next_deadline(
        runs: &Mutex<HashMap<Uuid, Arc<LocalRunEventStream>>>,
    ) -> Option<OffsetDateTime> {
        let now = OffsetDateTime::now_utc();
        let mut runs = runs
            .lock()
            .expect("runtime event stream runs lock poisoned");
        runs.retain(|_, run| {
            if run.expired_at(now) {
                run.retire_payloads();
                false
            } else {
                true
            }
        });
        runs.values().map(|run| run.retention_deadline()).min()
    }

    #[cfg(test)]
    pub(crate) fn has_live_broadcast_for_tests(&self, run_id: Uuid) -> Result<bool> {
        let run = self.run(run_id)?;
        let has_broadcast = run
            .broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .is_some();
        Ok(has_broadcast)
    }

    fn run(&self, run_id: Uuid) -> Result<Arc<LocalRunEventStream>> {
        let mut runs = self
            .runs
            .lock()
            .expect("runtime event stream runs lock poisoned");
        let run = runs
            .get(&run_id)
            .ok_or_else(|| anyhow!("runtime event stream is not open"))?;
        if run.expired_at(OffsetDateTime::now_utc()) {
            run.retire_payloads();
            runs.remove(&run_id);
            return Err(anyhow!("runtime event stream is not open"));
        }
        Ok(Arc::clone(run))
    }

    fn purge_expired_runs(&self) {
        Self::purge_and_next_deadline(&self.runs);
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
        self.schedule_updates.send_replace(());
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
            status: if run.is_closed() {
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
    fn append_terminal_if_missing_and_close(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        let incoming_reason = RuntimeEventCloseReason::from_terminal_event_type(&event.event_type)
            .ok_or_else(|| {
                anyhow!("runtime event stream terminal append requires a terminal event")
            })?;
        ensure_ephemeral_payload_size(&event.payload)?;
        let run = self;

        let outcome = {
            // `ring` is the stream's serialization point for append and close. Holding it for
            // the terminal check, optional append, and closure prevents concurrent EOF recovery
            // retries from observing the same missing terminal.
            let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
            let existing_terminal_reason = ring.terminal_reason;

            if run.is_closed() {
                if existing_terminal_reason.is_some() {
                    return Ok(AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal);
                }
                return Err(anyhow!(
                    "runtime event stream is closed without a terminal event"
                ));
            }

            if ring.retired && existing_terminal_reason.is_none() {
                return Err(anyhow!(
                    "runtime event stream is closed without a terminal event"
                ));
            }

            let (outcome, close_reason) = if let Some(existing_reason) = existing_terminal_reason {
                (
                    AppendTerminalIfMissingAndCloseOutcome::ExistingTerminal,
                    existing_reason,
                )
            } else {
                let sequence = run.next_sequence.load(Ordering::SeqCst);
                let envelope = RuntimeEventEnvelope::new(run_id, sequence, event);
                let retained = RetainedRuntimeEvent::encode(&envelope)?;
                let retained_bytes = retained.wire.len();
                run.make_room_for(&mut ring, retained_bytes)?;
                run.next_sequence.store(sequence + 1, Ordering::SeqCst);
                *run.last_event_at
                    .lock()
                    .expect("runtime event last event lock poisoned") = envelope.occurred_at;
                ring.bytes = ring.bytes.saturating_add(retained_bytes);
                ring.terminal_reason = Some(incoming_reason);
                ring.events.push_back(Arc::new(retained));
                (
                    AppendTerminalIfMissingAndCloseOutcome::Appended,
                    incoming_reason,
                )
            };

            let final_sequence = run.next_sequence.load(Ordering::SeqCst) - 1;
            *run.closed_at
                .lock()
                .expect("runtime event closed_at lock poisoned") = Some(OffsetDateTime::now_utc());
            run.closed_sender.send_replace(Some(RuntimeEventClosure {
                reason: close_reason,
                final_sequence,
            }));
            // Subscribers backfill the retained terminal when they observe closure.
            run.release_live_broadcast();
            outcome
        };
        Ok(outcome)
    }

    fn new(policy: RuntimeEventStreamPolicy, broadcast_capacity: usize) -> Self {
        let (broadcaster, _) = broadcast::channel(broadcast_capacity);
        let (closed_sender, _) = watch::channel(None);
        let now = OffsetDateTime::now_utc();
        Self {
            next_sequence: AtomicI64::new(1),
            ring: Mutex::new(RetainedRuntimeEvents::default()),
            broadcaster: Mutex::new(Some(broadcaster)),
            closed_sender,
            policy,
            closed_at: Mutex::new(None),
            last_event_at: Mutex::new(now),
            subscribers: Mutex::new(Vec::new()),
        }
    }

    fn retire_payloads(&self) {
        let mut ring = self.ring.lock().expect("runtime event ring lock poisoned");
        if ring.retired {
            return;
        }
        let subscribers = self
            .subscribers
            .lock()
            .expect("runtime event subscribers lock poisoned")
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        let mut events = std::mem::take(&mut ring.events);
        ring.bytes = 0;
        ring.retired = true;
        if let Some(minimum_sequence) = subscribers
            .iter()
            .map(|subscriber| subscriber.sequence.load(Ordering::Acquire))
            .min()
        {
            events.retain(|event| event.sequence > minimum_sequence);
            let events = Arc::new(events);
            for subscriber in subscribers {
                *subscriber
                    .retired_events
                    .lock()
                    .expect("runtime event backfill lock poisoned") = Some(Arc::clone(&events));
            }
        }
        // Wake orphan forwarding tasks as well as closed generations. A missing
        // terminal stays missing; dropping the broadcaster is not a terminal fact.
        self.release_live_broadcast();
    }

    fn subscribe_backfill(
        &self,
        from_sequence: Option<i64>,
    ) -> Result<(Arc<LiveBackfill>, Vec<RuntimeEventEnvelope>)> {
        let requested_sequence = from_sequence.unwrap_or(0);
        let (subscriber, retained) = {
            let ring = self.ring.lock().expect("runtime event ring lock poisoned");
            if ring.retired {
                return Err(anyhow!("runtime event replay expired"));
            }
            Self::validate_replay_cursor(
                &ring,
                requested_sequence,
                self.next_sequence.load(Ordering::SeqCst),
            )?;
            let subscriber = Arc::new(LiveBackfill {
                sequence: AtomicI64::new(requested_sequence),
                retired_events: Mutex::new(None),
            });
            let mut subscribers = self
                .subscribers
                .lock()
                .expect("runtime event subscribers lock poisoned");
            subscribers.retain(|subscriber| subscriber.strong_count() > 0);
            subscribers.push(Arc::downgrade(&subscriber));
            let retained = ring
                .events
                .iter()
                .filter(|event| event.sequence > requested_sequence)
                .cloned()
                .collect::<Vec<_>>();
            (subscriber, retained)
        };
        let replay = retained
            .iter()
            .map(|event| event.decode())
            .collect::<Result<Vec<_>>>()?;
        Ok((subscriber, replay))
    }

    fn validate_replay_cursor(
        ring: &RetainedRuntimeEvents,
        requested_sequence: i64,
        next_sequence: i64,
    ) -> Result<()> {
        if let Some(front) = ring.events.front() {
            if requested_sequence < front.sequence - 1 {
                return Err(anyhow!("runtime event replay expired"));
            }
        } else if requested_sequence < next_sequence - 1 {
            return Err(anyhow!("runtime event replay expired"));
        }
        Ok(())
    }

    fn subscribe_live(&self) -> broadcast::Receiver<RuntimeEventEnvelope> {
        self.broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .as_ref()
            .map(broadcast::Sender::subscribe)
            .unwrap_or_else(|| broadcast::channel(1).1)
    }

    fn broadcast(&self, event: RuntimeEventEnvelope) {
        if let Some(sender) = self
            .broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .as_ref()
        {
            let _ = sender.send(event);
        }
    }

    fn release_live_broadcast(&self) {
        self.broadcaster
            .lock()
            .expect("runtime event broadcaster lock poisoned")
            .take();
    }

    fn retention_duration(&self) -> TimeDuration {
        match self.closed_sender.borrow().map(|closure| closure.reason) {
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

    fn is_closed(&self) -> bool {
        self.closed_sender.borrow().is_some()
    }

    fn replay_from_ring(
        &self,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        let requested_sequence = from_sequence.unwrap_or(0);
        let retained = {
            let ring = self.ring.lock().expect("runtime event ring lock poisoned");
            Self::validate_replay_cursor(
                &ring,
                requested_sequence,
                self.next_sequence.load(Ordering::SeqCst),
            )?;
            ring.events
                .iter()
                .filter(|event| event.sequence > requested_sequence)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        };
        retained.iter().map(|event| event.decode()).collect()
    }

    fn events_after_sequence(
        &self,
        sequence: i64,
        limit: usize,
        subscriber: &LiveBackfill,
    ) -> Result<Vec<RuntimeEventEnvelope>> {
        let retained = {
            // Retirement holds this same lock while handing off the ring, so a page
            // sees either the hot ring or the consumer-owned retired backlog.
            let ring = self.ring.lock().expect("runtime event ring lock poisoned");
            let retired_events = subscriber
                .retired_events
                .lock()
                .expect("runtime event backfill lock poisoned");
            let events = retired_events.as_deref().unwrap_or(&ring.events);
            events
                .iter()
                .filter(|event| event.sequence > sequence)
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        };
        retained.iter().map(|event| event.decode()).collect()
    }

    fn remove_retained_event(ring: &mut RetainedRuntimeEvents, index: usize) -> Result<()> {
        if let Some(event) = ring.events.remove(index) {
            ring.bytes = ring.bytes.saturating_sub(event.wire.len());
            if event.terminal_reason.is_some() {
                ring.terminal_reason = ring.events.iter().find_map(|event| event.terminal_reason);
            }
        }
        Ok(())
    }

    fn make_room_for(&self, ring: &mut RetainedRuntimeEvents, incoming_bytes: usize) -> Result<()> {
        match self.policy.overflow_behavior {
            RuntimeEventOverflowBehavior::DropOldEphemeralKeepRequired => {
                while ring.events.len() >= self.policy.max_events
                    || ring.bytes.saturating_add(incoming_bytes) > self.policy.max_bytes
                {
                    let Some(index) = ring.events.iter().position(|event| !event.is_required())
                    else {
                        let capacity = if ring.events.len() >= self.policy.max_events {
                            "event"
                        } else {
                            "byte"
                        };
                        return Err(anyhow!("runtime event stream {capacity} capacity exceeded"));
                    };
                    Self::remove_retained_event(ring, index)?;
                }
            }
        }
        Ok(())
    }

    fn trim_to_policy(&self, ring: &mut RetainedRuntimeEvents) -> Result<()> {
        while ring.events.len() > self.policy.max_events || ring.bytes > self.policy.max_bytes {
            let Some(index) = ring.events.iter().position(|event| !event.is_required()) else {
                break;
            };
            Self::remove_retained_event(ring, index)?;
        }
        Ok(())
    }
}

fn is_required_event(event: &RuntimeEventEnvelope) -> bool {
    is_required_durability(event.durability)
}

fn is_required_durability(durability: RuntimeEventDurability) -> bool {
    matches!(
        durability,
        RuntimeEventDurability::DurableRequired | RuntimeEventDurability::AuditRequired
    )
}

fn is_required_delivery_event(event: &RuntimeEventEnvelope) -> bool {
    is_required_event(event)
        || matches!(
            event.event_type.as_str(),
            "text_delta"
                | "reasoning_delta"
                | "reasoning_signature_delta"
                | "tool_call_delta"
                | "tool_call_commit"
                | "mcp_call_delta"
                | "mcp_call_commit"
                | "provider_responses_output_delta"
                | "provider_output_item_added"
                | "provider_output_item_done"
                | "usage_delta"
                | "usage_snapshot"
                | "usage_recorded"
                | "finish"
                | "flow_finished"
                | "flow_incomplete"
                | "flow_failed"
                | "flow_cancelled"
                | "waiting_human"
                | "waiting_callback"
        )
}

async fn send_retained_after_sequence(
    run: &LocalRunEventStream,
    required: &RuntimeEventRequiredLane,
    diagnostic: &RuntimeEventDiagnosticLane,
    last_sent_sequence: &mut i64,
    subscriber: &LiveBackfill,
) -> bool {
    // Freeze the upper bound so sustained producers cannot monopolize this task.
    let through_sequence = run.next_sequence.load(Ordering::SeqCst) - 1;
    while *last_sent_sequence < through_sequence {
        let events = match run.events_after_sequence(
            *last_sent_sequence,
            LIVE_BACKFILL_PAGE_SIZE,
            subscriber,
        ) {
            Ok(events) => events,
            Err(error) => {
                tracing::error!(%error, "runtime event replay decode failed");
                return false;
            }
        };
        if events.is_empty() || events[0].sequence > through_sequence {
            break;
        }
        for event in events
            .into_iter()
            .take_while(|event| event.sequence <= through_sequence)
        {
            let sequence = event.sequence;
            if is_required_delivery_event(&event) {
                if required.send(event).await.is_err() {
                    return false;
                }
            } else {
                let _ = diagnostic.try_send(event);
            }
            *last_sent_sequence = sequence;
            subscriber.sequence.store(sequence, Ordering::Release);
        }
    }
    true
}

#[async_trait::async_trait]
impl RuntimeEventStream for LocalRuntimeEventStream {
    async fn open_run(&self, run_id: Uuid, policy: RuntimeEventStreamPolicy) -> Result<()> {
        self.start_expiry_scheduler();
        self.purge_expired_runs();
        let mut runs = self
            .runs
            .lock()
            .expect("runtime event stream runs lock poisoned");
        match runs.get(&run_id) {
            Some(run) if run.is_closed() => {
                // The old generation is no longer reachable by the map scheduler.
                // Leave its pending live delivery with consumer leases only.
                run.retire_payloads();
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
        drop(runs);
        self.schedule_updates.send_replace(());
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
            if run.is_closed() || ring.retired {
                return Err(anyhow!("runtime event stream is closed"));
            }

            let sequence = run.next_sequence.load(Ordering::SeqCst);
            let envelope = RuntimeEventEnvelope::new(run_id, sequence, event);
            let retained = RetainedRuntimeEvent::encode(&envelope)?;
            let retained_bytes = retained.wire.len();
            run.make_room_for(&mut ring, retained_bytes)?;
            run.next_sequence.store(sequence + 1, Ordering::SeqCst);
            *run.last_event_at
                .lock()
                .expect("runtime event last event lock poisoned") = envelope.occurred_at;
            ring.bytes = ring.bytes.saturating_add(retained_bytes);
            if let Some(reason) = retained.terminal_reason {
                ring.terminal_reason.get_or_insert(reason);
            }
            ring.events.push_back(Arc::new(retained));
            envelope
        };

        run.broadcast(envelope.clone());
        Ok(envelope)
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        run_id: Uuid,
        event: RuntimeEventPayload,
    ) -> Result<AppendTerminalIfMissingAndCloseOutcome> {
        let outcome = self
            .run(run_id)?
            .append_terminal_if_missing_and_close(run_id, event)?;
        self.schedule_updates.send_replace(());
        Ok(outcome)
    }

    async fn subscribe(
        &self,
        run_id: Uuid,
        from_sequence: Option<i64>,
    ) -> Result<RuntimeEventSubscription> {
        let run = self.run(run_id)?;
        let terminal_writer: Arc<dyn RuntimeEventTerminalWriter> =
            Arc::new(LocalRuntimeEventTerminalWriter {
                run_id,
                run: Arc::clone(&run),
                schedule_updates: self.schedule_updates.clone(),
            });
        let mut live_receiver = run.subscribe_live();
        let closure = run.closed_sender.subscribe();
        let (subscriber, replay) = run.subscribe_backfill(from_sequence)?;
        let mut last_sent_sequence = replay
            .last()
            .map(|event| event.sequence)
            .unwrap_or_else(|| from_sequence.unwrap_or(0));
        let (required_sender, diagnostic_sender, live_events) =
            RuntimeEventReceiver::bounded_lanes(self.broadcast_capacity);

        if closure.borrow().is_some() {
            // Close may have appended a terminal after the first replay snapshot.
            // The closure is published while holding `ring`, so this second read
            // includes every event through its final sequence.
            let replay =
                run.events_after_sequence(from_sequence.unwrap_or(0), usize::MAX, &subscriber)?;
            drop(required_sender);
            drop(diagnostic_sender);
            return Ok(RuntimeEventSubscription {
                terminal_writer,
                replay,
                live_events,
                closure,
            });
        }

        let live_run = Arc::clone(&run);
        let mut closed_receiver = closure.clone();
        if closed_receiver.borrow().is_some() {
            let replay =
                run.events_after_sequence(from_sequence.unwrap_or(0), usize::MAX, &subscriber)?;
            drop(required_sender);
            drop(diagnostic_sender);
            return Ok(RuntimeEventSubscription {
                terminal_writer,
                replay,
                live_events,
                closure,
            });
        }

        subscriber
            .sequence
            .store(last_sent_sequence, Ordering::Release);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = required_sender.closed() => break,
                    changed = closed_receiver.changed() => {
                        if changed.is_err() || closed_receiver.borrow().is_some() {
                            let _ = send_retained_after_sequence(
                                &live_run,
                                &required_sender,
                                &diagnostic_sender,
                                &mut last_sent_sequence,
                                &subscriber,
                            ).await;
                            break;
                        }
                    }
                    received = live_receiver.recv() => {
                        match received {
                            Ok(event) if event.sequence <= last_sent_sequence => {}
                            Ok(event) => {
                                // Broadcast delivery is already typed. Decode the replay ring
                                // only when a gap requires backfill (including reordered sends).
                                if event.sequence > last_sent_sequence.saturating_add(1)
                                    && !send_retained_after_sequence(
                                        &live_run,
                                        &required_sender,
                                        &diagnostic_sender,
                                        &mut last_sent_sequence,
                                        &subscriber,
                                    ).await
                                {
                                    break;
                                }
                                if event.sequence <= last_sent_sequence {
                                    continue;
                                }
                                let sequence = event.sequence;
                                if is_required_delivery_event(&event) {
                                    if required_sender.send(event).await.is_err() {
                                        break;
                                    }
                                } else {
                                    let _ = diagnostic_sender.try_send(event);
                                }
                                last_sent_sequence = sequence;
                                subscriber.sequence.store(sequence, Ordering::Release);
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => {
                                if !send_retained_after_sequence(
                                    &live_run,
                                    &required_sender,
                                    &diagnostic_sender,
                                    &mut last_sent_sequence,
                                    &subscriber,
                                ).await {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                let _ = send_retained_after_sequence(
                                    &live_run,
                                    &required_sender,
                                    &diagnostic_sender,
                                    &mut last_sent_sequence,
                                    &subscriber,
                                ).await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(RuntimeEventSubscription {
            terminal_writer,
            replay,
            live_events,
            closure,
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
        if !run.is_closed() {
            let final_sequence = run.next_sequence.load(Ordering::SeqCst) - 1;
            *run.closed_at
                .lock()
                .expect("runtime event closed_at lock poisoned") = Some(OffsetDateTime::now_utc());
            run.closed_sender.send_replace(Some(RuntimeEventClosure {
                reason,
                final_sequence,
            }));
            run.release_live_broadcast();
            self.schedule_updates.send_replace(());
        }
        Ok(())
    }

    async fn trim(&self, run_id: Uuid, policy: RuntimeEventTrimPolicy) -> Result<()> {
        let run = self.run(run_id)?;
        if let Some(before_sequence) = policy.before_sequence {
            let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
            if ring.retired {
                return Err(anyhow!("runtime event stream is not open"));
            }
            ring.events.retain(|event| {
                event.sequence >= before_sequence || (policy.keep_required && event.is_required())
            });
            ring.bytes = ring.events.iter().map(|event| event.wire.len()).sum();
            ring.terminal_reason = ring.events.iter().find_map(|event| event.terminal_reason);
            run.trim_to_policy(&mut ring)?;
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
            let retained = run
                .ring
                .lock()
                .expect("runtime event ring lock poisoned")
                .events
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            for event in retained {
                entries.push(Self::event_snapshot(&event.decode()?, &run, now));
            }
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
        let event = run
            .ring
            .lock()
            .expect("runtime event ring lock poisoned")
            .events
            .iter()
            .find(|event| event.sequence == sequence)
            .cloned();
        let Some(event) = event else {
            return Ok(None);
        };
        let event = event.decode()?;
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            Self::event_snapshot(&event, &run, OffsetDateTime::now_utc()),
            event.payload,
            reveal_mode,
        )))
    }
}
