use super::{
    runtime::{Key, RuntimeInner, activity},
    runtime_types::*,
};
use control_plane_contracts::ports::{EventBusError, EventConsumer, EventDelivery};
use domain::event_center::EventTriggerPhase;
use std::sync::Arc;

pub struct EventCenterSubscription {
    inner: Arc<RuntimeInner>,
    spec: EventRuntimeSubscription,
    consumer: Option<Box<dyn EventConsumer>>,
}
impl EventCenterSubscription {
    pub(super) fn new(
        inner: Arc<RuntimeInner>,
        spec: EventRuntimeSubscription,
        consumer: Box<dyn EventConsumer>,
    ) -> Self {
        Self {
            inner,
            spec,
            consumer: Some(consumer),
        }
    }
    pub fn spec(&self) -> &EventRuntimeSubscription {
        &self.spec
    }

    /// Successful queue dequeue is the handoff fact. No business result can undo it.
    pub async fn recv(&self) -> anyhow::Result<Option<EventCenterDelivery>> {
        while self.ready().await? {
            if let Some(delivery) = self.try_recv()? {
                return Ok(Some(delivery));
            }
        }
        Ok(None)
    }

    /// Advisory, non-consuming readiness; current authorization may be checked after this wait.
    pub async fn ready(&self) -> anyhow::Result<bool> {
        Ok(self
            .consumer
            .as_ref()
            .ok_or(EventBusError::Closed)?
            .ready()
            .await?)
    }

    /// Atomic handoff without a suspension point. A competing consumer can win readiness;
    /// an empty result requires another readiness wait and a fresh validity check.
    pub fn try_recv(&self) -> anyhow::Result<Option<EventCenterDelivery>> {
        let consumer = self.consumer.as_ref().ok_or(EventBusError::Closed)?;
        let Some(delivery) = consumer.try_recv()? else {
            return Ok(None);
        };
        let signal: EventCenterSignal = serde_json::from_value(delivery.body().clone())
            .map_err(|_| EventBusError::InvalidBody)?;
        if signal.workspace_id != self.spec.workspace_id
            || signal.contract_id != self.spec.contract_id
            || signal.contract_version != self.spec.contract_version
            || signal.phase() != self.spec.trigger_phase
        {
            return Err(EventBusError::InvalidBody.into());
        }
        let key = (self.spec.workspace_id, self.spec.subscription_id);
        let mut state = self.inner.lock();
        let mut generation = None;
        if let Some(membership) = state.memberships.get_mut(&key) {
            generation = Some(membership.generation);
            membership.buffered = membership.buffered.saturating_sub(1);
            membership.in_flight = membership.in_flight.saturating_add(1);
            membership.consumed = membership.consumed.saturating_add(1);
        }
        // Lifecycle observers do not recursively generate consumed-of-consumed signals.
        // The business dequeue is recorded before attempting any observer admission.
        if signal.phase() == EventTriggerPhase::Published && signal.source_context.is_none() {
            let mut consumed = self.inner.signal(
                &self.spec,
                EventCenterSignalBody::Consumed {
                    source_event_id: signal.event_id.to_string(),
                    subscription_id: self.spec.subscription_id.to_string(),
                    consumer_id: self.spec.consumer_id.to_string(),
                },
            );
            consumed.workflow_ancestor_ids = signal.workflow_ancestor_ids.clone();
            state.observation.record(activity(&consumed, None));
            let _ = self.inner.dispatch(&mut state, &consumed);
        }
        drop(state);
        Ok(Some(EventCenterDelivery {
            signal,
            _delivery: delivery,
            _tracking: DeliveryTracking {
                inner: self.inner.clone(),
                key,
                generation,
            },
        }))
    }

    /// Close the logical queue for every competing consumer; already accepted work drains.
    pub fn close(&self) {
        let mut state = self.inner.lock();
        if let Some(membership) = state
            .memberships
            .get_mut(&(self.spec.workspace_id, self.spec.subscription_id))
        {
            membership.closed = true;
        }
        if let Some(consumer) = &self.consumer {
            consumer.close();
        }
    }
}
impl Drop for EventCenterSubscription {
    fn drop(&mut self) {
        let mut state = self.inner.lock();
        let key = (self.spec.workspace_id, self.spec.subscription_id);
        // Retire the adapter consumer before exposing membership removal, under
        // the same lock as subscribe. A concurrent reopen must get a fresh queue
        // generation rather than inherit buffered work from a disappearing handle.
        drop(self.consumer.take());
        if let Some(membership) = state.memberships.get_mut(&key) {
            membership.consumers.remove(&self.spec.consumer_id);
            if membership.consumers.is_empty() {
                state.memberships.remove(&key);
            }
        }
        // Live EventDelivery guards independently retain their in-flight charge.
    }
}

/// Owns the bus processing guard until the real consumer releases this delivery.
/// Dropping or completing it releases capacity, never business-success acknowledgement.
pub struct EventCenterDelivery {
    signal: EventCenterSignal,
    _delivery: EventDelivery,
    _tracking: DeliveryTracking,
}
impl EventCenterDelivery {
    pub fn signal(&self) -> &EventCenterSignal {
        &self.signal
    }
    pub fn complete(self) {}
}
struct DeliveryTracking {
    inner: Arc<RuntimeInner>,
    key: Key,
    generation: Option<uuid::Uuid>,
}
impl Drop for DeliveryTracking {
    fn drop(&mut self) {
        if let Some(membership) = self.inner.lock().memberships.get_mut(&self.key) {
            if self.generation != Some(membership.generation) {
                return;
            }
            membership.in_flight = membership.in_flight.saturating_sub(1);
        }
    }
}
