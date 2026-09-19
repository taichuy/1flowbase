use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use super::{
    AdmissionRequest, DeadlineKind, InvocationCompletion, InvocationRequest, LifecycleEvent,
    RegistryError, TerminationKind, TransportClock, TransportFenceStatus, TransportInstant,
    TransportOwnerId, TransportProviderId, TransportRegistryConfig, TransportRuntimeTargetId,
    TransportSessionId, TransportSessionRegistry, TransportSessionState,
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
        provider_hard_deadline: None,
    }
}

fn invocation_request(deadline_millis: Option<u64>) -> InvocationRequest {
    InvocationRequest {
        deadline: deadline_millis.map(TransportInstant::from_millis),
    }
}

#[test]
fn fsm_and_single_flight_are_owned_by_the_registry() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(2)).unwrap();
    let fence = registry.admit(request("one")).unwrap();
    registry.activate(&fence).unwrap();

    let invocation = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    assert_eq!(
        registry.begin_invocation(&fence, invocation_request(None)),
        Err(RegistryError::InflightExists)
    );
    registry
        .finish_invocation(&invocation, InvocationCompletion::WaitingTool)
        .unwrap();
    assert_eq!(
        registry.safe_snapshot().sessions[0].state,
        TransportSessionState::WaitingTool
    );

    let resumed = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
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
    let owner = registry.safe_snapshot().sessions[0].owner_id.clone();
    registry.activate(&original).unwrap();
    let completed = registry
        .begin_invocation(&original, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&completed, InvocationCompletion::Active)
        .unwrap();
    let replacement = registry.rotate_generation(&original).unwrap();
    assert!(replacement.generation.get() > original.generation.get());
    assert_eq!(registry.safe_snapshot().sessions[0].owner_id, owner);
    let before_stale_observation = registry.safe_snapshot();
    assert!(matches!(
        registry.fence_status(&original),
        TransportFenceStatus::Stale { .. }
    ));
    assert_eq!(registry.safe_snapshot(), before_stale_observation);
    assert!(matches!(
        registry.activate(&original),
        Err(RegistryError::StaleGeneration { .. })
    ));
    assert!(matches!(
        registry.finish_invocation(&completed, InvocationCompletion::Active),
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
fn completed_invocation_deadline_does_not_bound_a_later_invocation() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.logical_max_age = Duration::from_secs(2 * 60 * 60);
    settings.invocation_default = Duration::from_secs(30 * 60);
    settings.waiting_tool_lease = Duration::from_secs(60 * 60);
    settings.physical_soft_drain_age = Duration::from_secs(80 * 60);
    settings.physical_max_age = Duration::from_secs(90 * 60);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let fence = registry.admit(request("long-tool-loop")).unwrap();
    registry.activate(&fence).unwrap();
    let first = registry
        .begin_invocation(&fence, invocation_request(Some(30 * 60 * 1_000)))
        .unwrap();
    registry
        .finish_invocation(&first, InvocationCompletion::WaitingTool)
        .unwrap();

    clock.advance(Duration::from_secs(30 * 60 + 1));
    registry.maintain();
    assert!(registry.tombstone(&session_id("long-tool-loop")).is_none());
    let second_deadline = 60 * 60 * 1_000;
    let second = registry
        .begin_invocation(&fence, invocation_request(Some(second_deadline)))
        .unwrap();
    assert!(second.sequence() > first.sequence());
    assert_eq!(second.deadline().as_millis(), second_deadline);
}

#[test]
fn invocation_deadline_is_visible_and_cleared_without_a_session_tombstone() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("invocation-only")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(Some(1_000)))
        .unwrap();
    let active = registry.safe_snapshot().sessions.remove(0);
    assert_eq!(active.invocation_deadline, Some(lease.deadline()));
    assert_eq!(active.invocation_ttl, Some(Duration::from_secs(1)));

    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert!(registry.tombstone(&session_id("invocation-only")).is_none());
    assert_eq!(
        registry.safe_snapshot().sessions[0].invocation_ttl,
        Some(Duration::ZERO)
    );
    registry
        .finish_invocation(&lease, InvocationCompletion::WaitingTool)
        .unwrap();
    let waiting = registry.safe_snapshot().sessions.remove(0);
    assert_eq!(waiting.invocation_deadline, None);
    assert_eq!(waiting.invocation_ttl, None);
    assert!(waiting.state_ttl > Duration::ZERO);
}

#[test]
fn waiting_renewal_is_capped_by_the_logical_deadline() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.logical_max_age = Duration::from_secs(20);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let fence = registry.admit(request("bounded")).unwrap();
    registry.activate(&fence).unwrap();
    registry
        .transition(&fence, TransportSessionState::WaitingTool)
        .unwrap();

    clock.advance(Duration::from_secs(19));
    registry.renew_state_lease(&fence).unwrap();
    let snapshot = registry.safe_snapshot();
    assert_eq!(snapshot.sessions[0].logical_ttl, Duration::from_secs(1));
    assert_eq!(snapshot.sessions[0].state_ttl, Duration::from_secs(1));
    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert!(registry.is_empty());
    assert_eq!(
        registry.tombstone(&session_id("bounded")).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
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
        let lease = registry
            .begin_invocation(&fence, invocation_request(None))
            .unwrap();
        registry.finish_invocation(&lease, completion).unwrap();
        clock.advance(Duration::from_secs(56));
        registry.maintain();
        if completion == InvocationCompletion::IdleAffinity {
            assert_eq!(
                registry.state(&fence).unwrap(),
                TransportSessionState::IdleReleased
            );
            clock.advance(Duration::from_secs(144));
            registry.maintain();
        }
        assert!(
            registry.is_empty(),
            "{completion:?} must have a finite logical lifetime"
        );
    }
}

#[test]
fn waiting_state_lease_expires_independently() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.waiting_tool_lease = Duration::from_secs(1);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let fence = registry.admit(request("waiting-deadline")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&lease, InvocationCompletion::WaitingTool)
        .unwrap();

    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert_eq!(
        registry
            .tombstone(&session_id("waiting-deadline"))
            .unwrap()
            .kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::StateLease)
    );
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
    let mut settings = config(2);
    settings.logical_max_age = Duration::from_secs(1);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    registry.admit(request("expired")).unwrap();
    clock.advance(Duration::from_millis(500));
    let live = registry.admit(request("live-idle")).unwrap();
    registry.activate(&live).unwrap();
    registry
        .transition(&live, TransportSessionState::IdleAffinity)
        .unwrap();

    clock.advance(Duration::from_millis(500));
    registry.admit(request("new")).unwrap();
    assert_eq!(
        registry.tombstone(&session_id("expired")).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
    );
    assert!(registry.tombstone(&session_id("live-idle")).is_none());
}

#[test]
fn only_active_sessions_cause_admission_rejection_without_eviction() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let fence = registry.admit(request("active")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();

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
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();

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
    let fence = registry.admit(admission).unwrap();
    assert_eq!(registry.mark_owner_orphaned(&owner), vec![fence.clone()]);
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );

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

#[test]
fn owner_disconnect_does_not_regress_terminal_path_states_to_orphaned() {
    for (index, terminal_state) in [
        TransportSessionState::Draining,
        TransportSessionState::Faulted,
        TransportSessionState::Closing,
    ]
    .into_iter()
    .enumerate()
    {
        let clock = FakeClock::default();
        let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
        let admission = request(&format!("terminal-{index}"));
        let owner = admission.owner_id.clone();
        let fence = registry.admit(admission).unwrap();
        registry.activate(&fence).unwrap();
        registry.transition(&fence, terminal_state).unwrap();

        assert!(registry.mark_owner_orphaned(&owner).is_empty());
        assert_eq!(registry.state(&fence).unwrap(), terminal_state);
    }
}

#[test]
fn physical_fault_rotation_requires_close_ack_and_preserves_logical_deadline_and_sequence() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let first = registry.admit(request("fault-successor")).unwrap();
    registry.activate(&first).unwrap();
    let failed = registry
        .begin_invocation(&first, invocation_request(Some(10_000)))
        .unwrap();
    registry
        .finish_invocation(&failed, InvocationCompletion::Faulted)
        .unwrap();
    assert!(matches!(
        registry.rotate_generation(&first),
        Err(RegistryError::InvalidTransition { .. })
    ));
    registry
        .record_close_acknowledgement(&first, false)
        .unwrap();
    assert!(registry.rotate_generation(&first).is_err());
    registry.record_close_acknowledgement(&first, true).unwrap();
    // A duplicate negative ACK cannot revoke an accepted close fact.
    registry
        .record_close_acknowledgement(&first, false)
        .unwrap();
    clock.advance(Duration::from_secs(1));
    let second = registry.rotate_generation(&first).unwrap();
    registry.activate(&second).unwrap();
    let next = registry
        .begin_invocation(&second, invocation_request(Some(60_000)))
        .unwrap();
    assert!(second.generation > first.generation);
    assert!(next.sequence() > failed.sequence());
    assert_eq!(next.deadline().as_millis(), 60_000);
    assert_eq!(
        registry.safe_snapshot().sessions[0].logical_ttl,
        Duration::from_secs(199)
    );
    let before = registry.safe_snapshot();
    assert!(matches!(
        registry.finish_invocation(&failed, InvocationCompletion::Faulted),
        Err(RegistryError::StaleGeneration { .. })
    ));
    assert!(matches!(
        registry.record_close_acknowledgement(&first, true),
        Err(RegistryError::StaleGeneration { .. })
    ));
    assert_eq!(registry.safe_snapshot(), before);
}

#[test]
fn faulted_invocation_is_terminal_and_expired_fault_lease_cannot_rotate() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("fault-expiry")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    assert_eq!(
        registry.rotate_generation(&fence),
        Err(RegistryError::InflightExists)
    );
    registry
        .finish_invocation(&lease, InvocationCompletion::Faulted)
        .unwrap();
    assert_eq!(
        registry.finish_invocation(&lease, InvocationCompletion::Active),
        Err(RegistryError::NoInflight)
    );
    registry.record_close_acknowledgement(&fence, true).unwrap();
    clock.advance(Duration::from_secs(5));
    assert_eq!(
        registry.rotate_generation(&fence),
        Err(RegistryError::NotFound)
    );
    assert_eq!(
        registry.tombstone(&fence.session_id).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::StateLease)
    );
}

#[test]
fn physical_fault_wins_over_pending_soft_drain_without_replaying_inflight_work() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("drain-fault")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    clock.advance(Duration::from_secs(80));
    registry.maintain();
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Draining
    );
    registry
        .finish_invocation(&lease, InvocationCompletion::Faulted)
        .unwrap();
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Faulted
    );
    assert!(!registry.safe_snapshot().sessions[0].inflight);
    assert!(registry.rotate_generation(&fence).is_err());
}

#[test]
fn idle_affinity_89_90_91_retains_logical_deadline_and_requires_close_ack() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.idle_affinity_lease = Duration::from_secs(90);
    settings.physical_soft_drain_age = Duration::from_secs(150);
    settings.physical_max_age = Duration::from_secs(180);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let fence = registry.admit(request("idle-release")).unwrap();
    registry.activate(&fence).unwrap();
    let first = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&first, InvocationCompletion::IdleAffinity)
        .unwrap();
    clock.advance(Duration::from_secs(89));
    registry.maintain();
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::IdleAffinity
    );
    registry.drain_events();
    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::IdleReleased
    );
    assert!(registry.tombstone(&fence.session_id).is_none());
    assert_eq!(registry.drain_events().len(), 1);
    assert!(registry.rotate_generation(&fence).is_err());
    assert!(registry
        .begin_invocation(&fence, invocation_request(None))
        .is_err());
    registry
        .record_close_acknowledgement(&fence, false)
        .unwrap();
    assert!(registry.rotate_generation(&fence).is_err());
    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert!(
        registry.drain_events().is_empty(),
        "release emits close only once"
    );
    registry.record_close_acknowledgement(&fence, true).unwrap();
    let next = registry.rotate_generation(&fence).unwrap();
    registry.activate(&next).unwrap();
    let second = registry
        .begin_invocation(&next, invocation_request(None))
        .unwrap();
    assert!(second.sequence() > first.sequence());
    assert!(next.generation > fence.generation);
    assert_eq!(
        registry.safe_snapshot().sessions[0].logical_ttl,
        Duration::from_secs(109)
    );
    assert!(registry
        .finish_invocation(&first, InvocationCompletion::IdleAffinity)
        .is_err());
    assert!(registry.record_close_acknowledgement(&fence, true).is_err());
    registry
        .finish_invocation(&second, InvocationCompletion::IdleAffinity)
        .unwrap();
    clock.advance(Duration::from_secs(109));
    registry.maintain();
    assert_eq!(
        registry.tombstone(&next.session_id).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
    );
}
