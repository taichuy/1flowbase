use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::types::{
    AdmissionRequest, CapacityRejection, DeadlineKind, InvocationCompletion, InvocationLease,
    InvocationRequest, LifecycleEvent, RegistryError, SafeRegistrySnapshot, SafeSessionSnapshot,
    TerminationKind, TerminationReceipt, TransportDeadline, TransportFence, TransportFenceStatus,
    TransportGeneration, TransportInstant, TransportOwnerId, TransportProviderId,
    TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId, TransportSessionState,
};

pub trait TransportClock: Send + Sync {
    fn now(&self) -> TransportInstant;
}

#[derive(Clone, Debug)]
pub struct SystemTransportClock {
    monotonic_origin: Instant,
    unix_origin_millis: u64,
}

impl Default for SystemTransportClock {
    fn default() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        Self {
            monotonic_origin: Instant::now(),
            unix_origin_millis: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        }
    }
}

impl TransportClock for SystemTransportClock {
    fn now(&self) -> TransportInstant {
        let monotonic_millis =
            u64::try_from(self.monotonic_origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        TransportInstant::from_millis(self.unix_origin_millis.saturating_add(monotonic_millis))
    }
}

struct LogicalSessionRecord {
    owner_id: TransportOwnerId,
    provider_id: TransportProviderId,
    state: TransportSessionState,
    created_at: TransportInstant,
    state_since: TransportInstant,
    absolute_deadline: TransportDeadline,
    state_deadline: TransportDeadline,
    invocation: Option<InvocationRecord>,
    next_invocation: u64,
}

struct InvocationRecord {
    sequence: u64,
    deadline: TransportDeadline,
}

struct PhysicalGenerationRecord {
    runtime_target_id: TransportRuntimeTargetId,
    generation: TransportGeneration,
    created_at: TransportInstant,
    soft_deadline: TransportDeadline,
    hard_deadline: TransportDeadline,
    close_acknowledged: Option<bool>,
}

struct SessionRecord {
    logical: LogicalSessionRecord,
    physical: PhysicalGenerationRecord,
}

pub struct TransportSessionRegistry<C> {
    clock: C,
    config: TransportRegistryConfig,
    sessions: BTreeMap<TransportSessionId, SessionRecord>,
    tombstones: VecDeque<TerminationReceipt>,
    events: VecDeque<LifecycleEvent>,
    next_generation: u64,
}

impl<C: TransportClock> TransportSessionRegistry<C> {
    pub fn new(clock: C, config: TransportRegistryConfig) -> Result<Self, RegistryError> {
        config.validate()?;
        Ok(Self {
            clock,
            config,
            sessions: BTreeMap::new(),
            tombstones: VecDeque::new(),
            events: VecDeque::new(),
            next_generation: 1,
        })
    }

    pub fn admit(&mut self, request: AdmissionRequest) -> Result<TransportFence, RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        if self.sessions.contains_key(&request.session_id) {
            return Err(RegistryError::AlreadyExists);
        }

        let logical_absolute_deadline = now.saturating_add(self.config.logical_max_age);
        let physical_max_deadline = now.saturating_add(self.config.physical_max_age);
        let physical_hard_deadline = request
            .provider_hard_deadline
            .map_or(physical_max_deadline, |deadline| {
                deadline.min(physical_max_deadline)
            });
        if physical_hard_deadline <= now {
            return Err(RegistryError::DeadlineInPast);
        }

        if self.sessions.len() >= self.config.capacity {
            if let Some(session_id) = self.eviction_candidate() {
                self.terminate_by_id(&session_id, TerminationKind::CapacityEvicted, now);
            } else {
                return Err(RegistryError::Capacity(CapacityRejection {
                    capacity: self.config.capacity,
                    active: self
                        .sessions
                        .values()
                        .filter(|record| record.logical.state == TransportSessionState::Active)
                        .count(),
                }));
            }
        }

        let generation = self.allocate_generation()?;
        let physical_soft_deadline = now
            .saturating_add(self.config.physical_soft_drain_age)
            .min(physical_hard_deadline);
        let fence = TransportFence {
            session_id: request.session_id.clone(),
            generation,
        };
        self.sessions.insert(
            request.session_id,
            SessionRecord {
                logical: LogicalSessionRecord {
                    owner_id: request.owner_id,
                    provider_id: request.provider_id,
                    state: TransportSessionState::Opening,
                    created_at: now,
                    state_since: now,
                    absolute_deadline: logical_absolute_deadline,
                    state_deadline: logical_absolute_deadline,
                    invocation: None,
                    next_invocation: 1,
                },
                physical: PhysicalGenerationRecord {
                    runtime_target_id: request.runtime_target_id,
                    generation,
                    created_at: now,
                    soft_deadline: physical_soft_deadline,
                    hard_deadline: physical_hard_deadline,
                    close_acknowledged: None,
                },
            },
        );
        Ok(fence)
    }

    pub fn activate(&mut self, fence: &TransportFence) -> Result<(), RegistryError> {
        self.transition(fence, TransportSessionState::Active)
    }

    pub fn transition(
        &mut self,
        fence: &TransportFence,
        to: TransportSessionState,
    ) -> Result<(), RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        let from = self.record(fence)?.logical.state;
        if !valid_transition(from, to) {
            return Err(RegistryError::InvalidTransition { from, to });
        }
        if self.record(fence)?.logical.invocation.is_some()
            && !matches!(
                to,
                TransportSessionState::Draining
                    | TransportSessionState::Faulted
                    | TransportSessionState::Closing
            )
        {
            return Err(RegistryError::InflightExists);
        }
        self.set_state(fence, to, now)
    }

    pub fn begin_invocation(
        &mut self,
        fence: &TransportFence,
        request: InvocationRequest,
    ) -> Result<InvocationLease, RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        let default_deadline = now.saturating_add(self.config.invocation_default);
        let deadline = request.deadline.unwrap_or(default_deadline);
        let record = self.record_mut(fence)?;
        if deadline <= now {
            return Err(RegistryError::DeadlineInPast);
        }
        if record.logical.invocation.is_some() {
            return Err(RegistryError::InflightExists);
        }
        if !matches!(
            record.logical.state,
            TransportSessionState::Active
                | TransportSessionState::WaitingTool
                | TransportSessionState::IdleAffinity
                | TransportSessionState::Orphaned
        ) {
            return Err(RegistryError::InvalidTransition {
                from: record.logical.state,
                to: TransportSessionState::Active,
            });
        }
        let from = record.logical.state;
        let sequence = record.logical.next_invocation;
        record.logical.next_invocation = record
            .logical
            .next_invocation
            .checked_add(1)
            .ok_or(RegistryError::InvocationSequenceExhausted)?;
        record.logical.invocation = Some(InvocationRecord { sequence, deadline });
        record.logical.state = TransportSessionState::Active;
        record.logical.state_since = now;
        record.logical.state_deadline = record.logical.absolute_deadline;
        if from != TransportSessionState::Active {
            self.push_event(LifecycleEvent::StateChanged {
                fence: fence.clone(),
                from,
                to: TransportSessionState::Active,
                at: now,
            });
        }
        Ok(InvocationLease::new(fence.clone(), sequence, deadline))
    }

    pub fn finish_invocation(
        &mut self,
        lease: &InvocationLease,
        completion: InvocationCompletion,
    ) -> Result<(), RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        let target = completion.state();
        let record = self.record_mut(&lease.fence)?;
        let Some(invocation) = record.logical.invocation.as_ref() else {
            return Err(RegistryError::NoInflight);
        };
        if invocation.sequence != lease.sequence() || invocation.deadline != lease.deadline() {
            return Err(RegistryError::StaleInvocation);
        }
        record.logical.invocation = None;
        // A failed invocation retires a draining physical connection. Other
        // concurrent close/orphan directives still own their terminal state.
        if record.logical.state == TransportSessionState::Draining
            && target == TransportSessionState::Faulted
        {
            return self.set_state(&lease.fence, target, now);
        }
        // A soft-drain or close directive wins over a concurrently finishing invocation.
        if matches!(
            record.logical.state,
            TransportSessionState::Draining
                | TransportSessionState::Faulted
                | TransportSessionState::Closing
                | TransportSessionState::Orphaned
        ) {
            return Ok(());
        }
        if !valid_transition(record.logical.state, target) && record.logical.state != target {
            return Err(RegistryError::InvalidTransition {
                from: record.logical.state,
                to: target,
            });
        }
        self.set_state(&lease.fence, target, now)
    }

    pub fn renew_state_lease(&mut self, fence: &TransportFence) -> Result<(), RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        let config = self.config.clone();
        let record = self.record_mut(fence)?;
        record.logical.state_deadline = capped_state_deadline(&config, record, now);
        Ok(())
    }

    /// Allocates a fresh physical generation for the same logical session.
    ///
    /// The previous fence immediately becomes stale, preventing delayed events from mutating the
    /// new generation (including a close/reopen ABA sequence).
    pub fn rotate_generation(
        &mut self,
        fence: &TransportFence,
    ) -> Result<TransportFence, RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        let record = self.record(fence)?;
        if record.logical.invocation.is_some() {
            return Err(RegistryError::InflightExists);
        }
        if matches!(
            record.logical.state,
            TransportSessionState::Orphaned | TransportSessionState::Closing
        ) || (record.logical.state == TransportSessionState::Faulted
            && record.physical.close_acknowledged != Some(true))
        {
            return Err(RegistryError::InvalidTransition {
                from: record.logical.state,
                to: TransportSessionState::Opening,
            });
        }
        let generation = self.allocate_generation()?;
        let config = self.config.clone();
        let record = self.record_mut(fence)?;
        let from = record.logical.state;
        record.physical.generation = generation;
        record.physical.created_at = now;
        record.physical.close_acknowledged = None;
        record.logical.state = TransportSessionState::Opening;
        record.logical.state_since = now;
        record.logical.state_deadline = record.logical.absolute_deadline;
        record.physical.hard_deadline = now.saturating_add(config.physical_max_age);
        record.physical.soft_deadline = now
            .saturating_add(config.physical_soft_drain_age)
            .min(record.physical.hard_deadline);
        let next = TransportFence {
            session_id: fence.session_id.clone(),
            generation,
        };
        self.push_event(LifecycleEvent::StateChanged {
            fence: next.clone(),
            from,
            to: TransportSessionState::Opening,
            at: now,
        });
        Ok(next)
    }

    pub fn terminate(
        &mut self,
        fence: &TransportFence,
        kind: TerminationKind,
    ) -> Result<TerminationReceipt, RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        self.record(fence)?;
        Ok(self
            .terminate_by_id(&fence.session_id, kind, now)
            .expect("validated session must be removable"))
    }

    /// Applies deadline expiry and soft-drain transitions at the injected clock's current time.
    pub fn maintain(&mut self) {
        let now = self.clock.now();
        self.maintain_at(now);
    }

    pub fn drain_events(&mut self) -> Vec<LifecycleEvent> {
        self.events.drain(..).collect()
    }

    pub fn tombstone(&self, session_id: &TransportSessionId) -> Option<&TerminationReceipt> {
        self.tombstones
            .iter()
            .rev()
            .find(|receipt| receipt.fence.session_id == *session_id)
    }

    pub fn record_close_acknowledgement(
        &mut self,
        fence: &TransportFence,
        acknowledged: bool,
    ) -> Result<(), RegistryError> {
        // Faulted physical generations retain the logical record and its fixed
        // deadline. Their close ACK authorizes rotation, never logical readmission.
        if let Some(record) = self.sessions.get_mut(&fence.session_id) {
            ensure_generation(record, fence)?;
            if record.logical.state == TransportSessionState::Faulted {
                if record.physical.close_acknowledged != Some(true) {
                    record.physical.close_acknowledged = Some(acknowledged);
                }
                return Ok(());
            }
        }
        let receipt = self
            .tombstones
            .iter_mut()
            .rev()
            .find(|receipt| receipt.fence == *fence)
            .ok_or(RegistryError::NotFound)?;
        receipt.close_acknowledged = Some(acknowledged);
        Ok(())
    }

    pub fn fence(&self, session_id: &TransportSessionId) -> Option<TransportFence> {
        self.sessions.get(session_id).map(|record| TransportFence {
            session_id: session_id.clone(),
            generation: record.physical.generation,
        })
    }

    /// Classifies a fence without changing logical or physical session state.
    pub fn fence_status(&self, fence: &TransportFence) -> TransportFenceStatus {
        let Some(record) = self.sessions.get(&fence.session_id) else {
            return TransportFenceStatus::Missing;
        };
        if record.physical.generation == fence.generation {
            TransportFenceStatus::Current
        } else {
            TransportFenceStatus::Stale {
                current: record.physical.generation,
                received: fence.generation,
            }
        }
    }

    pub fn state(&self, fence: &TransportFence) -> Result<TransportSessionState, RegistryError> {
        Ok(self.record(fence)?.logical.state)
    }

    pub fn runtime_target_id(
        &self,
        fence: &TransportFence,
    ) -> Result<&TransportRuntimeTargetId, RegistryError> {
        Ok(&self.record(fence)?.physical.runtime_target_id)
    }

    pub fn physical_hard_deadline(
        &self,
        fence: &TransportFence,
    ) -> Result<TransportDeadline, RegistryError> {
        Ok(self.record(fence)?.physical.hard_deadline)
    }

    pub fn mark_owner_orphaned(&mut self, owner_id: &TransportOwnerId) -> Vec<TransportFence> {
        let now = self.clock.now();
        self.maintain_at(now);
        let fences = self
            .sessions
            .iter()
            .filter(|(_, record)| {
                &record.logical.owner_id == owner_id
                    && matches!(
                        record.logical.state,
                        TransportSessionState::Opening
                            | TransportSessionState::Active
                            | TransportSessionState::WaitingTool
                            | TransportSessionState::IdleAffinity
                    )
            })
            .map(|(session_id, record)| TransportFence {
                session_id: session_id.clone(),
                generation: record.physical.generation,
            })
            .collect::<Vec<_>>();
        for fence in &fences {
            let _ = self.set_state(fence, TransportSessionState::Orphaned, now);
        }
        fences
    }

    pub fn terminate_all(&mut self, kind: TerminationKind) -> Vec<TerminationReceipt> {
        let now = self.clock.now();
        let ids = self.sessions.keys().cloned().collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| self.terminate_by_id(&id, kind, now))
            .collect()
    }

    pub fn safe_snapshot(&self) -> SafeRegistrySnapshot {
        let now = self.clock.now();
        let sessions = self
            .sessions
            .iter()
            .map(|(session_id, record)| SafeSessionSnapshot {
                fence: TransportFence {
                    session_id: session_id.clone(),
                    generation: record.physical.generation,
                },
                owner_id: record.logical.owner_id.clone(),
                provider_id: record.logical.provider_id.clone(),
                runtime_target_id: record.physical.runtime_target_id.clone(),
                state: record.logical.state,
                inflight: record.logical.invocation.is_some(),
                age: now.saturating_duration_since(record.logical.created_at),
                state_age: now.saturating_duration_since(record.logical.state_since),
                state_ttl: record.logical.state_deadline.saturating_duration_since(now),
                logical_ttl: record
                    .logical
                    .absolute_deadline
                    .saturating_duration_since(now),
                invocation_deadline: record
                    .logical
                    .invocation
                    .as_ref()
                    .map(|invocation| invocation.deadline),
                invocation_ttl: record
                    .logical
                    .invocation
                    .as_ref()
                    .map(|invocation| invocation.deadline.saturating_duration_since(now)),
                physical_ttl: record.physical.hard_deadline.saturating_duration_since(now),
                deadline_kind: effective_deadline(record).1,
                eviction_priority: eviction_rank(record.logical.state),
            })
            .collect();
        SafeRegistrySnapshot {
            observed_at: now,
            capacity: self.config.capacity,
            tombstone_ttl: self.config.tombstone_ttl,
            sessions,
            tombstones: self.tombstones.iter().cloned().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    fn maintain_at(&mut self, now: TransportInstant) {
        while self.tombstones.front().is_some_and(|receipt| {
            now.saturating_duration_since(receipt.terminated_at) >= self.config.tombstone_ttl
        }) {
            self.tombstones.pop_front();
        }
        let expired: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(id, record)| expired_kind(record, now).map(|kind| (id.clone(), kind)))
            .collect();
        for (session_id, kind) in expired {
            self.terminate_by_id(&session_id, kind, now);
        }

        let drain: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, record)| {
                now >= record.physical.soft_deadline
                    && !matches!(
                        record.logical.state,
                        TransportSessionState::Draining
                            | TransportSessionState::Faulted
                            | TransportSessionState::Closing
                    )
            })
            .map(|(id, record)| TransportFence {
                session_id: id.clone(),
                generation: record.physical.generation,
            })
            .collect();
        for fence in drain {
            let _ = self.set_state(&fence, TransportSessionState::Draining, now);
        }
    }

    fn eviction_candidate(&self) -> Option<TransportSessionId> {
        self.sessions
            .iter()
            .filter_map(|(id, record)| {
                eviction_rank(record.logical.state).map(|rank| {
                    (
                        rank,
                        record.logical.state_since,
                        id.clone(),
                        record.physical.generation,
                    )
                })
            })
            .min()
            .map(|(_, _, id, _)| id)
    }

    fn set_state(
        &mut self,
        fence: &TransportFence,
        to: TransportSessionState,
        now: TransportInstant,
    ) -> Result<(), RegistryError> {
        let config = self.config.clone();
        let record = self.record_mut(fence)?;
        let from = record.logical.state;
        record.logical.state = to;
        record.logical.state_since = now;
        record.logical.state_deadline = capped_state_deadline(&config, record, now);
        if from != to {
            self.push_event(LifecycleEvent::StateChanged {
                fence: fence.clone(),
                from,
                to,
                at: now,
            });
        }
        Ok(())
    }

    fn terminate_by_id(
        &mut self,
        session_id: &TransportSessionId,
        kind: TerminationKind,
        now: TransportInstant,
    ) -> Option<TerminationReceipt> {
        let record = self.sessions.remove(session_id)?;
        let receipt = TerminationReceipt {
            fence: TransportFence {
                session_id: session_id.clone(),
                generation: record.physical.generation,
            },
            owner_id: record.logical.owner_id,
            provider_id: record.logical.provider_id,
            runtime_target_id: record.physical.runtime_target_id,
            previous_state: record.logical.state,
            kind,
            terminated_at: now,
            connection_age: now.saturating_duration_since(record.physical.created_at),
            close_acknowledged: None,
        };
        self.tombstones.push_back(receipt.clone());
        while self.tombstones.len() > self.config.tombstone_capacity {
            self.tombstones.pop_front();
        }
        self.push_event(LifecycleEvent::Terminated(receipt.clone()));
        Some(receipt)
    }

    fn push_event(&mut self, event: LifecycleEvent) {
        self.events.push_back(event);
        while self.events.len() > self.config.event_capacity {
            self.events.pop_front();
        }
    }

    fn record(&self, fence: &TransportFence) -> Result<&SessionRecord, RegistryError> {
        let record = self
            .sessions
            .get(&fence.session_id)
            .ok_or(RegistryError::NotFound)?;
        ensure_generation(record, fence)?;
        Ok(record)
    }

    fn record_mut(&mut self, fence: &TransportFence) -> Result<&mut SessionRecord, RegistryError> {
        let record = self
            .sessions
            .get_mut(&fence.session_id)
            .ok_or(RegistryError::NotFound)?;
        ensure_generation(record, fence)?;
        Ok(record)
    }

    fn allocate_generation(&mut self) -> Result<TransportGeneration, RegistryError> {
        let generation = TransportGeneration::new(self.next_generation);
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(RegistryError::GenerationExhausted)?;
        Ok(generation)
    }
}

fn ensure_generation(record: &SessionRecord, fence: &TransportFence) -> Result<(), RegistryError> {
    if record.physical.generation != fence.generation {
        return Err(RegistryError::StaleGeneration {
            expected: record.physical.generation,
            received: fence.generation,
        });
    }
    Ok(())
}

fn valid_transition(from: TransportSessionState, to: TransportSessionState) -> bool {
    use TransportSessionState as State;
    matches!(
        (from, to),
        (
            State::Opening,
            State::Active | State::Orphaned | State::Faulted | State::Closing
        ) | (
            State::Active,
            State::WaitingTool
                | State::IdleAffinity
                | State::Orphaned
                | State::Draining
                | State::Faulted
                | State::Closing
        ) | (
            State::WaitingTool | State::IdleAffinity | State::Orphaned,
            State::Active | State::Orphaned | State::Draining | State::Faulted | State::Closing
        ) | (State::Draining, State::Faulted | State::Closing)
            | (State::Faulted, State::Closing)
    )
}

fn capped_state_deadline(
    config: &TransportRegistryConfig,
    record: &SessionRecord,
    now: TransportInstant,
) -> TransportDeadline {
    let lease = match record.logical.state {
        TransportSessionState::Opening | TransportSessionState::Active => {
            return record.logical.absolute_deadline;
        }
        TransportSessionState::WaitingTool => config.waiting_tool_lease,
        TransportSessionState::IdleAffinity => config.idle_affinity_lease,
        TransportSessionState::Orphaned => config.orphan_grace,
        TransportSessionState::Draining => {
            record.physical.hard_deadline.saturating_duration_since(now)
        }
        TransportSessionState::Faulted => config.fault_grace,
        TransportSessionState::Closing => config.closing_grace,
    };
    now.saturating_add(lease)
        .min(record.logical.absolute_deadline)
}

fn effective_deadline(record: &SessionRecord) -> (TransportDeadline, DeadlineKind) {
    [
        (record.physical.hard_deadline, DeadlineKind::PhysicalHard),
        (
            record.logical.absolute_deadline,
            DeadlineKind::LogicalAbsolute,
        ),
        (record.logical.state_deadline, DeadlineKind::StateLease),
    ]
    .into_iter()
    .min_by_key(|(deadline, kind)| (*deadline, deadline_priority(*kind)))
    .expect("transport sessions always have a deadline")
}

fn expired_kind(record: &SessionRecord, now: TransportInstant) -> Option<TerminationKind> {
    let candidates = [
        (record.physical.hard_deadline, DeadlineKind::PhysicalHard),
        (
            record.logical.absolute_deadline,
            DeadlineKind::LogicalAbsolute,
        ),
        (record.logical.state_deadline, DeadlineKind::StateLease),
    ];
    candidates
        .into_iter()
        .filter(|(deadline, _)| now >= *deadline)
        .min_by_key(|(deadline, kind)| (*deadline, deadline_priority(*kind)))
        .map(|(_, kind)| {
            if kind == DeadlineKind::PhysicalHard {
                TerminationKind::ProviderHardMax
            } else if record.logical.state == TransportSessionState::Orphaned {
                TerminationKind::OwnerOrphaned
            } else {
                TerminationKind::DeadlineExceeded(kind)
            }
        })
}

fn deadline_priority(kind: DeadlineKind) -> u8 {
    match kind {
        DeadlineKind::PhysicalHard => 0,
        DeadlineKind::Task => 1,
        DeadlineKind::LogicalAbsolute => 2,
        DeadlineKind::StateLease => 3,
    }
}

fn eviction_rank(state: TransportSessionState) -> Option<u8> {
    match state {
        TransportSessionState::Orphaned => Some(0),
        TransportSessionState::IdleAffinity => Some(1),
        TransportSessionState::Draining => Some(2),
        TransportSessionState::WaitingTool => Some(3),
        TransportSessionState::Opening
        | TransportSessionState::Active
        | TransportSessionState::Faulted
        | TransportSessionState::Closing => None,
    }
}
