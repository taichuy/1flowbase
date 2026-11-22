use std::{
    cmp::{Ordering as CmpOrdering, Reverse},
    collections::{BinaryHeap, HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    ProviderContinuation, ProviderContinuationSlotId, ProviderProtocolContextSlotId,
    ProviderProtocolContextValue, ProviderTransportPayload, ProviderTransportSlotId,
    ProviderTransportStore,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::{watch, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct MemoryProviderTransportStore {
    retention: Duration,
    state: Arc<RwLock<TransportStoreState>>,
    maintenance_started: Arc<AtomicBool>,
    maintenance_revision: watch::Sender<u64>,
}

#[derive(Default)]
struct TransportStoreState {
    next_generation: u64,
    entries: HashMap<ProviderTransportSlotId, TransportEntry>,
    protocol_contexts: HashMap<ProviderProtocolContextSlotId, ProtocolContextEntry>,
    continuations: HashMap<ProviderContinuationSlotId, ContinuationEntry>,
    flow_protocol_contexts: HashMap<Uuid, HashSet<ProviderProtocolContextSlotId>>,
    flow_continuations: HashMap<Uuid, HashSet<ProviderContinuationSlotId>>,
    expiry_heap: BinaryHeap<Reverse<ScheduledExpiry>>,
}

#[derive(Clone)]
struct TransportEntry {
    payload: ProviderTransportPayload,
    expires_at: OffsetDateTime,
    generation: u64,
}

#[derive(Clone)]
struct ContinuationEntry {
    continuation: ProviderContinuation,
    expires_at: OffsetDateTime,
    generation: u64,
}

#[derive(Clone)]
struct ProtocolContextEntry {
    value: ProviderProtocolContextValue,
    expires_at: OffsetDateTime,
    generation: u64,
}

#[derive(Clone, Copy)]
enum ExpiryEntry {
    Transport(ProviderTransportSlotId),
    ProtocolContext(ProviderProtocolContextSlotId),
    Continuation(ProviderContinuationSlotId),
}

#[derive(Clone, Copy)]
struct ScheduledExpiry {
    expires_at: OffsetDateTime,
    generation: u64,
    entry: ExpiryEntry,
}

impl PartialEq for ScheduledExpiry {
    fn eq(&self, other: &Self) -> bool {
        self.expires_at == other.expires_at && self.generation == other.generation
    }
}

impl Eq for ScheduledExpiry {}

impl PartialOrd for ScheduledExpiry {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledExpiry {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.expires_at
            .cmp(&other.expires_at)
            .then_with(|| self.generation.cmp(&other.generation))
    }
}

impl TransportStoreState {
    fn alloc_generation(&mut self) -> u64 {
        self.next_generation = self.next_generation.wrapping_add(1);
        self.next_generation
    }

    fn live_count(&self) -> usize {
        self.entries.len() + self.protocol_contexts.len() + self.continuations.len()
    }

    fn is_live(&self, scheduled: &ScheduledExpiry) -> bool {
        match scheduled.entry {
            ExpiryEntry::Transport(slot_id) => self
                .entries
                .get(&slot_id)
                .is_some_and(|entry| entry.generation == scheduled.generation),
            ExpiryEntry::ProtocolContext(slot_id) => self
                .protocol_contexts
                .get(&slot_id)
                .is_some_and(|entry| entry.generation == scheduled.generation),
            ExpiryEntry::Continuation(slot_id) => self
                .continuations
                .get(&slot_id)
                .is_some_and(|entry| entry.generation == scheduled.generation),
        }
    }

    fn pop_stale_heap_heads(&mut self) {
        while let Some(&Reverse(ref top)) = self.expiry_heap.peek() {
            if self.is_live(top) {
                break;
            }
            self.expiry_heap.pop();
        }
    }

    fn maybe_compact_expiry_heap(&mut self) {
        let live = self.live_count();
        if self.expiry_heap.len() <= live.saturating_mul(2).saturating_add(64) {
            return;
        }
        let mut rebuilt = BinaryHeap::with_capacity(live);
        for (&slot_id, entry) in &self.entries {
            rebuilt.push(Reverse(ScheduledExpiry {
                expires_at: entry.expires_at,
                generation: entry.generation,
                entry: ExpiryEntry::Transport(slot_id),
            }));
        }
        for (&slot_id, entry) in &self.protocol_contexts {
            rebuilt.push(Reverse(ScheduledExpiry {
                expires_at: entry.expires_at,
                generation: entry.generation,
                entry: ExpiryEntry::ProtocolContext(slot_id),
            }));
        }
        for (&slot_id, entry) in &self.continuations {
            rebuilt.push(Reverse(ScheduledExpiry {
                expires_at: entry.expires_at,
                generation: entry.generation,
                entry: ExpiryEntry::Continuation(slot_id),
            }));
        }
        self.expiry_heap = rebuilt;
    }

    fn schedule_expiry(
        &mut self,
        expires_at: OffsetDateTime,
        generation: u64,
        entry: ExpiryEntry,
    ) -> bool {
        self.pop_stale_heap_heads();
        self.expiry_heap.push(Reverse(ScheduledExpiry {
            expires_at,
            generation,
            entry,
        }));
        self.maybe_compact_expiry_heap();
        self.expiry_heap
            .peek()
            .is_some_and(|Reverse(top)| top.generation == generation)
    }

    fn next_live_expiry(&mut self) -> Option<OffsetDateTime> {
        self.pop_stale_heap_heads();
        self.expiry_heap.peek().map(|Reverse(top)| top.expires_at)
    }

    fn untrack_protocol_context(&mut self, slot_id: ProviderProtocolContextSlotId) {
        let flow_run_id = slot_id.flow_run_id();
        if let Some(slots) = self.flow_protocol_contexts.get_mut(&flow_run_id) {
            slots.remove(&slot_id);
            if slots.is_empty() {
                self.flow_protocol_contexts.remove(&flow_run_id);
            }
        }
    }

    fn remove_protocol_context(
        &mut self,
        slot_id: ProviderProtocolContextSlotId,
    ) -> Option<ProtocolContextEntry> {
        let removed = self.protocol_contexts.remove(&slot_id);
        self.untrack_protocol_context(slot_id);
        removed
    }

    fn untrack_continuation(&mut self, slot_id: ProviderContinuationSlotId) {
        let flow_run_id = slot_id.flow_run_id();
        if let Some(slots) = self.flow_continuations.get_mut(&flow_run_id) {
            slots.remove(&slot_id);
            if slots.is_empty() {
                self.flow_continuations.remove(&flow_run_id);
            }
        }
    }

    fn remove_continuation(
        &mut self,
        slot_id: ProviderContinuationSlotId,
    ) -> Option<ContinuationEntry> {
        let removed = self.continuations.remove(&slot_id);
        self.untrack_continuation(slot_id);
        removed
    }

    fn evict_expired(&mut self, now: OffsetDateTime) -> usize {
        let mut removed = 0;
        while let Some(&Reverse(top)) = self.expiry_heap.peek() {
            if !self.is_live(&top) {
                self.expiry_heap.pop();
                continue;
            }
            if top.expires_at > now {
                break;
            }
            self.expiry_heap.pop();
            match top.entry {
                ExpiryEntry::Transport(slot_id) => {
                    if self.entries.remove(&slot_id).is_some() {
                        removed += 1;
                    }
                }
                ExpiryEntry::ProtocolContext(slot_id) => {
                    if self.remove_protocol_context(slot_id).is_some() {
                        removed += 1;
                    }
                }
                ExpiryEntry::Continuation(slot_id) => {
                    if self.remove_continuation(slot_id).is_some() {
                        removed += 1;
                    }
                }
            }
        }
        self.maybe_compact_expiry_heap();
        removed
    }
}

impl MemoryProviderTransportStore {
    pub fn new(retention: Duration) -> Self {
        let (maintenance_revision, _) = watch::channel(0);
        Self {
            retention,
            state: Arc::new(RwLock::new(TransportStoreState::default())),
            maintenance_started: Arc::new(AtomicBool::new(false)),
            maintenance_revision,
        }
    }

    fn ensure_expiry_maintenance(&self, notify_earliest: bool) {
        if notify_earliest {
            self.maintenance_revision
                .send_modify(|revision| *revision = revision.wrapping_add(1));
        }
        if self
            .maintenance_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let mut changes = self.maintenance_revision.subscribe();
        let state_weak = Arc::downgrade(&self.state);
        tokio::spawn(async move {
            loop {
                let Some(state) = state_weak.upgrade() else {
                    break;
                };
                let next_expiry = state.write().await.next_live_expiry();
                drop(state);
                match next_expiry {
                    None => {
                        if changes.changed().await.is_err() {
                            break;
                        }
                    }
                    Some(expiry) => {
                        let remaining = expiry - OffsetDateTime::now_utc();
                        if remaining <= Duration::ZERO {
                            let Some(state) = state_weak.upgrade() else {
                                break;
                            };
                            state.write().await.evict_expired(OffsetDateTime::now_utc());
                        } else {
                            let delay = std::time::Duration::try_from(remaining)
                                .unwrap_or(std::time::Duration::ZERO);
                            tokio::select! {
                                _ = tokio::time::sleep(delay) => {}
                                result = changes.changed() => {
                                    if result.is_err() { break; }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    #[cfg(test)]
    pub(crate) async fn retained_counts_for_test(&self) -> (usize, usize, usize) {
        let state = self.state.read().await;
        (
            state.entries.len(),
            state.protocol_contexts.len(),
            state.continuations.len(),
        )
    }

    fn validate_policy(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.retention > Duration::ZERO,
            "provider_transport_policy_invalid"
        );
        Ok(())
    }
}

#[async_trait]
impl ProviderTransportStore for MemoryProviderTransportStore {
    async fn put(
        &self,
        slot_id: ProviderTransportSlotId,
        payload: ProviderTransportPayload,
    ) -> anyhow::Result<()> {
        self.validate_policy()?;
        let expires_at = OffsetDateTime::now_utc() + self.retention;
        let is_earliest = {
            let mut state = self.state.write().await;
            let generation = state.alloc_generation();
            state.entries.insert(
                slot_id,
                TransportEntry {
                    payload,
                    expires_at,
                    generation,
                },
            );
            state.schedule_expiry(expires_at, generation, ExpiryEntry::Transport(slot_id))
        };
        self.ensure_expiry_maintenance(is_earliest);
        Ok(())
    }

    async fn get(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<Option<ProviderTransportPayload>> {
        let now = OffsetDateTime::now_utc();
        let mut state = self.state.write().await;
        let Some(entry) = state.entries.get(&slot_id).cloned() else {
            return Ok(None);
        };
        if entry.expires_at <= now {
            state.entries.remove(&slot_id);
            state.maybe_compact_expiry_heap();
            return Ok(None);
        }
        Ok(Some(entry.payload))
    }

    async fn consume(
        &self,
        slot_id: ProviderTransportSlotId,
    ) -> anyhow::Result<ProviderTransportPayload> {
        let now = OffsetDateTime::now_utc();
        let mut state = self.state.write().await;
        let entry = state.entries.remove(&slot_id);
        state.maybe_compact_expiry_heap();
        let entry = entry
            .filter(|entry| entry.expires_at > now)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_transport_missing"))?;
        Ok(entry.payload)
    }

    async fn delete(&self, slot_id: ProviderTransportSlotId) -> anyhow::Result<bool> {
        let mut state = self.state.write().await;
        let removed = state.entries.remove(&slot_id).is_some();
        if removed {
            state.maybe_compact_expiry_heap();
        }
        Ok(removed)
    }

    async fn put_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
        value: ProviderProtocolContextValue,
    ) -> anyhow::Result<()> {
        self.validate_policy()?;
        let expires_at = OffsetDateTime::now_utc() + self.retention;
        let is_earliest = {
            let mut state = self.state.write().await;
            let generation = state.alloc_generation();
            state.protocol_contexts.insert(
                slot_id,
                ProtocolContextEntry {
                    value,
                    expires_at,
                    generation,
                },
            );
            state
                .flow_protocol_contexts
                .entry(slot_id.flow_run_id())
                .or_default()
                .insert(slot_id);
            state.schedule_expiry(
                expires_at,
                generation,
                ExpiryEntry::ProtocolContext(slot_id),
            )
        };
        self.ensure_expiry_maintenance(is_earliest);
        Ok(())
    }

    async fn get_protocol_context(
        &self,
        slot_id: ProviderProtocolContextSlotId,
    ) -> anyhow::Result<Option<ProviderProtocolContextValue>> {
        let now = OffsetDateTime::now_utc();
        let mut state = self.state.write().await;
        let Some(entry) = state.protocol_contexts.get(&slot_id).cloned() else {
            return Ok(None);
        };
        if entry.expires_at <= now {
            state.remove_protocol_context(slot_id);
            state.maybe_compact_expiry_heap();
            return Ok(None);
        }
        Ok(Some(entry.value))
    }

    async fn delete_flow_run_protocol_contexts(&self, flow_run_id: Uuid) -> anyhow::Result<usize> {
        let mut state = self.state.write().await;
        let Some(slots) = state.flow_protocol_contexts.remove(&flow_run_id) else {
            return Ok(0);
        };
        let mut removed = 0;
        for slot_id in slots {
            if state.protocol_contexts.remove(&slot_id).is_some() {
                removed += 1;
            }
        }
        if removed > 0 {
            state.maybe_compact_expiry_heap();
        }
        Ok(removed)
    }

    async fn put_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
        continuation: ProviderContinuation,
    ) -> anyhow::Result<()> {
        self.validate_policy()?;
        let expires_at = OffsetDateTime::now_utc() + self.retention;
        let is_earliest = {
            let mut state = self.state.write().await;
            let generation = state.alloc_generation();
            state.continuations.insert(
                slot_id,
                ContinuationEntry {
                    continuation,
                    expires_at,
                    generation,
                },
            );
            state
                .flow_continuations
                .entry(slot_id.flow_run_id())
                .or_default()
                .insert(slot_id);
            state.schedule_expiry(expires_at, generation, ExpiryEntry::Continuation(slot_id))
        };
        self.ensure_expiry_maintenance(is_earliest);
        Ok(())
    }

    async fn get_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<Option<ProviderContinuation>> {
        let now = OffsetDateTime::now_utc();
        let mut state = self.state.write().await;
        let Some(entry) = state.continuations.get(&slot_id).cloned() else {
            return Ok(None);
        };
        if entry.expires_at <= now {
            state.remove_continuation(slot_id);
            state.maybe_compact_expiry_heap();
            return Ok(None);
        }
        Ok(Some(entry.continuation))
    }

    async fn consume_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<ProviderContinuation> {
        let now = OffsetDateTime::now_utc();
        let mut state = self.state.write().await;
        let entry = state.remove_continuation(slot_id);
        state.maybe_compact_expiry_heap();
        let entry = entry
            .filter(|entry| entry.expires_at > now)
            .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))?;
        Ok(entry.continuation)
    }

    async fn claim_continuation(
        &self,
        flow_run_id: Uuid,
        resume_claim_id: Uuid,
    ) -> anyhow::Result<ProviderContinuation> {
        let current = ProviderContinuationSlotId::for_flow_run(flow_run_id);
        let claimed = ProviderContinuationSlotId::for_resume_claim(flow_run_id, resume_claim_id);
        let now = OffsetDateTime::now_utc();
        let (continuation, is_earliest) = {
            let mut state = self.state.write().await;
            if let Some(entry) = state.continuations.get(&claimed).cloned() {
                if entry.expires_at > now {
                    return Ok(entry.continuation);
                }
                state.remove_continuation(claimed);
            }
            let entry = state
                .remove_continuation(current)
                .filter(|entry| entry.expires_at > now)
                .ok_or_else(|| anyhow::anyhow!("ephemeral_continuation_missing"))?;
            let generation = state.alloc_generation();
            let continuation = entry.continuation.clone();
            let expires_at = entry.expires_at;
            state.continuations.insert(
                claimed,
                ContinuationEntry {
                    continuation: entry.continuation,
                    expires_at,
                    generation,
                },
            );
            state
                .flow_continuations
                .entry(flow_run_id)
                .or_default()
                .insert(claimed);
            let is_earliest =
                state.schedule_expiry(expires_at, generation, ExpiryEntry::Continuation(claimed));
            (continuation, is_earliest)
        };
        self.ensure_expiry_maintenance(is_earliest);
        Ok(continuation)
    }

    async fn delete_continuation(
        &self,
        slot_id: ProviderContinuationSlotId,
    ) -> anyhow::Result<bool> {
        let mut state = self.state.write().await;
        let removed = state.remove_continuation(slot_id).is_some();
        if removed {
            state.maybe_compact_expiry_heap();
        }
        Ok(removed)
    }

    async fn clear_flow_run(&self, flow_run_id: Uuid) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state
            .entries
            .remove(&ProviderTransportSlotId::for_flow_run(flow_run_id));
        if let Some(slots) = state.flow_continuations.remove(&flow_run_id) {
            for slot_id in slots {
                state.continuations.remove(&slot_id);
            }
        }
        if let Some(slots) = state.flow_protocol_contexts.remove(&flow_run_id) {
            for slot_id in slots {
                state.protocol_contexts.remove(&slot_id);
            }
        }
        state.maybe_compact_expiry_heap();
        Ok(())
    }

    async fn clear_expired(&self) -> anyhow::Result<usize> {
        let now = OffsetDateTime::now_utc();
        Ok(self.state.write().await.evict_expired(now))
    }
}
