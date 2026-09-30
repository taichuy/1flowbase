use super::*;

#[test]
fn active_orphan_survives_grace_and_real_completion_owns_retention() {
    for completion in [
        InvocationCompletion::WaitingTool,
        InvocationCompletion::IdleAffinity,
        InvocationCompletion::Faulted,
    ] {
        let clock = FakeClock::default();
        let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
        let fence = registry.admit(request("active-orphan")).unwrap();
        registry.activate(&fence).unwrap();
        let lease = registry
            .begin_invocation(&fence, invocation_request(Some(70_000)))
            .unwrap();
        registry.mark_invocation_orphaned(&lease).unwrap();
        registry.drain_events();

        clock.advance(Duration::from_secs(7));
        registry.maintain();
        assert!(registry.tombstone(&fence.session_id).is_none());
        assert!(registry.drain_events().is_empty());
        assert_eq!(
            registry.state(&fence).unwrap(),
            TransportSessionState::Orphaned
        );
        assert_eq!(
            registry.invocation_deadline(&fence).unwrap(),
            Some(lease.deadline())
        );
        assert_eq!(
            registry.begin_invocation(&fence, invocation_request(Some(80_000))),
            Err(RegistryError::InflightExists)
        );
        assert_eq!(
            registry.rotate_generation(&fence),
            Err(RegistryError::InflightExists)
        );

        registry.finish_invocation(&lease, completion).unwrap();
        assert!(!registry.invocation_inflight(&fence).unwrap());
        assert_eq!(registry.invocation_deadline(&fence).unwrap(), None);
        if completion == InvocationCompletion::Faulted {
            assert_eq!(
                registry.state(&fence).unwrap(),
                TransportSessionState::Faulted
            );
            assert!(registry.rotate_generation(&fence).is_err());
            registry
                .record_closure_evidence(&fence, &released_evidence(&fence, 7))
                .unwrap();
            assert!(registry.rotate_generation(&fence).unwrap().generation > fence.generation);
        } else {
            assert_eq!(
                registry.state(&fence).unwrap(),
                TransportSessionState::Orphaned
            );
            let next = registry
                .begin_invocation(&fence, invocation_request(Some(80_000)))
                .unwrap();
            assert_eq!(next.fence, fence);
            assert_eq!(next.sequence(), lease.sequence() + 1);
        }
    }
}

#[test]
fn active_orphan_execution_deadline_still_expires_and_late_finish_cannot_revive() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
    let fence = registry.admit(request("orphan-deadline")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(Some(10_000)))
        .unwrap();
    registry.mark_invocation_orphaned(&lease).unwrap();
    clock.advance(Duration::from_secs(9));
    registry.maintain();
    assert!(registry.tombstone(&fence.session_id).is_none());
    clock.advance(Duration::from_secs(1));
    registry.maintain();
    assert_eq!(
        registry.tombstone(&fence.session_id).unwrap().kind,
        TerminationKind::OwnerOrphaned
    );
    assert_eq!(
        registry.finish_invocation(&lease, InvocationCompletion::WaitingTool),
        Err(RegistryError::NotFound)
    );
    assert_eq!(registry.state(&fence), Err(RegistryError::NotFound));
}

#[test]
fn deferred_delivery_expiry_cannot_mask_physical_hard_deadline() {
    for orphaned in [false, true] {
        let clock = FakeClock::default();
        let mut settings = config(1);
        settings.logical_max_age = Duration::from_secs(3);
        settings.orphan_grace = Duration::from_secs(1);
        settings.physical_soft_drain_age = Duration::from_secs(4);
        settings.physical_max_age = Duration::from_secs(5);
        let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
        let fence = registry.admit(request("physical-deadline")).unwrap();
        registry.activate(&fence).unwrap();
        let lease = registry
            .begin_invocation(&fence, invocation_request(Some(10_000)))
            .unwrap();
        if orphaned {
            registry.mark_invocation_orphaned(&lease).unwrap();
        }
        clock.advance(Duration::from_secs(3));
        registry.maintain();
        assert!(registry.tombstone(&fence.session_id).is_none());
        clock.advance(Duration::from_secs(2));
        registry.maintain();
        assert_eq!(
            registry.tombstone(&fence.session_id).unwrap().kind,
            TerminationKind::ProviderHardMax
        );
        assert_eq!(
            registry.finish_invocation(&lease, InvocationCompletion::Active),
            Err(RegistryError::NotFound)
        );
    }
}
