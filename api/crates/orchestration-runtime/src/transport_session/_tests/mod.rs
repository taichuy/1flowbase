use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use super::{
    AdmissionRequest, DeadlineKind, InvocationCompletion, LifecycleEvent, RegistryError,
    TerminationKind, TransportClock, TransportInstant, TransportOwnerId, TransportProviderId,
    TransportRegistryConfig, TransportRuntimeTargetId, TransportSessionId,
    TransportSessionRegistry, TransportSessionState,
};

#[derive(Clone, Default)]
struct FakeClock(Arc<AtomicU64>);

impl FakeClock {
    fn advance(&self, duration: Duration) {
        self.0.fetch_add(
            u64::try_from(duration.as_millis()).unwrap(),
            Ordering::SeqCst,
        );
    }
}

impl TransportClock for FakeClock {
    fn now(&self) -> TransportInstant {
        TransportInstant::from_millis(self.0.load(Ordering::SeqCst))
    }
}

fn config(capacity: usize) -> TransportRegistryConfig {
    TransportRegistryConfig {
        capacity,
        tombstone_capacity: 4,
        tombstone_ttl: Duration::from_secs(300),
        event_capacity: 64,
        logical_max_age: Duration::from_secs(200),
        invocation_default: Duration::from_secs(100),
        waiting_tool_lease: Duration::from_secs(55),
        orphan_grace: Duration::from_secs(6),
        idle_affinity_lease: Duration::from_secs(9),
        physical_soft_drain_age: Duration::from_secs(80),
        physical_max_age: Duration::from_secs(90),
        fault_grace: Duration::from_secs(5),
        closing_grace: Duration::from_secs(2),
    }
}

fn session_id(value: &str) -> TransportSessionId {
    TransportSessionId::new(value).unwrap()
}

fn request(value: &str) -> AdmissionRequest {
    AdmissionRequest {
        session_id: session_id(value),
        owner_id: TransportOwnerId::new(format!("owner-{value}")).unwrap(),
        provider_id: TransportProviderId::new(format!("provider-{value}")).unwrap(),
        runtime_target_id: TransportRuntimeTargetId::new(format!("target-{value}")).unwrap(),
        task_deadline: None,
        provider_hard_deadline: None,
    }
}

#[test]
fn fsm_and_single_flight_are_owned_by_the_registry() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(2)).unwrap();
    let fence = registry.admit(request("one")).unwrap();
    registry.activate(&fence).unwrap();

    let invocation = registry.begin_invocation(&fence).unwrap();
    assert_eq!(
        registry.begin_invocation(&fence),
        Err(RegistryError::InflightExists)
    );
    registry
        .finish_invocation(&invocation, InvocationCompletion::WaitingTool)
        .unwrap();
    assert_eq!(
        registry.safe_snapshot().sessions[0].state,
        TransportSessionState::WaitingTool
    );

    let resumed = registry.begin_invocation(&fence).unwrap();
    assert!(resumed.sequence() > invocation.sequence());
    registry
        .finish_invocation(&resumed, InvocationCompletion::IdleAffinity)
        .unwrap();
    assert_eq!(
        registry.safe_snapshot().sessions[0].state,
        TransportSessionState::IdleAffinity
    );
}

#[test]
fn generation_fence_rejects_delayed_events_and_close_reopen_aba() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(2)).unwrap();
    let original = registry.admit(request("same-logical-id")).unwrap();
    registry.activate(&original).unwrap();
    let replacement = registry.rotate_generation(&original).unwrap();
    assert!(replacement.generation.get() > original.generation.get());
    assert!(matches!(
        registry.activate(&original),
        Err(RegistryError::StaleGeneration { .. })
    ));

    registry.activate(&replacement).unwrap();
    registry
        .terminate(&replacement, TerminationKind::OwnerClosed)
        .unwrap();
    let reopened = registry.admit(request("same-logical-id")).unwrap();
    assert!(reopened.generation.get() > replacement.generation.get());
    assert!(matches!(
        registry.activate(&replacement),
        Err(RegistryError::StaleGeneration { .. }) | Err(RegistryError::NotFound)
    ));
}

#[test]
fn waiting_renewal_is_capped_by_task_and_logical_deadlines() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let mut admission = request("bounded");
    admission.task_deadline = Some(TransportInstant::from_millis(20_000));
    let fence = registry.admit(admission).unwrap();
    registry.activate(&fence).unwrap();
    registry
        .transition(&fence, TransportSessionState::WaitingTool)
        .unwrap();

    clock.advance(Duration::from_secs(19));
    registry.renew_state_lease(&fence).unwrap();
    assert_eq!(
        registry.safe_snapshot().sessions[0].logical_ttl,
        Duration::from_secs(1)
    );
    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert!(registry.is_empty());
    assert_eq!(
        registry.tombstone(&session_id("bounded")).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::Task)
    );
}

#[test]
fn every_non_terminal_state_has_a_finite_lease() {
    let states = [
        InvocationCompletion::WaitingTool,
        InvocationCompletion::IdleAffinity,
        InvocationCompletion::Orphaned,
        InvocationCompletion::Faulted,
        InvocationCompletion::Closing,
    ];
    for (index, completion) in states.into_iter().enumerate() {
        let clock = FakeClock::default();
        let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
        let fence = registry.admit(request(&format!("state-{index}"))).unwrap();
        registry.activate(&fence).unwrap();
        let lease = registry.begin_invocation(&fence).unwrap();
        registry.finish_invocation(&lease, completion).unwrap();
        clock.advance(Duration::from_secs(56));
        registry.maintain();
        assert!(registry.is_empty(), "{completion:?} must expire");
    }
}

#[test]
fn physical_soft_and_hard_deadlines_drain_then_terminate() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.invocation_default = Duration::from_secs(150);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let fence = registry.admit(request("physical")).unwrap();
    registry.activate(&fence).unwrap();

    clock.advance(Duration::from_secs(80));
    registry.maintain();
    assert_eq!(
        registry.safe_snapshot().sessions[0].state,
        TransportSessionState::Draining
    );
    clock.advance(Duration::from_secs(10));
    registry.maintain();
    assert_eq!(
        registry.tombstone(&session_id("physical")).unwrap().kind,
        TerminationKind::ProviderHardMax
    );
}

#[test]
fn capacity_eviction_uses_the_stable_policy_order() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(5)).unwrap();
    let states = [
        ("orphan", TransportSessionState::Orphaned),
        ("idle", TransportSessionState::IdleAffinity),
        ("drain", TransportSessionState::Draining),
        ("waiting", TransportSessionState::WaitingTool),
        ("active", TransportSessionState::Active),
    ];
    for (id, state) in states {
        let fence = registry.admit(request(id)).unwrap();
        registry.activate(&fence).unwrap();
        if state != TransportSessionState::Active {
            registry.transition(&fence, state).unwrap();
        }
    }

    for (new_id, evicted_id) in [
        ("new-1", "orphan"),
        ("new-2", "idle"),
        ("new-3", "drain"),
        ("new-4", "waiting"),
    ] {
        registry.admit(request(new_id)).unwrap();
        assert_eq!(
            registry.tombstone(&session_id(evicted_id)).unwrap().kind,
            TerminationKind::CapacityEvicted
        );
    }
    assert!(matches!(
        registry.admit(request("rejected")),
        Err(RegistryError::Capacity(_))
    ));
    assert!(registry.tombstone(&session_id("active")).is_none());
}

#[test]
fn capacity_tie_break_is_deterministic() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(2)).unwrap();
    for id in ["orphan-b", "orphan-a"] {
        let fence = registry.admit(request(id)).unwrap();
        registry.activate(&fence).unwrap();
        registry
            .transition(&fence, TransportSessionState::Orphaned)
            .unwrap();
    }
    registry.admit(request("new")).unwrap();
    assert!(registry.tombstone(&session_id("orphan-a")).is_some());
    assert!(registry.tombstone(&session_id("orphan-b")).is_none());
}

#[test]
fn expired_sessions_are_reaped_before_live_eviction() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(2)).unwrap();
    let mut expiring = request("expired");
    expiring.task_deadline = Some(TransportInstant::from_millis(1_000));
    registry.admit(expiring).unwrap();
    let live = registry.admit(request("live-idle")).unwrap();
    registry.activate(&live).unwrap();
    registry
        .transition(&live, TransportSessionState::IdleAffinity)
        .unwrap();

    clock.advance(Duration::from_secs(1));
    registry.admit(request("new")).unwrap();
    assert_eq!(
        registry.tombstone(&session_id("expired")).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::Task)
    );
    assert!(registry.tombstone(&session_id("live-idle")).is_none());
}

#[test]
fn only_active_sessions_cause_admission_rejection_without_eviction() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let fence = registry.admit(request("active")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry.begin_invocation(&fence).unwrap();

    let Err(RegistryError::Capacity(rejection)) = registry.admit(request("other")) else {
        panic!("ordinary capacity pressure must reject while the sole session is active");
    };
    assert_eq!(rejection.active, 1);
    assert!(registry.tombstone(&session_id("active")).is_none());
    registry
        .finish_invocation(&lease, InvocationCompletion::Active)
        .unwrap();
}

#[test]
fn tombstones_and_events_are_bounded_and_snapshot_is_metadata_only() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.tombstone_capacity = 2;
    settings.event_capacity = 2;
    let mut registry = TransportSessionRegistry::new(clock, settings).unwrap();
    for id in ["one", "two", "three"] {
        let fence = registry.admit(request(id)).unwrap();
        registry
            .terminate(&fence, TerminationKind::OwnerClosed)
            .unwrap();
    }

    let snapshot = registry.safe_snapshot();
    assert_eq!(snapshot.tombstones.len(), 2);
    assert_eq!(snapshot.tombstones[0].fence.session_id.as_str(), "two");
    assert_eq!(snapshot.tombstones[1].fence.session_id.as_str(), "three");
    assert_eq!(registry.drain_events().len(), 2);
    assert!(registry.tombstone(&session_id("one")).is_none());
    assert!(matches!(
        registry.tombstone(&session_id("three")),
        Some(receipt) if receipt.owner_id.as_str() == "owner-three"
    ));
}

#[test]
fn termination_event_preserves_the_same_stable_receipt_as_the_tombstone() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let fence = registry.admit(request("receipt")).unwrap();
    let receipt = registry
        .terminate(&fence, TerminationKind::Shutdown)
        .unwrap();
    let event = registry
        .drain_events()
        .into_iter()
        .find_map(|event| match event {
            LifecycleEvent::Terminated(receipt) => Some(receipt),
            LifecycleEvent::StateChanged { .. } => None,
        })
        .unwrap();
    assert_eq!(event, receipt);
    assert_eq!(registry.tombstone(&session_id("receipt")), Some(&receipt));
    registry.record_close_acknowledgement(&fence, true).unwrap();
    assert_eq!(
        registry
            .tombstone(&session_id("receipt"))
            .unwrap()
            .close_acknowledged,
        Some(true)
    );
}

#[test]
fn owner_disconnect_marks_inflight_session_orphaned_and_finish_cannot_revive_it() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let admission = request("socket");
    let owner = admission.owner_id.clone();
    let fence = registry.admit(admission).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry.begin_invocation(&fence).unwrap();

    assert_eq!(registry.mark_owner_orphaned(&owner), vec![fence.clone()]);
    registry
        .finish_invocation(&lease, InvocationCompletion::IdleAffinity)
        .unwrap();

    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
}

#[test]
fn orphan_expiry_has_a_stable_typed_tombstone_until_ttl_cleanup() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.orphan_grace = Duration::from_secs(1);
    settings.tombstone_ttl = Duration::from_secs(2);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let admission = request("orphan-callback");
    let owner = admission.owner_id.clone();
    let session = admission.session_id.clone();
    registry.admit(admission).unwrap();
    registry.mark_owner_orphaned(&owner);

    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert_eq!(
        registry.tombstone(&session).unwrap().kind,
        TerminationKind::OwnerOrphaned
    );
    let snapshot = registry.safe_snapshot();
    assert_eq!(snapshot.tombstone_ttl, Duration::from_secs(2));
    assert_eq!(snapshot.tombstones.len(), 1);

    clock.advance(Duration::from_secs(2));
    registry.maintain();
    assert!(registry.tombstone(&session).is_none());
}
