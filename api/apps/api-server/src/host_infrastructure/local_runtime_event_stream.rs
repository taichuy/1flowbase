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
const DEFAULT_RECOVERABLE_RETENTION: TimeDuration = TimeDuration::minutes(5);

#[derive(Clone)]
pub struct LocalRuntimeEventStream {
    runs: Arc<Mutex<HashMap<Uuid, Arc<LocalRunEventStream>>>>,
    broadcast_capacity: usize,
    schedule_updates: watch::Sender<()>,
    scheduler_started: Arc<AtomicBool>,
    recoverable_retention: TimeDuration,
    recoverable_byte_budget: Option<usize>,
}

struct LocalRuntimeEventTerminalWriter {
    run_id: Uuid,
    run: Arc<LocalRunEventStream>,
    schedule_updates: watch::Sender<()>,
}

#[async_trait::async_trait]
impl RuntimeEventTerminalWriter for LocalRuntimeEventTerminalWriter {
    fn claim_persistence_owner(&self) -> bool {
        let mut ring = self
            .run
            .ring
            .lock()
            .expect("runtime event ring lock poisoned");
        if ring.persistence_owner_claimed {
            return false;
        }
        ring.persistence_owner_claimed = true;
        true
    }

    fn generation_id(&self) -> Option<Uuid> {
        Some(self.run.generation_id)
    }

    fn durable_replay_boundary(&self) -> Option<i64> {
        self.run
            .ring
            .lock()
            .expect("runtime event ring lock poisoned")
            .durable_replay_boundary
    }

    fn set_durable_replay_boundary(&self, sequence: i64) -> Result<()> {
        if sequence < 0 {
            return Err(anyhow!("durable replay boundary must be nonnegative"));
        }
        let mut ring = self
            .run
            .ring
            .lock()
            .expect("runtime event ring lock poisoned");
        match ring.durable_replay_boundary {
            Some(boundary) if boundary != sequence => {
                Err(anyhow!("durable replay boundary is already bound"))
            }
            _ => {
                ring.durable_replay_boundary = Some(sequence);
                Ok(())
            }
        }
    }

    fn record_persistence_failure(&self) {
        let mut ring = self
            .run
            .ring
            .lock()
            .expect("runtime event ring lock poisoned");
        ring.persistence_failed = true;
        ring.confirmed_at = None;
        drop(ring);
        self.schedule_updates.send_replace(());
    }

    fn closure(&self) -> Option<RuntimeEventClosure> {
        *self.run.closed_sender.borrow()
    }

    async fn confirm_terminal_persisted(&self, final_sequence: i64) -> Result<()> {
        let mut ring = self
            .run
            .ring
            .lock()
            .expect("runtime event ring lock poisoned");
        let closure = self
            .closure()
            .ok_or_else(|| anyhow!("runtime event generation is not closed"))?;
        if ring.persistence_failed {
            return Err(anyhow!(
                "runtime event generation has a persistence failure"
            ));
        }
        if ring.durable_replay_boundary.is_none() {
            return Err(anyhow!(
                "runtime event generation has no durable replay boundary"
            ));
        }
        if closure.final_sequence != final_sequence
            || ring.terminal_sequence != Some(final_sequence)
            || ring.terminal_reason != Some(closure.reason)
        {
            return Err(anyhow!(
                "persisted terminal does not match the closed generation"
            ));
        }
        ring.confirmed_at
            .get_or_insert_with(OffsetDateTime::now_utc);
        drop(ring);
        self.schedule_updates.send_replace(());
        Ok(())
    }

    async fn close_generation(&self, reason: RuntimeEventCloseReason) -> Result<()> {
        // The Arc is captured when the writer is issued. Reopening the same run
        // replaces the map entry, but never redirects this authority.
        self.run.close(reason);
        self.schedule_updates.send_replace(());
        Ok(())
    }

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
    generation_id: Uuid,
    recoverable_retention: TimeDuration,
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
    ephemeral_count: usize,
    sequence_order_may_be_unsorted: bool,
    terminal_reason: Option<RuntimeEventCloseReason>,
    terminal_sequence: Option<i64>,
    durable_replay_boundary: Option<i64>,
    persistence_failed: bool,
    persistence_owner_claimed: bool,
    confirmed_at: Option<OffsetDateTime>,
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
    required_delivery: bool,
    terminal_reason: Option<RuntimeEventCloseReason>,
    wire: Box<[u8]>,
}

impl RetainedRuntimeEvent {
    fn encode(event: &RuntimeEventEnvelope) -> Result<Self> {
        Ok(Self {
            sequence: event.sequence,
            required_delivery: is_required_delivery_event(event),
            terminal_reason: RuntimeEventCloseReason::from_terminal_event_type(&event.event_type),
            wire: serde_json::to_vec(event)?.into_boxed_slice(),
        })
    }

    fn decode(&self) -> Result<RuntimeEventEnvelope> {
        Ok(serde_json::from_slice(&self.wire)?)
    }

    fn is_required(&self) -> bool {
        self.required_delivery
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
            recoverable_retention: DEFAULT_RECOVERABLE_RETENTION,
            recoverable_byte_budget: None,
        }
    }
}

impl LocalRuntimeEventStream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure before opening runs. This controls only recoverable closed hot data.
    pub fn with_recoverable_retention(mut self, retention: std::time::Duration) -> Self {
        self.recoverable_retention = TimeDuration::try_from(retention).unwrap_or(TimeDuration::MAX);
        self
    }

    /// Optional budget for proof-confirmed closed hot rings, never producer admission.
    pub fn with_recoverable_byte_budget(mut self, bytes: usize) -> Self {
        self.recoverable_byte_budget = Some(bytes);
        self
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
        let byte_budget = self.recoverable_byte_budget;
        tokio::spawn(async move { Self::expire_idle_runs(runs, updates, byte_budget).await });
    }

    async fn expire_idle_runs(
        runs: Weak<Mutex<HashMap<Uuid, Arc<LocalRunEventStream>>>>,
        mut updates: watch::Receiver<()>,
        byte_budget: Option<usize>,
    ) {
        loop {
            // Register the revision before inspecting deadlines, so a mutation during
            // the scan cannot be lost before the timer or wait is armed.
            updates.borrow_and_update();
            let Some(runs) = runs.upgrade() else { break };
            let next_deadline = Self::purge_and_next_deadline(&runs, byte_budget);
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
        byte_budget: Option<usize>,
    ) -> Option<OffsetDateTime> {
        let now = OffsetDateTime::now_utc();
        let mut runs = runs
            .lock()
            .expect("runtime event stream runs lock poisoned");
        runs.retain(|_, run| !run.retire_if_expired(now));
        if let Some(budget) = byte_budget {
            let mut recoverable = runs
                .iter()
                .filter_map(|(run_id, run)| {
                    let ring = run.ring.lock().expect("runtime event ring lock poisoned");
                    ring.confirmed_at
                        .filter(|_| !ring.persistence_failed && run.is_closed())
                        .map(|confirmed_at| (*run_id, confirmed_at, ring.bytes))
                })
                .collect::<Vec<_>>();
            recoverable.sort_by_key(|(_, confirmed_at, _)| *confirmed_at);
            let mut retained_bytes = recoverable
                .iter()
                .fold(0usize, |total, (_, _, bytes)| total.saturating_add(*bytes));
            for (run_id, _, bytes) in recoverable {
                if retained_bytes <= budget {
                    break;
                }
                // A failure could have raced with the selection. Recheck while holding
                // the ring serialization lock before claiming payload retirement.
                if let Some(run) = runs.get(&run_id) {
                    if run.retire_recoverable_payloads() {
                        runs.remove(&run_id);
                        retained_bytes = retained_bytes.saturating_sub(bytes);
                    }
                }
            }
        }
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
        if run.retire_if_expired(OffsetDateTime::now_utc()) {
            runs.remove(&run_id);
            return Err(anyhow!("runtime event stream is not open"));
        }
        Ok(Arc::clone(run))
    }

    fn purge_expired_runs(&self) {
        Self::purge_and_next_deadline(&self.runs, self.recoverable_byte_budget);
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

    #[cfg(test)]
    pub(crate) fn set_confirmation_timestamp_for_tests(
        &self,
        run_id: Uuid,
        confirmed_at: OffsetDateTime,
    ) -> Result<()> {
        let run = self.run(run_id)?;
        let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
        if ring.confirmed_at.is_none() {
            return Err(anyhow!("generation is not confirmed"));
        }
        ring.confirmed_at = Some(confirmed_at);
        drop(ring);
        self.schedule_updates.send_replace(());
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn retention_deadline_for_tests(&self, run_id: Uuid) -> Result<OffsetDateTime> {
        Ok(self.run(run_id)?.retention_deadline())
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

fn is_required_durability(durability: RuntimeEventDurability) -> bool {
    matches!(
        durability,
        RuntimeEventDurability::DurableRequired | RuntimeEventDurability::AuditRequired
    )
}

fn is_required_delivery_event(event: &RuntimeEventEnvelope) -> bool {
    requires_delivery(&event.event_type, event.durability)
}

fn requires_delivery(event_type: &str, durability: RuntimeEventDurability) -> bool {
    is_required_durability(durability)
        || matches!(
            event_type,
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
    async fn terminal_writer(&self, run_id: Uuid) -> Result<Arc<dyn RuntimeEventTerminalWriter>> {
        Ok(Arc::new(LocalRuntimeEventTerminalWriter {
            run_id,
            run: self.run(run_id)?,
            schedule_updates: self.schedule_updates.clone(),
        }))
    }

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
                    Arc::new(LocalRunEventStream::new(
                        policy,
                        self.broadcast_capacity,
                        self.recoverable_retention,
                    )),
                );
            }
            Some(_) => {}
            None => {
                runs.insert(
                    run_id,
                    Arc::new(LocalRunEventStream::new(
                        policy,
                        self.broadcast_capacity,
                        self.recoverable_retention,
                    )),
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
        if !requires_delivery(&event.event_type, event.durability) {
            ensure_ephemeral_payload_size(&event.payload)?;
        }
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
            run.make_room_for(&mut ring, retained_bytes, retained.is_required())?;
            run.next_sequence.store(sequence + 1, Ordering::SeqCst);
            *run.last_event_at
                .lock()
                .expect("runtime event last event lock poisoned") = envelope.occurred_at;
            ring.push(retained);
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
        run.close(reason);
        self.schedule_updates.send_replace(());
        Ok(())
    }

    async fn trim(&self, run_id: Uuid, policy: RuntimeEventTrimPolicy) -> Result<()> {
        let run = self.run(run_id)?;
        if let Some(before_sequence) = policy.before_sequence {
            let mut ring = run.ring.lock().expect("runtime event ring lock poisoned");
            if ring.retired {
                return Err(anyhow!("runtime event stream is not open"));
            }
            let mut ephemeral_count = 0;
            ring.events.retain(|event| {
                let required = event.is_required();
                let keep = event.sequence >= before_sequence || (policy.keep_required && required);
                ephemeral_count += usize::from(keep && !required);
                keep
            });
            ring.ephemeral_count = ephemeral_count;
            ring.bytes = ring.events.iter().map(|event| event.wire.len()).sum();
            ring.terminal_reason = ring.events.iter().find_map(|event| event.terminal_reason);
            ring.terminal_sequence = ring
                .events
                .iter()
                .find(|event| event.terminal_reason.is_some())
                .map(|event| event.sequence);
            run.trim_to_policy(&mut ring)?;
        }
        Ok(())
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        let mut capabilities = EphemeralInspectionCapabilities::supported();
        capabilities.max_payload_size_bytes = u64::MAX;
        capabilities
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

#[path = "local_runtime_event_stream/run_event_retention.rs"]
mod run_event_retention;
