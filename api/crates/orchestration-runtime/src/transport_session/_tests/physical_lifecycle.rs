use super::*;

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
fn orphan_expiry_keeps_tombstone_until_physical_release() {
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
    assert!(
        registry.tombstone(&session).is_some(),
        "an orphaned generation stays fenced until physical release is proven"
    );
    registry
        .record_closure_evidence(&fence, &released_evidence(&fence, 7))
        .unwrap();
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
fn physical_fault_rotation_requires_release_evidence_and_preserves_logical_deadline_and_sequence() {
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
    assert!(
        registry.rotate_generation(&first).is_err(),
        "ACK alone is not local release proof"
    );
    registry
        .record_closure_evidence(&first, &released_evidence(&first, 7))
        .unwrap();
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
fn idle_affinity_89_90_91_retains_logical_deadline_and_requires_release_evidence() {
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
    assert!(
        registry.rotate_generation(&fence).is_err(),
        "ACK alone is not local release proof"
    );
    registry
        .record_closure_evidence(&fence, &released_evidence(&fence, 7))
        .unwrap();
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
    clock.advance(Duration::from_secs(90));
    registry.maintain();
    assert_eq!(
        registry.state(&next).unwrap(),
        TransportSessionState::IdleReleased
    );
    clock.advance(Duration::from_secs(19));
    registry.maintain();
    assert_eq!(
        registry.tombstone(&next.session_id).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
    );
}

#[test]
fn socket_orphan_requires_current_invocation_sequence_and_fence() {
    let mut registry = TransportSessionRegistry::new(FakeClock::default(), config(1)).unwrap();
    let fence = registry.admit(request("scoped")).unwrap();
    registry.activate(&fence).unwrap();
    let old = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&old, InvocationCompletion::IdleAffinity)
        .unwrap();
    assert_eq!(
        registry.mark_invocation_orphaned(&old),
        Err(RegistryError::NoInflight)
    );
    let current = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    assert_eq!(
        registry.mark_invocation_orphaned(&old),
        Err(RegistryError::StaleInvocation)
    );
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Active
    );
    registry.mark_invocation_orphaned(&current).unwrap();
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
}

#[test]
fn close_release_identity_is_fenced_and_does_not_relax_orphan_or_inflight() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let fence = registry.admit(request("close-isolation")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry.mark_invocation_orphaned(&lease).unwrap();
    let evidence = released_evidence(&fence, 7);
    registry.record_closure_evidence(&fence, &evidence).unwrap();
    assert!(registry.rotate_generation(&fence).is_err());
    registry
        .finish_invocation(&lease, InvocationCompletion::IdleAffinity)
        .unwrap();
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
    assert!(registry.rotate_generation(&fence).is_err());
    assert_eq!(
        registry.record_closure_evidence(&fence, &released_evidence(&fence, 8)),
        Err(RegistryError::InvalidClosureEvidence)
    );
    assert_eq!(
        registry.safe_snapshot().sessions[0].closure_evidence,
        Some(evidence)
    );
}

#[test]
fn close_release_late_evidence_cannot_modify_rotated_generation() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let first = registry.admit(request("close-stale")).unwrap();
    registry.activate(&first).unwrap();
    let lease = registry
        .begin_invocation(&first, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&lease, InvocationCompletion::Faulted)
        .unwrap();
    let evidence = released_evidence(&first, 7);
    registry.record_closure_evidence(&first, &evidence).unwrap();
    let next = registry.rotate_generation(&first).unwrap();
    assert!(registry.record_closure_evidence(&first, &evidence).is_err());
    assert!(registry.record_closure_evidence(&next, &evidence).is_err());
    assert!(registry.safe_snapshot().sessions[0]
        .closure_evidence
        .is_none());
}

/// The delivery-unbound fact must survive a completion, but it must not keep the
/// *in-flight* orphan grace, which would reap a session whose tool round is still
/// coming back. Retention follows the real completion instead.
#[test]
fn orphaned_completion_keeps_the_unbound_fact_and_uses_the_completion_lease() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("orphan-completion")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry.mark_invocation_orphaned(&lease).unwrap();
    assert!(registry.invocation_inflight(&fence).unwrap());
    let in_flight_ttl = registry.safe_snapshot().sessions[0].state_ttl;
    assert_eq!(in_flight_ttl, Duration::from_secs(6));

    registry
        .finish_invocation(&lease, InvocationCompletion::WaitingTool)
        .unwrap();

    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Orphaned,
        "unbinding the delivery is a preserved fact, not a state rewrite"
    );
    assert!(!registry.invocation_inflight(&fence).unwrap());
    assert_eq!(
        registry.safe_snapshot().sessions[0].state_ttl,
        Duration::from_secs(55),
        "a completed tool round is retained under the waiting-tool lease"
    );

    clock.advance(Duration::from_secs(6));
    registry.maintain();
    assert!(
        registry
            .tombstone(&session_id("orphan-completion"))
            .is_none(),
        "the in-flight orphan grace no longer reaps a completed call"
    );

    let successor = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    assert!(successor.sequence() > lease.sequence());
    assert_eq!(
        registry.state(&fence).unwrap(),
        TransportSessionState::Active
    );
    assert!(registry.invocation_inflight(&fence).unwrap());
}

#[test]
fn a_stale_lease_cannot_extend_another_invocations_orphan_retention() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("orphan-stale")).unwrap();
    registry.activate(&fence).unwrap();
    let stale = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&stale, InvocationCompletion::IdleAffinity)
        .unwrap();
    let current = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry.mark_invocation_orphaned(&current).unwrap();
    let before = registry.safe_snapshot();

    assert_eq!(
        registry.finish_invocation(&stale, InvocationCompletion::WaitingTool),
        Err(RegistryError::StaleInvocation)
    );
    assert!(registry.invocation_inflight(&fence).unwrap());
    assert_eq!(registry.safe_snapshot(), before);
}

#[test]
fn orphaned_failed_invocation_requires_physical_release_before_successor_generation() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock, config(1)).unwrap();
    let first = registry.admit(request("orphaned-prewarm-failure")).unwrap();
    registry.activate(&first).unwrap();
    let failed = registry
        .begin_invocation(&first, invocation_request(None))
        .unwrap();
    registry.mark_invocation_orphaned(&failed).unwrap();
    registry.drain_events();

    registry
        .finish_invocation(&failed, InvocationCompletion::Faulted)
        .unwrap();

    assert!(!registry.invocation_inflight(&first).unwrap());
    assert_eq!(
        registry.state(&first).unwrap(),
        TransportSessionState::Faulted
    );
    assert!(
        registry.drain_events().into_iter().any(|event| matches!(
            event,
            LifecycleEvent::StateChanged {
                fence,
                from: TransportSessionState::Orphaned,
                to: TransportSessionState::Faulted,
                ..
            } if fence == first
        )),
        "the coordinator must receive the fault transition that requests physical close"
    );
    assert!(
        registry
            .begin_invocation(&first, invocation_request(None))
            .is_err(),
        "a failed physical generation cannot accept another invocation"
    );
    assert!(
        registry.rotate_generation(&first).is_err(),
        "clearing inflight is not physical release evidence"
    );
    registry.record_close_acknowledgement(&first, true).unwrap();
    assert!(
        registry.rotate_generation(&first).is_err(),
        "a peer ACK alone is not proof of local release"
    );
    registry
        .record_closure_evidence(&first, &released_evidence(&first, 7))
        .unwrap();
    let next = registry.rotate_generation(&first).unwrap();
    assert!(next.generation > first.generation);
    registry.activate(&next).unwrap();
    let successor = registry
        .begin_invocation(&next, invocation_request(None))
        .unwrap();
    assert_eq!(successor.sequence(), failed.sequence() + 1);
    assert!(registry.invocation_inflight(&next).unwrap());
}

#[test]
fn confirmed_exit_cannot_settle_an_active_invocation_or_rewrite_a_known_incarnation() {
    let mut registry = TransportSessionRegistry::new(FakeClock::default(), config(1)).unwrap();
    let fence = registry.admit(request("exit-proof")).unwrap();
    registry.activate(&fence).unwrap();
    let invocation = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    let before = registry.safe_snapshot();
    assert_eq!(
        registry.record_worker_exit_evidence(&fence, &confirmed_worker_exit(&fence, 7)),
        Err(RegistryError::InflightExists)
    );
    assert_eq!(registry.safe_snapshot(), before);
    registry
        .finish_invocation(&invocation, InvocationCompletion::IdleAffinity)
        .unwrap();
    registry
        .record_worker_exit_evidence(&fence, &confirmed_worker_exit(&fence, 7))
        .unwrap();
    let released = registry.safe_snapshot();
    assert_eq!(released.sessions[0].state, TransportSessionState::Faulted);
    assert_eq!(
        registry.record_worker_exit_evidence(&fence, &confirmed_worker_exit(&fence, 8)),
        Err(RegistryError::InvalidClosureEvidence)
    );
    assert_eq!(registry.safe_snapshot(), released);
    let next = registry.rotate_generation(&fence).unwrap();
    registry.activate(&next).unwrap();
    let current = registry.safe_snapshot();
    assert!(registry
        .record_worker_exit_evidence(&fence, &confirmed_worker_exit(&fence, 7))
        .is_err());
    assert!(registry
        .record_worker_exit_evidence(&next, &confirmed_worker_exit(&fence, 7))
        .is_err());
    assert_eq!(registry.safe_snapshot(), current);
}

#[test]
fn released_fault_delayed_successor_ignores_old_fault_deadline() {
    assert_released_fault_delayed_successor(6);
}

#[test]
fn released_fault_delayed_successor_ignores_old_physical_deadline() {
    assert_released_fault_delayed_successor(91);
}

#[test]
fn released_fault_still_expires_at_original_logical_absolute_deadline() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("released-fault-expiry")).unwrap();
    registry.activate(&fence).unwrap();
    registry
        .record_worker_exit_evidence(&fence, &confirmed_worker_exit(&fence, 7))
        .unwrap();
    clock.advance(Duration::from_secs(199));
    registry.maintain();
    assert_eq!(registry.state(&fence), Ok(TransportSessionState::Faulted));
    assert_eq!(
        registry.safe_snapshot().sessions[0].logical_ttl,
        Duration::from_secs(1)
    );
    clock.advance(Duration::from_secs(1));
    assert_eq!(
        registry.rotate_generation(&fence),
        Err(RegistryError::NotFound)
    );
    assert_eq!(
        registry.tombstone(&fence.session_id).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
    );
}

#[test]
fn released_fault_with_inflight_invocation_still_expires_and_cannot_revive() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("released-fault-inflight")).unwrap();
    registry.activate(&fence).unwrap();
    let invocation = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .transition(&fence, TransportSessionState::Faulted)
        .unwrap();
    registry
        .record_closure_evidence(&fence, &released_evidence(&fence, 7))
        .unwrap();
    assert_eq!(
        registry.rotate_generation(&fence),
        Err(RegistryError::InflightExists)
    );
    clock.advance(Duration::from_secs(6));
    assert_eq!(
        registry.rotate_generation(&fence),
        Err(RegistryError::NotFound)
    );
    assert_eq!(
        registry.tombstone(&fence.session_id).unwrap().kind,
        TerminationKind::DeadlineExceeded(DeadlineKind::StateLease)
    );
    assert_eq!(
        registry.finish_invocation(&invocation, InvocationCompletion::Active),
        Err(RegistryError::NotFound)
    );
    assert_eq!(registry.state(&fence), Err(RegistryError::NotFound));
}
