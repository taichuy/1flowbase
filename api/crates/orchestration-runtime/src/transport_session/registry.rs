use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::types::{
    AdmissionRequest, CapacityRejection, DeadlineKind, InvocationCompletion, InvocationLease,
    LifecycleEvent, RegistryError, SafeRegistrySnapshot, SafeSessionSnapshot, TerminationKind,
    TerminationReceipt, TransportDeadline, TransportFence, TransportGeneration, TransportInstant,
    TransportOwnerId, TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId,
    TransportSessionState,
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

struct SessionRecord {
    owner_id: TransportOwnerId,
    runtime_target_id: TransportRuntimeTargetId,
    generation: TransportGeneration,
    state: TransportSessionState,
    created_at: TransportInstant,
    state_since: TransportInstant,
    task_deadline: TransportDeadline,
    logical_absolute_deadline: TransportDeadline,
    state_deadline: TransportDeadline,
    physical_soft_deadline: TransportDeadline,
    physical_hard_deadline: TransportDeadline,
    inflight: Option<u64>,
    next_invocation: u64,
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

        let task_deadline = request
            .task_deadline
            .unwrap_or_else(|| now.saturating_add(self.config.invocation_default));
        let logical_absolute_deadline = now.saturating_add(self.config.logical_max_age);
        let physical_max_deadline = now.saturating_add(self.config.physical_max_age);
        let physical_hard_deadline = request
            .provider_hard_deadline
            .map_or(physical_max_deadline, |deadline| {
                deadline.min(physical_max_deadline)
            });
        if task_deadline <= now || physical_hard_deadline <= now {
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
                        .filter(|record| record.state == TransportSessionState::Active)
                        .count(),
                }));
            }
        }

        let generation = self.allocate_generation()?;
        let state_deadline = task_deadline.min(logical_absolute_deadline);
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
                owner_id: request.owner_id,
                runtime_target_id: request.runtime_target_id,
                generation,
                state: TransportSessionState::Opening,
                created_at: now,
                state_since: now,
                task_deadline,
                logical_absolute_deadline,
                state_deadline,
                physical_soft_deadline,
                physical_hard_deadline,
                inflight: None,
                next_invocation: 1,
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
        let from = self.record(fence)?.state;
        if !valid_transition(from, to) {
            return Err(RegistryError::InvalidTransition { from, to });
        }
        if self.record(fence)?.inflight.is_some()
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
    ) -> Result<InvocationLease, RegistryError> {
        let now = self.clock.now();
        self.maintain_at(now);
        let record = self.record_mut(fence)?;
        if record.inflight.is_some() {
            return Err(RegistryError::InflightExists);
        }
        if !matches!(
            record.state,
            TransportSessionState::Active
                | TransportSessionState::WaitingTool
                | TransportSessionState::IdleAffinity
                | TransportSessionState::Orphaned
        ) {
            return Err(RegistryError::InvalidTransition {
                from: record.state,
                to: TransportSessionState::Active,
            });
        }
        let from = record.state;
        let sequence = record.next_invocation;
        record.next_invocation = record
            .next_invocation
            .checked_add(1)
            .ok_or(RegistryError::InvocationSequenceExhausted)?;
        record.inflight = Some(sequence);
        record.state = TransportSessionState::Active;
        record.state_since = now;
        record.state_deadline = record.task_deadline.min(record.logical_absolute_deadline);
        if from != TransportSessionState::Active {
            self.push_event(LifecycleEvent::StateChanged {
                fence: fence.clone(),
                from,
                to: TransportSessionState::Active,
                at: now,
            });
        }
        Ok(InvocationLease::new(fence.clone(), sequence))
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
        let Some(sequence) = record.inflight else {
            return Err(RegistryError::NoInflight);
        };
        if sequence != lease.sequence() {
            return Err(RegistryError::StaleInvocation);
        }
        record.inflight = None;
        // A soft-drain or close directive wins over a concurrently finishing invocation.
        if matches!(
            record.state,
            TransportSessionState::Draining
                | TransportSessionState::Faulted
                | TransportSessionState::Closing
                | TransportSessionState::Orphaned
        ) {
            return Ok(());
        }
        if !valid_transition(record.state, target) && record.state != target {
            return Err(RegistryError::InvalidTransition {
                from: record.state,
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
        record.state_deadline = capped_state_deadline(&config, record, now);
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
        if self.record(fence)?.inflight.is_some() {
            return Err(RegistryError::InflightExists);
        }
        let generation = self.allocate_generation()?;
        let config = self.config.clone();
        let record = self.record_mut(fence)?;
        let from = record.state;
        record.generation = generation;
        record.state = TransportSessionState::Opening;
        record.state_since = now;
        record.state_deadline = record.task_deadline.min(record.logical_absolute_deadline);
        record.physical_hard_deadline = now.saturating_add(config.physical_max_age);
        record.physical_soft_deadline = now
            .saturating_add(config.physical_soft_drain_age)
            .min(record.physical_hard_deadline);
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

    pub fn fence(&self, session_id: &TransportSessionId) -> Option<TransportFence> {
        self.sessions.get(session_id).map(|record| TransportFence {
            session_id: session_id.clone(),
            generation: record.generation,
        })
    }

    pub fn state(&self, fence: &TransportFence) -> Result<TransportSessionState, RegistryError> {
        Ok(self.record(fence)?.state)
    }

    pub fn runtime_target_id(
        &self,
        fence: &TransportFence,
    ) -> Result<&TransportRuntimeTargetId, RegistryError> {
        Ok(&self.record(fence)?.runtime_target_id)
    }

    pub fn physical_hard_deadline(
        &self,
        fence: &TransportFence,
    ) -> Result<TransportDeadline, RegistryError> {
        Ok(self.record(fence)?.physical_hard_deadline)
    }

    pub fn mark_owner_orphaned(&mut self, owner_id: &TransportOwnerId) -> Vec<TransportFence> {
        let now = self.clock.now();
        self.maintain_at(now);
        let fences = self
            .sessions
            .iter()
            .filter(|(_, record)| {
                &record.owner_id == owner_id
                    && matches!(
                        record.state,
                        TransportSessionState::Active
                            | TransportSessionState::WaitingTool
                            | TransportSessionState::IdleAffinity
                    )
            })
            .map(|(session_id, record)| TransportFence {
                session_id: session_id.clone(),
                generation: record.generation,
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
                    generation: record.generation,
                },
                owner_id: record.owner_id.clone(),
                state: record.state,
                inflight: record.inflight.is_some(),
                age: now.saturating_duration_since(record.created_at),
                state_age: now.saturating_duration_since(record.state_since),
                logical_ttl: effective_logical_deadline(record).saturating_duration_since(now),
                physical_ttl: record.physical_hard_deadline.saturating_duration_since(now),
            })
            .collect();
        SafeRegistrySnapshot {
            observed_at: now,
            capacity: self.config.capacity,
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
                now >= record.physical_soft_deadline
                    && !matches!(
                        record.state,
                        TransportSessionState::Draining
                            | TransportSessionState::Faulted
                            | TransportSessionState::Closing
                    )
            })
            .map(|(id, record)| TransportFence {
                session_id: id.clone(),
                generation: record.generation,
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
                eviction_rank(record.state)
                    .map(|rank| (rank, record.state_since, id.clone(), record.generation))
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
        let from = record.state;
        record.state = to;
        record.state_since = now;
        record.state_deadline = capped_state_deadline(&config, record, now);
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
                generation: record.generation,
            },
            owner_id: record.owner_id,
            runtime_target_id: record.runtime_target_id,
            previous_state: record.state,
            kind,
            terminated_at: now,
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
    if record.generation != fence.generation {
        return Err(RegistryError::StaleGeneration {
            expected: record.generation,
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
            State::Active | State::Faulted | State::Closing
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
    let lease = match record.state {
        TransportSessionState::Opening | TransportSessionState::Active => {
            return record.task_deadline.min(record.logical_absolute_deadline);
        }
        TransportSessionState::WaitingTool => config.waiting_tool_lease,
        TransportSessionState::IdleAffinity => config.idle_affinity_lease,
        TransportSessionState::Orphaned => config.orphan_grace,
        TransportSessionState::Draining => {
            record.physical_hard_deadline.saturating_duration_since(now)
        }
        TransportSessionState::Faulted => config.fault_grace,
        TransportSessionState::Closing => config.closing_grace,
    };
    now.saturating_add(lease)
        .min(record.task_deadline)
        .min(record.logical_absolute_deadline)
}

fn effective_logical_deadline(record: &SessionRecord) -> TransportDeadline {
    record
        .task_deadline
        .min(record.logical_absolute_deadline)
        .min(record.state_deadline)
}

fn expired_kind(record: &SessionRecord, now: TransportInstant) -> Option<TerminationKind> {
    let candidates = [
        (record.physical_hard_deadline, DeadlineKind::PhysicalHard),
        (record.task_deadline, DeadlineKind::Task),
        (
            record.logical_absolute_deadline,
            DeadlineKind::LogicalAbsolute,
        ),
        (record.state_deadline, DeadlineKind::StateLease),
    ];
    candidates
        .into_iter()
        .filter(|(deadline, _)| now >= *deadline)
        .min_by_key(|(deadline, kind)| (*deadline, deadline_priority(*kind)))
        .map(|(_, kind)| {
            if kind == DeadlineKind::PhysicalHard {
                TerminationKind::ProviderHardMax
            } else if record.state == TransportSessionState::Orphaned {
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
