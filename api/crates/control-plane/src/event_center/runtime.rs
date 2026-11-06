use super::{observation::Observation, runtime_types::*, subscription::EventCenterSubscription};
use control_plane_contracts::{
    ControlPlaneContractError as Error,
    ports::{EventBus, EventBusError},
};
use domain::event_center::EventDefinitionRecord;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
    sync::{Arc, Mutex, MutexGuard},
};
use time::OffsetDateTime;
use uuid::Uuid;

pub(super) type Key = (Uuid, Uuid);
#[derive(Clone)]
pub struct EventCenterRuntime {
    pub(super) inner: Arc<RuntimeInner>,
}
pub(super) struct RuntimeInner {
    pub instance_id: Uuid,
    pub bus: Arc<dyn EventBus>,
    pub policy: Option<EventCenterRuntimePolicy>,
    state: Mutex<RuntimeState>,
}
pub(super) struct RuntimeState {
    pub memberships: BTreeMap<Key, Membership>,
    pub observation: Observation,
    pub sources: BTreeMap<Uuid, super::source::RegisteredSource>,
}
pub(super) struct Membership {
    pub generation: Uuid,
    pub spec: EventRuntimeSubscription,
    pub source_target: Option<EventSourceTarget>,
    pub workflow_id: Option<Uuid>,
    pub topic: String,
    pub consumers: BTreeSet<Uuid>,
    pub closed: bool,
    pub buffered: u64,
    pub in_flight: u64,
    pub accepted: u64,
    pub consumed: u64,
    pub admission_rejected: u64,
}
impl RuntimeInner {
    pub fn lock(&self) -> MutexGuard<'_, RuntimeState> {
        // No await occurs under this lock. Source payload callbacks only project local data.
        // Drop must recover accounting even if a source projection panics.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn signal(
        &self,
        spec: &EventRuntimeSubscription,
        body: EventCenterSignalBody,
    ) -> EventCenterSignal {
        EventCenterSignal {
            source_context: None,
            workflow_ancestor_ids: Vec::new(),
            event_id: Uuid::now_v7(),
            occurred_at_unix_ms: unix_ms(),
            workspace_id: spec.workspace_id,
            contract_id: spec.contract_id.clone(),
            contract_version: spec.contract_version.clone(),
            body,
        }
    }

    /// Nonblocking, per-target admission. A control signal never waits for queue capacity.
    pub fn dispatch(
        &self,
        state: &mut RuntimeState,
        signal: &EventCenterSignal,
    ) -> Result<Vec<EventTargetAdmission>, EventBusError> {
        let mut targets = Vec::new();
        let RuntimeState {
            memberships,
            observation,
            ..
        } = state;
        for membership in memberships.values_mut().filter(|membership| {
            super::source::matches_source(membership, signal)
                && membership.spec.workspace_id == signal.workspace_id
                && membership.spec.contract_id == signal.contract_id
                && membership.spec.contract_version == signal.contract_version
                && membership.spec.trigger_phase == signal.phase()
        }) {
            if membership
                .workflow_id
                .is_some_and(|id| signal.workflow_ancestor_ids.contains(&id))
            {
                membership.admission_rejected = membership.admission_rejected.saturating_add(1);
                let target = EventTargetAdmission {
                    subscription_id: membership.spec.subscription_id,
                    status: EventTargetAdmissionStatus::Reentrant,
                };
                observation.record(activity(signal, Some(&target)));
                targets.push(target);
                continue;
            }
            let mut outgoing = signal.clone();
            if let Some(id) = membership.workflow_id {
                outgoing.workflow_ancestor_ids.push(id);
            }
            let bytes = json_bytes(&outgoing)?;
            let result = if membership.closed {
                Err(EventBusError::Closed)
            } else {
                self.bus
                    .try_reserve(&membership.topic, bytes)
                    .and_then(|permit| {
                        let body = serde_json::to_value(&outgoing)
                            .map_err(|_| EventBusError::InvalidBody)?;
                        permit.send(body)
                    })
            };
            let status = match result {
                Ok(()) => {
                    membership.accepted = membership.accepted.saturating_add(1);
                    membership.buffered = membership.buffered.saturating_add(1);
                    EventTargetAdmissionStatus::Accepted
                }
                Err(error) => {
                    membership.admission_rejected = membership.admission_rejected.saturating_add(1);
                    match error {
                        EventBusError::NoConsumer => EventTargetAdmissionStatus::NoConsumer,
                        EventBusError::Closed => EventTargetAdmissionStatus::Closed,
                        EventBusError::Unconfigured => EventTargetAdmissionStatus::Unconfigured,
                        EventBusError::InvalidBody | EventBusError::BodyExceedsReservation => {
                            EventTargetAdmissionStatus::InvalidBody
                        }
                        _ => EventTargetAdmissionStatus::ResourceExhausted,
                    }
                }
            };
            let target = EventTargetAdmission {
                subscription_id: membership.spec.subscription_id,
                status,
            };
            observation.record(activity(signal, Some(&target)));
            targets.push(target);
        }
        Ok(targets)
    }
}

impl EventCenterRuntime {
    pub fn new(
        bus: Arc<dyn EventBus>,
        policy: Option<EventCenterRuntimePolicy>,
    ) -> anyhow::Result<Self> {
        if let Some(policy) = policy {
            bus.configure_budget(policy.bus_limits)?;
        }
        Ok(Self {
            inner: Arc::new(RuntimeInner {
                instance_id: Uuid::now_v7(),
                bus,
                policy,
                state: Mutex::new(RuntimeState {
                    memberships: BTreeMap::new(),
                    sources: BTreeMap::new(),
                    observation: Observation::new(policy),
                }),
            }),
        })
    }

    pub fn subscribe(
        &self,
        spec: EventRuntimeSubscription,
        emit_subscribed: bool,
    ) -> anyhow::Result<EventCenterSubscription> {
        self.subscribe_bound(spec, emit_subscribed, None, None)
    }

    pub(super) fn subscribe_bound(
        &self,
        spec: EventRuntimeSubscription,
        emit_subscribed: bool,
        source_target: Option<EventSourceTarget>,
        workflow_id: Option<Uuid>,
    ) -> anyhow::Result<EventCenterSubscription> {
        if self.inner.policy.is_none() {
            return Err(EventBusError::Unconfigured.into());
        }
        if spec.contract_id.trim().is_empty() || spec.contract_version.trim().is_empty() {
            return Err(Error::InvalidInput("event_identity_required").into());
        }
        let key = (spec.workspace_id, spec.subscription_id);
        let mut state = self.inner.lock();
        let first = !state.memberships.contains_key(&key);
        let topic = if let Some(existing) = state.memberships.get(&key) {
            if existing.workflow_id != workflow_id
                || existing.source_target != source_target
                || existing.spec.contract_id != spec.contract_id
                || existing.spec.contract_version != spec.contract_version
                || existing.spec.trigger_phase != spec.trigger_phase
            {
                return Err(Error::Conflict("event_subscription_identity").into());
            }
            if existing.closed {
                return Err(EventBusError::Closed.into());
            }
            if existing.consumers.contains(&spec.consumer_id) {
                return Err(Error::Conflict("event_consumer_identity").into());
            }
            existing.topic.clone()
        } else {
            // Include the complete contract identity so the bus's topic-name byte policy
            // also bounds membership identity metadata; UUIDs isolate runtime generations.
            format!(
                "event-center/{}/{}/{}/{}/{}:{}",
                self.inner.instance_id,
                spec.workspace_id,
                spec.subscription_id,
                spec.contract_id.len(),
                spec.contract_id,
                spec.contract_version
            ) + &source_target
                .as_ref()
                .map(|target| format!("/source/{}", target.source_id))
                .unwrap_or_default()
        };
        let consumer = self.inner.bus.open_consumer(&topic)?;
        // Dispatch against the existing observer set, under the membership lock, before
        // adding this member. A subscribed observer must never receive its own join.
        if first && emit_subscribed {
            let signal = self.inner.signal(
                &spec,
                EventCenterSignalBody::Subscribed {
                    subscription_id: spec.subscription_id.to_string(),
                    consumer_id: spec.consumer_id.to_string(),
                },
            );
            state.observation.record(activity(&signal, None));
            // A rejected observer must never undo logical registration.
            let _ = self.inner.dispatch(&mut state, &signal);
        }
        let membership = state.memberships.entry(key).or_insert_with(|| Membership {
            generation: Uuid::now_v7(),
            spec: spec.clone(),
            source_target,
            workflow_id,
            topic,
            consumers: BTreeSet::new(),
            closed: false,
            buffered: 0,
            in_flight: 0,
            accepted: 0,
            consumed: 0,
            admission_rejected: 0,
        });
        membership.consumers.insert(spec.consumer_id);
        drop(state);
        Ok(EventCenterSubscription::new(
            self.inner.clone(),
            spec,
            consumer,
        ))
    }

    pub fn publish(
        &self,
        definition: &EventDefinitionRecord,
        payload: Value,
    ) -> anyhow::Result<EventPublishReceipt> {
        self.publish_with_ancestors(definition, payload, Vec::new())
    }

    pub fn publish_with_ancestors(
        &self,
        definition: &EventDefinitionRecord,
        payload: Value,
        workflow_ancestor_ids: Vec<Uuid>,
    ) -> anyhow::Result<EventPublishReceipt> {
        if self.inner.policy.is_none() {
            return Err(EventBusError::Unconfigured.into());
        }
        if !definition.enabled {
            return Err(Error::InvalidInput("event_unavailable").into());
        }
        super::configuration::validate_identity(
            &definition.contract_id,
            &definition.contract_version,
        )?;
        let validator = super::configuration::payload_validator(&definition.payload_schema)?;
        if !validator.is_valid(&payload) {
            return Err(Error::InvalidInput("event_payload_schema_violation").into());
        }
        let signal = EventCenterSignal {
            source_context: None,
            workflow_ancestor_ids,
            event_id: Uuid::now_v7(),
            occurred_at_unix_ms: unix_ms(),
            workspace_id: definition.workspace_id,
            contract_id: definition.contract_id.clone(),
            contract_version: definition.contract_version.clone(),
            body: EventCenterSignalBody::Published { payload },
        };
        let mut state = self.inner.lock();
        state.observation.record(activity(&signal, None));
        let targets = self.inner.dispatch(&mut state, &signal)?;
        Ok(EventPublishReceipt {
            event_id: signal.event_id,
            workspace_id: signal.workspace_id,
            targets,
        })
    }

    pub fn snapshot(&self, workspace_id: Uuid) -> EventRuntimeSnapshot {
        let state = self.inner.lock();
        let (activity, observation_bytes, observation_loss) =
            state.observation.snapshot(workspace_id);
        let queues = state
            .memberships
            .values()
            .filter(|membership| {
                membership.spec.workspace_id == workspace_id
                    && membership.source_target.as_ref().is_none_or(|target| {
                        target.scope
                            == domain::event_center::EventSourceScope::Workspace { workspace_id }
                    })
            })
            .map(queue_snapshot)
            .collect();
        EventRuntimeSnapshot {
            instance_id: self.inner.instance_id,
            configured: self.inner.policy.is_some(),
            workspace_id,
            queues,
            admission_rejected: activity
                .iter()
                .filter(|value| {
                    value
                        .admission
                        .is_some_and(|status| status != EventTargetAdmissionStatus::Accepted)
                })
                .count() as u64,
            activity,
            observation_bytes,
            observation_loss,
            observation_max_bytes: self
                .inner
                .policy
                .map_or(0, |policy| policy.observation_max_bytes),
            observation_ttl_ms: self
                .inner
                .policy
                .map_or(0, |policy| policy.observation_ttl_ms),
        }
    }
}

pub(super) fn activity(
    signal: &EventCenterSignal,
    target: Option<&EventTargetAdmission>,
) -> EventRuntimeActivity {
    let (source_event_id, subscription_id, consumer_id) = match &signal.body {
        EventCenterSignalBody::Published { .. } => (None, None, None),
        EventCenterSignalBody::Subscribed {
            subscription_id,
            consumer_id,
        } => (
            None,
            Uuid::parse_str(subscription_id).ok(),
            Uuid::parse_str(consumer_id).ok(),
        ),
        EventCenterSignalBody::Consumed {
            source_event_id,
            subscription_id,
            consumer_id,
        } => (
            Uuid::parse_str(source_event_id).ok(),
            Uuid::parse_str(subscription_id).ok(),
            Uuid::parse_str(consumer_id).ok(),
        ),
    };
    let (original_subscriber_id, original_consumer_id) = match &signal.body {
        EventCenterSignalBody::Subscribed {
            subscription_id,
            consumer_id,
        }
        | EventCenterSignalBody::Consumed {
            subscription_id,
            consumer_id,
            ..
        } if signal.source_context.is_some() => {
            (Some(subscription_id.clone()), Some(consumer_id.clone()))
        }
        _ => (None, None),
    };
    EventRuntimeActivity {
        source_context: signal.source_context.clone(),
        original_subscriber_id,
        original_consumer_id,
        event_id: signal.event_id,
        workspace_id: signal.workspace_id,
        contract_id: signal.contract_id.clone(),
        contract_version: signal.contract_version.clone(),
        phase: signal.phase(),
        source_event_id,
        subscription_id: target.map(|value| value.subscription_id).or(
            if signal.source_context.is_some() {
                None
            } else {
                subscription_id
            },
        ),
        consumer_id: if signal.source_context.is_some() {
            None
        } else {
            consumer_id
        },
        admission: target.map(|value| value.status),
        observed_at_unix_ms: signal.occurred_at_unix_ms,
    }
}
pub(super) fn unix_ms() -> i64 {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}
fn json_bytes(value: &impl serde::Serialize) -> Result<u64, EventBusError> {
    struct Counter(u64);
    impl Write for Counter {
        fn write(&mut self, value: &[u8]) -> io::Result<usize> {
            self.0 = self
                .0
                .checked_add(value.len() as u64)
                .ok_or_else(|| io::Error::other("JSON size overflow"))?;
            Ok(value.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value).map_err(|_| EventBusError::InvalidBody)?;
    Ok(counter.0)
}

#[cfg(test)]
#[path = "_tests/runtime.rs"]
mod tests;

pub(super) fn queue_snapshot(membership: &Membership) -> EventRuntimeQueueSnapshot {
    EventRuntimeQueueSnapshot {
        source_target: membership.source_target.clone(),
        subscription_id: membership.spec.subscription_id,
        workspace_id: membership.spec.workspace_id,
        contract_id: membership.spec.contract_id.clone(),
        contract_version: membership.spec.contract_version.clone(),
        trigger_phase: membership.spec.trigger_phase,
        consumer_ids: membership.consumers.iter().copied().collect(),
        closed: membership.closed,
        buffered: membership.buffered,
        in_flight: membership.in_flight,
        accepted: membership.accepted,
        consumed: membership.consumed,
        admission_rejected: membership.admission_rejected,
    }
}
