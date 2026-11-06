use super::{
    runtime::{activity, Membership, RuntimeInner},
    *,
};
use control_plane_contracts::ports::{
    EventSourceFact, EventSourceObservation, EventSourceObservationStatus as Status,
    EventSourceObserver, EventSourceRegistration,
};
use domain::event_center::*;
use std::sync::{Arc, Weak};
use uuid::Uuid;

pub(super) struct RegisteredSource {
    pub(super) descriptor: EventSourceDescriptor,
    queues: Vec<EventSourceQueueSnapshot>,
    pub(super) bytes: u64,
}
struct SourceLease {
    inner: Weak<RuntimeInner>,
    id: Uuid,
}

pub(super) fn matches_source(membership: &Membership, signal: &EventCenterSignal) -> bool {
    match (&membership.source_target, &signal.source_context) {
        (None, None) => true,
        (Some(target), Some(context)) => {
            target.scope == context.scope && target.source_id == context.source_id
        }
        _ => false,
    }
}
fn workspace(scope: &EventSourceScope) -> Uuid {
    match scope {
        EventSourceScope::Workspace { workspace_id } => *workspace_id,
        _ => domain::SYSTEM_SCOPE_ID,
    }
}
fn active(descriptor: &EventSourceDescriptor) -> bool {
    descriptor.binding == EventSourceBinding::Active
        && descriptor.coverage == EventSourceCoverage::Live
        && descriptor.scope != EventSourceScope::Unbound
}
fn metadata_bytes(
    descriptor: &EventSourceDescriptor,
    queues: &[EventSourceQueueSnapshot],
) -> Option<u64> {
    serde_json::to_vec(&(descriptor, queues)).ok().map(|bytes| {
        (bytes.len() as u64)
            .saturating_add(std::mem::size_of::<RegisteredSource>() as u64)
            .saturating_add(std::mem::size_of::<Uuid>() as u64)
    })
}
impl EventSourceObserver for EventCenterRuntime {
    fn register_source(
        &self,
        descriptor: EventSourceDescriptor,
    ) -> anyhow::Result<Arc<dyn EventSourceRegistration>> {
        self.inner
            .policy
            .ok_or(control_plane_contracts::ports::EventBusError::Unconfigured)?;
        if [
            &descriptor.source_id,
            &descriptor.owner_id,
            &descriptor.point_id,
            &descriptor.contract_id,
            &descriptor.contract_version,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(anyhow::anyhow!("source_identity_required"));
        }
        if !active(&descriptor) && !descriptor.supported_phases.is_empty() {
            return Err(anyhow::anyhow!(
                "inactive_source_cannot_claim_trigger_phases"
            ));
        }
        let bytes = metadata_bytes(&descriptor, &[])
            .ok_or_else(|| anyhow::anyhow!("source_metadata_invalid"))?;
        let mut state = self.inner.lock();
        // Source-owned metadata and activity share the explicit observation budget.
        if !state.observation.replace_metadata(0, bytes) {
            return Err(
                control_plane_contracts::ports::EventBusError::ResourceExhausted("source_metadata")
                    .into(),
            );
        }
        let id = Uuid::now_v7();
        state.sources.insert(
            id,
            RegisteredSource {
                descriptor,
                queues: Vec::new(),
                bytes,
            },
        );
        Ok(Arc::new(SourceLease {
            inner: Arc::downgrade(&self.inner),
            id,
        }))
    }
}
impl EventCenterRuntime {
    /// Caller authorizes the requested scope before exposing this read-only projection.
    /// Each live run/generation has its own registration ID; logical source_id stays stable.
    pub fn snapshot_sources(&self, scope: EventSourceScope) -> EventSourceRuntimeSnapshot {
        let state = self.inner.lock();
        let (activity, observation_bytes, observation_loss) =
            state.observation.snapshot_scope(&scope);
        let sources = state
            .sources
            .iter()
            .filter(|(_, source)| source.descriptor.scope == scope)
            .map(|(id, source)| EventRegisteredSourceSnapshot {
                registration_id: *id,
                descriptor: source.descriptor.clone(),
                queues: source.queues.clone(),
                metadata_bytes: source.bytes,
            })
            .collect();
        let source_metadata_bytes = state
            .sources
            .values()
            .filter(|source| source.descriptor.scope == scope)
            .map(|source| source.bytes)
            .sum::<u64>();
        let workflow_queues = state
            .memberships
            .values()
            .filter(|membership| {
                membership
                    .source_target
                    .as_ref()
                    .is_some_and(|target| target.scope == scope)
            })
            .map(super::runtime::queue_snapshot)
            .collect();
        EventSourceRuntimeSnapshot {
            instance_id: self.inner.instance_id,
            configured: self.inner.policy.is_some(),
            observation_max_bytes: self
                .inner
                .policy
                .map_or(0, |policy| policy.observation_max_bytes),
            observation_ttl_ms: self
                .inner
                .policy
                .map_or(0, |policy| policy.observation_ttl_ms),
            workflow_queues,
            scope,
            sources,
            activity,
            observation_bytes: observation_bytes + source_metadata_bytes,
            source_metadata_bytes,
            observation_loss,
        }
    }
    pub fn subscribe_workflow(
        &self,
        spec: EventRuntimeSubscription,
        emit_subscribed: bool,
        workflow_id: Uuid,
    ) -> anyhow::Result<EventCenterSubscription> {
        self.subscribe_bound(spec, emit_subscribed, None, Some(workflow_id))
    }
    pub fn subscribe_source(
        &self,
        spec: EventRuntimeSubscription,
        target: EventSourceTarget,
    ) -> anyhow::Result<EventCenterSubscription> {
        if target.scope == EventSourceScope::Unbound
            || target.source_id.trim().is_empty()
            || matches!(&target.scope, EventSourceScope::Workspace { workspace_id } if *workspace_id != spec.workspace_id)
        {
            return Err(anyhow::anyhow!("source_subscription_scope_invalid"));
        }
        // Configuration restore is not an original source subscription fact.
        let workflow_id = target.workflow_id;
        self.subscribe_bound(spec, false, Some(target), Some(workflow_id))
    }
}
impl EventSourceRegistration for SourceLease {
    fn observe(&self, observation: EventSourceObservation) -> Status {
        self.observe_fact(observation, None)
    }
    fn observe_with_payload(
        &self,
        observation: EventSourceObservation,
        payload: &mut dyn FnMut() -> Option<serde_json::Value>,
    ) -> Status {
        self.observe_fact(observation, Some(payload))
    }
    fn set_queue_snapshots(&self, queues: Vec<EventSourceQueueSnapshot>) -> bool {
        self.replace_queues(queues)
    }
}
impl SourceLease {
    fn observe_fact(
        &self,
        observation: EventSourceObservation,
        payload: Option<&mut dyn FnMut() -> Option<serde_json::Value>>,
    ) -> Status {
        let Some(inner) = self.inner.upgrade() else {
            return Status::Unconfigured;
        };
        let mut state = inner.lock();
        let Some(source) = state.sources.get(&self.id) else {
            return Status::Inactive;
        };
        let descriptor = &source.descriptor;
        if !active(descriptor) {
            return Status::Inactive;
        }
        let phase = match &observation.fact {
            EventSourceFact::Published => EventTriggerPhase::Published,
            EventSourceFact::Subscribed { .. } => EventTriggerPhase::Subscribed,
            EventSourceFact::Consumed { .. } => EventTriggerPhase::Consumed,
        };
        if !descriptor.supported_phases.contains(&phase) {
            return Status::UnsupportedPhase;
        }
        if phase != EventTriggerPhase::Subscribed
            && observation
                .original_event_id
                .as_ref()
                .is_none_or(|id| id.trim().is_empty())
        {
            return Status::InvalidIdentity;
        }
        if match &observation.fact {
            EventSourceFact::Subscribed {
                subscriber_id,
                consumer_id,
            }
            | EventSourceFact::Consumed {
                subscriber_id,
                consumer_id,
            } => subscriber_id.trim().is_empty() || consumer_id.trim().is_empty(),
            _ => false,
        } {
            return Status::InvalidIdentity;
        }
        let published = matches!(&observation.fact, EventSourceFact::Published);
        let context = EventSourceContext {
            registration_id: self.id,
            scope: descriptor.scope.clone(),
            source_id: descriptor.source_id.clone(),
            point_id: descriptor.point_id.clone(),
            original_event_id: observation.original_event_id,
            producer_id: observation.producer_id,
        };
        let body = match observation.fact {
            EventSourceFact::Published => EventCenterSignalBody::Published {
                payload: serde_json::Value::Null,
            },
            EventSourceFact::Subscribed {
                subscriber_id,
                consumer_id,
            } => EventCenterSignalBody::Subscribed {
                subscription_id: subscriber_id,
                consumer_id,
            },
            EventSourceFact::Consumed {
                subscriber_id,
                consumer_id,
            } => EventCenterSignalBody::Consumed {
                source_event_id: context.original_event_id.clone().unwrap_or_default(),
                subscription_id: subscriber_id,
                consumer_id,
            },
        };
        let mut signal = EventCenterSignal {
            event_id: Uuid::now_v7(),
            occurred_at_unix_ms: observation.occurred_at_unix_ms,
            workspace_id: workspace(&context.scope),
            contract_id: descriptor.contract_id.clone(),
            contract_version: descriptor.contract_version.clone(),
            source_context: Some(context),
            workflow_ancestor_ids: observation.workflow_ancestor_ids,
            body,
        };
        // Record the owner fact even if all workflow admissions fail. Payload never enters activity.
        state.observation.record(activity(&signal, None));
        if published {
            let needs_body = state.memberships.values().any(|membership| {
                matches_source(membership, &signal)
                    && membership.spec.trigger_phase == signal.phase()
                    && membership.spec.contract_id == signal.contract_id
                    && membership.spec.contract_version == signal.contract_version
                    && !membership.closed
                    && membership
                        .workflow_id
                        .is_none_or(|id| !signal.workflow_ancestor_ids.contains(&id))
            });
            if needs_body {
                let Some(project) = payload else {
                    return Status::InvalidBody;
                };
                let Some(body) = project() else {
                    return Status::InvalidBody;
                };
                signal.body = EventCenterSignalBody::Published { payload: body };
            }
            // Without an eligible target, dispatch can only record Closed/Reentrant
            // admission facts. No placeholder Published body can enter a workflow queue.
        }
        match inner.dispatch(&mut state, &signal) {
            Ok(_) => Status::Observed,
            Err(_) => Status::InvalidBody,
        }
    }
    fn replace_queues(&self, queues: Vec<EventSourceQueueSnapshot>) -> bool {
        let Some(inner) = self.inner.upgrade() else {
            return false;
        };
        let mut state = inner.lock();
        let Some(source) = state.sources.get(&self.id) else {
            return false;
        };
        let Some(bytes) = metadata_bytes(&source.descriptor, &queues) else {
            return false;
        };
        let previous = source.bytes;
        if !state.observation.replace_metadata(previous, bytes) {
            return false;
        }
        if let Some(source) = state.sources.get_mut(&self.id) {
            source.queues = queues;
            source.bytes = bytes;
        }
        true
    }
}
impl Drop for SourceLease {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            let mut state = inner.lock();
            if let Some(source) = state.sources.remove(&self.id) {
                let released = state.observation.replace_metadata(source.bytes, 0);
                debug_assert!(
                    released,
                    "live source metadata reservation must be released"
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "_tests/source.rs"]
mod tests;
