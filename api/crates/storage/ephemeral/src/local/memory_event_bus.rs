use std::{
    collections::{HashMap, VecDeque},
    io::{self, Write},
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    EphemeralEntrySnapshot, EphemeralEntryValueSnapshot, EphemeralInspectionCapabilities,
    EphemeralValueRevealMode, EventBus, EventBusError, EventBusLimits, EventBusSnapshot,
    EventConsumer, EventDelivery, EventDeliveryLease, EventPermit, ephemeral_metadata_size_bytes,
};
use tokio::sync::Notify;

/// A single shared logical JSON budget. No production capacity is guessed by new/default.
#[derive(Clone, Default)]
pub struct MemoryEventBus {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    changed: Notify,
}

#[derive(Default)]
struct State {
    limits: Option<EventBusLimits>,
    topics: HashMap<u64, Topic>,
    entries: HashMap<u64, Entry>,
    next_id: u64,
    charged_json_bytes: u64,
    consumers: usize,
    waiters: usize,
    topic_name_bytes: usize,
    abnormal_terminations: u64,
}

struct Topic {
    name: Arc<str>,
    consumers: usize,
    closed: bool,
    buffered: VecDeque<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Reserved,
    Buffered,
    InFlight,
}
struct Entry {
    generation: u64,
    bytes: u64,
    body: Option<Arc<serde_json::Value>>,
    phase: Phase,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        // No user callbacks or awaits run under this mutex. Recover for Drop accounting.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn wake(&self) {
        self.changed.notify_waiters();
    }
}

impl State {
    fn next_id(&mut self) -> Result<u64, EventBusError> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(EventBusError::ResourceExhausted(
                "generation/sequence identifiers",
            ))?;
        Ok(self.next_id)
    }
    fn generation(&self, topic: &str) -> Result<u64, EventBusError> {
        self.topics
            .iter()
            .find(|(_, t)| t.consumers > 0 && t.name.as_ref() == topic)
            .map(|(id, t)| {
                if t.closed {
                    Err(EventBusError::Closed)
                } else {
                    Ok(*id)
                }
            })
            .unwrap_or(Err(EventBusError::NoConsumer))
    }
    fn remove_entry(&mut self, id: u64) {
        if let Some(entry) = self.entries.remove(&id) {
            self.charged_json_bytes -= entry.bytes;
            self.cleanup_topic(entry.generation);
        }
        if self.entries.is_empty() {
            self.entries = HashMap::new();
        }
    }
    fn cleanup_topic(&mut self, generation: u64) {
        if self
            .topics
            .get(&generation)
            .is_some_and(|t| t.consumers == 0)
            && !self.entries.values().any(|e| e.generation == generation)
        {
            if let Some(t) = self.topics.remove(&generation) {
                self.topic_name_bytes -= t.name.len();
            }
        }
        if self.topics.is_empty() {
            self.topics = HashMap::new();
        }
    }
    fn close(&mut self, generation: u64) {
        if let Some(t) = self.topics.get_mut(&generation) {
            t.closed = true;
        }
        // An issued permit may be constructing its body. Invalidate admission, but
        // keep its charge and retired directory alive until send fails or it drops.
    }
    fn snapshot(&self) -> EventBusSnapshot {
        EventBusSnapshot {
            limits: self.limits,
            topics: self.topics.len(),
            closed_topics: self.topics.values().filter(|t| t.closed).count(),
            retired_topics: self.topics.values().filter(|t| t.consumers == 0).count(),
            topic_name_bytes: self.topic_name_bytes,
            consumers: self.consumers,
            waiters: self.waiters,
            reserved: self
                .entries
                .values()
                .filter(|e| e.phase == Phase::Reserved)
                .count(),
            buffered: self
                .entries
                .values()
                .filter(|e| e.phase == Phase::Buffered)
                .count(),
            in_flight: self
                .entries
                .values()
                .filter(|e| e.phase == Phase::InFlight)
                .count(),
            charged_json_bytes: self.charged_json_bytes,
            abnormal_terminations: self.abnormal_terminations,
        }
    }
}

struct WaitTicket {
    inner: Arc<Inner>,
}
impl WaitTicket {
    fn acquire(inner: &Arc<Inner>, state: &mut State) -> Result<Self, EventBusError> {
        let limits = state.limits.ok_or(EventBusError::Unconfigured)?;
        if state.waiters >= limits.max_waiters {
            return Err(EventBusError::ResourceExhausted("waiters"));
        }
        state.waiters += 1;
        Ok(Self {
            inner: inner.clone(),
        })
    }
}
impl Drop for WaitTicket {
    fn drop(&mut self) {
        self.inner.lock().waiters -= 1;
        self.inner.wake();
    }
}

struct Permit {
    inner: Arc<Inner>,
    id: u64,
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.inner.lock().remove_entry(self.id);
        self.inner.wake();
    }
}

/// Count serde's logical JSON encoding without allocating a second serialized body.
struct JsonCounter {
    bytes: u64,
    limit: u64,
}
impl Write for JsonCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buf.len() as u64)
            .ok_or_else(|| io::Error::other("JSON size overflow"))?;
        if self.bytes > self.limit {
            return Err(io::Error::other("JSON exceeds reservation"));
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl EventPermit for Permit {
    fn send(self: Box<Self>, body: serde_json::Value) -> Result<(), EventBusError> {
        let upper = {
            let state = self.inner.lock();
            let entry = state.entries.get(&self.id).ok_or(EventBusError::Closed)?;
            if state
                .topics
                .get(&entry.generation)
                .is_none_or(|t| t.closed || t.consumers == 0)
            {
                return Err(EventBusError::Closed);
            }
            entry.bytes
        };
        let mut counter = JsonCounter {
            bytes: 0,
            limit: upper,
        };
        if serde_json::to_writer(&mut counter, &body).is_err() {
            return Err(if counter.bytes > upper {
                EventBusError::BodyExceedsReservation
            } else {
                EventBusError::InvalidBody
            });
        }
        let mut state = self.inner.lock();
        let generation = state
            .entries
            .get(&self.id)
            .ok_or(EventBusError::Closed)?
            .generation;
        if state
            .topics
            .get(&generation)
            .is_none_or(|t| t.closed || t.consumers == 0)
        {
            return Err(EventBusError::Closed);
        }
        let entry = state
            .entries
            .get_mut(&self.id)
            .ok_or(EventBusError::Closed)?;
        entry.body = Some(Arc::new(body));
        entry.phase = Phase::Buffered;
        state
            .topics
            .get_mut(&generation)
            .ok_or(EventBusError::Closed)?
            .buffered
            .push_back(self.id);
        // The queue now owns the charge. Release the lock before disarming this
        // permit; no suspension or user callback occurs between these operations.
        drop(state);
        let mut owned = self;
        owned.id = 0;
        owned.inner.wake();
        Ok(())
    }
}

struct Consumer {
    inner: Arc<Inner>,
    generation: u64,
}
#[async_trait]
impl EventConsumer for Consumer {
    async fn recv(&self) -> Result<Option<EventDelivery>, EventBusError> {
        let mut ticket = None;
        loop {
            let notified = self.inner.changed.notified();
            tokio::pin!(notified);
            {
                let mut state = self.inner.lock();
                let topic = state
                    .topics
                    .get_mut(&self.generation)
                    .ok_or(EventBusError::Closed)?;
                if let Some(id) = topic.buffered.pop_front() {
                    if topic.buffered.is_empty() {
                        topic.buffered = VecDeque::new();
                    }
                    let entry = state.entries.get_mut(&id).ok_or(EventBusError::Closed)?;
                    entry.phase = Phase::InFlight;
                    let body = entry
                        .body
                        .as_ref()
                        .ok_or(EventBusError::InvalidBody)?
                        .clone();
                    return Ok(Some(EventDelivery::new(
                        body,
                        Box::new(DeliveryLease {
                            inner: self.inner.clone(),
                            id,
                        }),
                    )));
                }
                if topic.closed {
                    return Ok(None);
                }
                if ticket.is_none() {
                    ticket = Some(WaitTicket::acquire(&self.inner, &mut state)?);
                }
                // Register with Notify only after the bounded ticket is acquired,
                // under the state lock so a sender/close cannot race past registration.
                notified.as_mut().enable();
            }
            notified.await;
        }
    }
    fn close(&self) {
        self.inner.lock().close(self.generation);
        self.inner.wake();
    }
}
impl Drop for Consumer {
    fn drop(&mut self) {
        let mut state = self.inner.lock();
        let Some(topic) = state.topics.get_mut(&self.generation) else {
            return;
        };
        topic.consumers -= 1;
        let last = topic.consumers == 0;
        let abnormal = last && (!topic.closed || !topic.buffered.is_empty());
        state.consumers -= 1;
        if last {
            if abnormal {
                state.abnormal_terminations = state.abnormal_terminations.saturating_add(1);
            }
            state.close(self.generation);
            let ids: Vec<_> = state
                .entries
                .iter()
                .filter(|(_, e)| e.generation == self.generation && e.phase == Phase::Buffered)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                state.remove_entry(id);
            }
            if let Some(topic) = state.topics.get_mut(&self.generation) {
                topic.buffered = VecDeque::new();
            }
            state.cleanup_topic(self.generation);
        }
        drop(state);
        self.inner.wake();
    }
}
struct DeliveryLease {
    inner: Arc<Inner>,
    id: u64,
}
impl EventDeliveryLease for DeliveryLease {}
impl Drop for DeliveryLease {
    fn drop(&mut self) {
        self.inner.lock().remove_entry(self.id);
        self.inner.wake();
    }
}

impl MemoryEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn resource_snapshot(state: &State) -> EphemeralEntrySnapshot {
        let metadata = serde_json::to_value(state.snapshot())
            .expect("event resource snapshot contains only JSON-serializable counters");
        EphemeralEntrySnapshot {
            contract_code: "event-bus".into(),
            group_code: None,
            entry_ref: "resources".into(),
            key: "event-bus:resources".into(),
            inspection_path: vec!["resources".into()],
            entry_kind: "event_bus_resources".into(),
            status: if state.consumers == 0 {
                "idle"
            } else {
                "active"
            }
            .into(),
            owner: None,
            value_size_bytes: 0,
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: None,
            created_at_unix: None,
            expires_at_unix: None,
            sensitive: false,
            metadata,
        }
    }

    fn entry_snapshot(state: &State, id: u64, entry: &Entry) -> EphemeralEntrySnapshot {
        let topic = &state.topics[&entry.generation];
        let entry_ref = id.to_string();
        let status = match entry.phase {
            Phase::Reserved => "reserved",
            Phase::Buffered => "buffered",
            Phase::InFlight => "in-flight",
        };
        let metadata = serde_json::json!({"topic": topic.name.as_ref(), "sequence": id, "generation": entry.generation,
            "closed": topic.closed, "consumers": topic.consumers, "reserved_json_bytes": entry.bytes});
        let mut counter = JsonCounter {
            bytes: 0,
            limit: u64::MAX,
        };
        if let Some(body) = &entry.body {
            let _ = serde_json::to_writer(&mut counter, body.as_ref());
        }
        EphemeralEntrySnapshot {
            contract_code: "event-bus".into(),
            group_code: Some(topic.name.to_string()),
            entry_ref: entry_ref.clone(),
            key: format!("{}:{id}", topic.name),
            inspection_path: vec![topic.name.to_string(), entry_ref],
            entry_kind: "event".into(),
            status: status.into(),
            owner: None,
            value_size_bytes: counter.bytes,
            metadata_size_bytes: ephemeral_metadata_size_bytes(&metadata),
            ttl_seconds: None,
            created_at_unix: None,
            expires_at_unix: None,
            sensitive: true,
            metadata,
        }
    }
}

#[async_trait]
impl EventBus for MemoryEventBus {
    fn configure_budget(&self, limits: EventBusLimits) -> Result<(), EventBusError> {
        let mut state = self.inner.lock();
        match state.limits {
            Some(existing) if existing != limits => Err(EventBusError::BudgetAlreadyConfigured),
            _ => {
                state.limits = Some(limits);
                Ok(())
            }
        }
    }
    fn open_consumer(&self, name: &str) -> Result<Box<dyn EventConsumer>, EventBusError> {
        let mut state = self.inner.lock();
        let limits = state.limits.ok_or(EventBusError::Unconfigured)?;
        if state.consumers >= limits.max_consumers {
            return Err(EventBusError::ResourceExhausted("consumers"));
        }
        let existing = state
            .topics
            .iter()
            .find(|(_, t)| t.consumers > 0 && t.name.as_ref() == name)
            .map(|(id, t)| (*id, t.closed));
        let generation = match existing {
            Some((_, true)) => return Err(EventBusError::Closed),
            Some((id, false)) => {
                state
                    .topics
                    .get_mut(&id)
                    .ok_or(EventBusError::Closed)?
                    .consumers += 1;
                id
            }
            None => {
                if state.topics.len() >= limits.max_topics {
                    return Err(EventBusError::ResourceExhausted("topics"));
                }
                if name.len()
                    > limits
                        .max_topic_name_bytes
                        .saturating_sub(state.topic_name_bytes)
                {
                    return Err(EventBusError::ResourceExhausted("topic name bytes"));
                }
                let id = state.next_id()?;
                state.topics.insert(
                    id,
                    Topic {
                        name: Arc::from(name),
                        consumers: 1,
                        closed: false,
                        buffered: VecDeque::new(),
                    },
                );
                state.topic_name_bytes += name.len();
                id
            }
        };
        state.consumers += 1;
        Ok(Box::new(Consumer {
            inner: self.inner.clone(),
            generation,
        }))
    }
    fn event_bus_snapshot(&self) -> EventBusSnapshot {
        self.inner.lock().snapshot()
    }

    async fn reserve(
        &self,
        topic: &str,
        upper: u64,
        deadline: Instant,
    ) -> Result<Box<dyn EventPermit>, EventBusError> {
        let mut ticket = None;
        let mut generation = None;
        loop {
            let notified = self.inner.changed.notified();
            tokio::pin!(notified);
            {
                let mut state = self.inner.lock();
                let limits = state.limits.ok_or(EventBusError::Unconfigured)?;
                if upper > limits.max_json_bytes || limits.max_items == 0 {
                    return Err(EventBusError::ResourceExhausted(
                        "event exceeds total budget",
                    ));
                }
                let id = match generation {
                    Some(id) => {
                        if state
                            .topics
                            .get(&id)
                            .is_none_or(|t| t.closed || t.consumers == 0)
                        {
                            return Err(EventBusError::Closed);
                        }
                        id
                    }
                    None => {
                        let id = state.generation(topic)?;
                        generation = Some(id);
                        id
                    }
                };
                if Instant::now() >= deadline {
                    return Err(EventBusError::DeadlineElapsed);
                }
                if state.entries.len() < limits.max_items
                    && upper <= limits.max_json_bytes - state.charged_json_bytes
                {
                    let entry_id = state.next_id()?;
                    state.entries.insert(
                        entry_id,
                        Entry {
                            generation: id,
                            bytes: upper,
                            body: None,
                            phase: Phase::Reserved,
                        },
                    );
                    state.charged_json_bytes += upper;
                    return Ok(Box::new(Permit {
                        inner: self.inner.clone(),
                        id: entry_id,
                    }));
                }
                if ticket.is_none() {
                    ticket = Some(WaitTicket::acquire(&self.inner, &mut state)?);
                }
                // Register with Notify only after the bounded ticket is acquired,
                // under the state lock so a sender/close cannot race past registration.
                notified.as_mut().enable();
            }
            tokio::select! {
                _ = notified => {},
                _ = tokio::time::sleep_until(deadline.into()) => return Err(EventBusError::DeadlineElapsed),
            }
        }
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        let mut capabilities = EphemeralInspectionCapabilities::supported();
        capabilities.max_payload_size_bytes =
            self.inner.lock().limits.map_or(0, |l| l.max_json_bytes);
        capabilities
    }
    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        let state = self.inner.lock();
        let mut entries: Vec<_> = state
            .entries
            .iter()
            .map(|(id, entry)| (*id, Self::entry_snapshot(&state, *id, entry)))
            .collect();
        entries.sort_by_key(|(id, _)| *id);
        let mut entries: Vec<_> = entries.into_iter().map(|(_, entry)| entry).collect();
        if state.limits.is_some() {
            entries.push(Self::resource_snapshot(&state));
        }
        Ok(entries)
    }
    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        let Ok(id) = entry_ref.parse::<u64>() else {
            return Ok(None);
        };
        let state = self.inner.lock();
        let Some(entry) = state.entries.get(&id) else {
            return Ok(None);
        };
        let Some(body) = &entry.body else {
            return Ok(None);
        };
        Ok(Some(EphemeralEntryValueSnapshot::from_value(
            Self::entry_snapshot(&state, id, entry),
            body.as_ref().clone(),
            reveal_mode,
        )))
    }
}

#[cfg(test)]
#[path = "../_tests/memory_event_bus_tests.rs"]
mod tests;
