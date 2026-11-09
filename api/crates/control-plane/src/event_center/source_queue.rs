//! The sole retained queue-observation ledger. Hot counters never resize or serialize it.
use super::source::SourceLease;
use control_plane_contracts::ports::{
    EventSourceFact, EventSourceObservation, EventSourceObservationStatus as Status,
    EventSourceQueueChange, EventSourceQueueConsumption, EventSourceQueueRegistration,
};
use domain::event_center::EventSourceQueueSnapshot;
use std::{mem::size_of, sync::Arc};
use uuid::Uuid;

pub(super) struct SourceQueueEntry {
    pub snapshot: Option<EventSourceQueueSnapshot>,
    pub bytes: u64,
    pub incremental: bool,
}
struct QueueLease {
    source: Arc<SourceLease>,
    id: Uuid,
}
fn fixed_bytes(incremental: bool) -> u64 {
    (size_of::<SourceQueueEntry>()
        + size_of::<Uuid>()
        + if incremental {
            size_of::<QueueLease>()
                + 2 * size_of::<usize>()
                + size_of::<Arc<dyn EventSourceQueueRegistration>>()
        } else {
            0
        }) as u64
}
pub(super) fn queue_entry(
    snapshot: EventSourceQueueSnapshot,
    incremental: bool,
) -> Option<SourceQueueEntry> {
    // Reserve the maximum serialized width of numeric fields once at admission. Unknown
    // counters stay unknown in the actual facts; this projection is accounting only.
    let mut maximum = snapshot.clone();
    maximum.buffered = Some(u64::MAX);
    maximum.in_flight = Some(u64::MAX);
    maximum.accepted = Some(u64::MAX);
    maximum.dequeued = Some(u64::MAX);
    maximum.dropped = Some(u64::MAX);
    maximum.observed_at_unix_ms = i64::MIN;
    let bytes =
        fixed_bytes(incremental).checked_add(serde_json::to_vec(&maximum).ok()?.len() as u64)?;
    Some(SourceQueueEntry {
        snapshot: Some(snapshot),
        bytes,
        incremental,
    })
}
impl SourceLease {
    pub(super) fn open_observed_queue(
        self: Arc<Self>,
        snapshot: EventSourceQueueSnapshot,
    ) -> Option<Arc<dyn EventSourceQueueRegistration>> {
        let inner = self.inner.upgrade()?;
        let mut state = inner.lock();
        let source = state.sources.get(&self.id)?;
        let previous = source.bytes;
        if snapshot.queue_id.trim().is_empty()
            || snapshot.subscriber_id.trim().is_empty()
            || snapshot.consumer_ids.len() != 1
            || snapshot.consumer_ids[0].trim().is_empty()
        {
            state.sources.get_mut(&self.id)?.queue_observation_complete = false;
            return None;
        }
        let candidate = queue_entry(snapshot, true);
        let Some(entry) = candidate else {
            state.sources.get_mut(&self.id)?.queue_observation_complete = false;
            return None;
        };
        let Some(next) = previous.checked_add(entry.bytes) else {
            state.sources.get_mut(&self.id)?.queue_observation_complete = false;
            return None;
        };
        if !state.observation.replace_metadata(previous, next) {
            state.sources.get_mut(&self.id)?.queue_observation_complete = false;
            return None;
        }
        let id = Uuid::now_v7();
        let source = state.sources.get_mut(&self.id)?;
        source.queues.insert(id, entry);
        source.bytes = next;
        drop(state);
        Some(Arc::new(QueueLease { source: self, id }))
    }
}
impl EventSourceQueueRegistration for QueueLease {
    fn record(&self, change: EventSourceQueueChange, at: i64) -> bool {
        let Some(inner) = self.source.inner.upgrade() else {
            return false;
        };
        let mut state = inner.lock();
        let Some(queue) = state
            .sources
            .get_mut(&self.source.id)
            .and_then(|source| source.queues.get_mut(&self.id))
            .and_then(|entry| entry.snapshot.as_mut())
        else {
            return false;
        };
        match change {
            EventSourceQueueChange::Accepted => {
                increment(&mut queue.accepted);
                increment(&mut queue.buffered);
            }
            EventSourceQueueChange::Dropped => increment(&mut queue.dropped),
        }
        queue.observed_at_unix_ms = at;
        true
    }
    fn consumed(&self, event: EventSourceQueueConsumption) -> Status {
        let Some(inner) = self.source.inner.upgrade() else {
            return Status::Inactive;
        };
        let identity = {
            let mut state = inner.lock();
            let Some(queue) = state
                .sources
                .get_mut(&self.source.id)
                .and_then(|source| source.queues.get_mut(&self.id))
                .and_then(|entry| entry.snapshot.as_mut())
            else {
                return Status::Inactive;
            };
            increment(&mut queue.dequeued);
            if let Some(buffered) = &mut queue.buffered {
                *buffered = buffered.saturating_sub(1);
            }
            queue.observed_at_unix_ms = event.occurred_at_unix_ms;
            (
                queue.subscriber_id.clone(),
                queue.consumer_ids.first().cloned(),
            )
        };
        let Some(consumer_id) = identity.1 else {
            return Status::InvalidIdentity;
        };
        self.source.observe_fact(
            EventSourceObservation {
                attributes: event.attributes.clone(),

                original_event_id: Some(event.original_event_id),
                producer_id: event.producer_id,
                occurred_at_unix_ms: event.occurred_at_unix_ms,
                workflow_ancestor_ids: event.workflow_ancestor_ids,
                fact: EventSourceFact::Consumed {
                    subscriber_id: identity.0,
                    consumer_id,
                },
            },
            None,
        )
    }
    fn retire(&self) {
        let Some(inner) = self.source.inner.upgrade() else {
            return;
        };
        let mut state = inner.lock();
        let Some(source) = state.sources.get_mut(&self.source.id) else {
            return;
        };
        let Some(queue) = source.queues.get_mut(&self.id) else {
            return;
        };
        if queue.snapshot.take().is_none() {
            return;
        }
        // Retired sender/receiver clones may still own this handle. Keep their fixed guard
        // and map-node charge until last drop, while releasing all queue strings immediately.
        let released = queue.bytes - fixed_bytes(true);
        queue.bytes = fixed_bytes(true);
        let previous = source.bytes;
        source.bytes -= released;
        let next = source.bytes;
        let accepted = state.observation.replace_metadata(previous, next);
        debug_assert!(accepted, "queue retirement only releases metadata");
    }
}
impl Drop for QueueLease {
    fn drop(&mut self) {
        let Some(inner) = self.source.inner.upgrade() else {
            return;
        };
        let mut state = inner.lock();
        let Some(source) = state.sources.get_mut(&self.source.id) else {
            return;
        };
        if let Some(queue) = source.queues.remove(&self.id) {
            let previous = source.bytes;
            source.bytes -= queue.bytes;
            let next = source.bytes;
            let accepted = state.observation.replace_metadata(previous, next);
            debug_assert!(accepted, "queue final drop only releases metadata");
        }
    }
}
fn increment(counter: &mut Option<u64>) {
    if let Some(value) = counter {
        *value = value.saturating_add(1);
    }
}

#[cfg(test)]
#[path = "_tests/source_queue.rs"]
mod tests;
