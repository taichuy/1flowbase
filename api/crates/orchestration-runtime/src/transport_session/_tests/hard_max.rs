use super::*;

#[test]
fn hard_max_tombstones_outlive_diagnostic_ttl_until_local_release() {
    for kind in [
        TerminationKind::ProviderHardMax,
        TerminationKind::DeadlineExceeded(DeadlineKind::PhysicalHard),
    ] {
        let clock = FakeClock::default();
        let mut registry = TransportSessionRegistry::new(clock.clone(), config(1)).unwrap();
        let old = registry.admit(request("hard-max-retention")).unwrap();
        registry.activate(&old).unwrap();
        let lease = registry
            .begin_invocation(&old, invocation_request(Some(10_000)))
            .unwrap();
        registry.claim_invocation_dispatch(&lease).unwrap();
        registry.terminate(&old, kind).unwrap();
        assert_eq!(
            registry.finish_invocation(&lease, InvocationCompletion::Faulted),
            Err(RegistryError::NotFound)
        );
        clock.advance(Duration::from_secs(301));
        registry.maintain();
        let receipt = registry
            .tombstone(&old.session_id)
            .expect("unreleased generation must remain fenced after diagnostic TTL");
        assert!(receipt.unsettled_invocation.is_none());
        assert!(receipt.closure_evidence.is_none());
        registry
            .record_closure_evidence(&old, &released_evidence(&old, 7))
            .unwrap();
        registry.maintain();
        assert!(registry.tombstone(&old.session_id).is_none());
    }
}

#[test]
fn physical_hard_expiry_keeps_old_lease_fenced_when_new_generation_starts() {
    let clock = FakeClock::default();
    let mut settings = config(1);
    settings.physical_soft_drain_age = Duration::from_secs(9);
    settings.physical_max_age = Duration::from_secs(10);
    let mut registry = TransportSessionRegistry::new(clock.clone(), settings).unwrap();
    let old = registry.admit(request("hard-max-new-generation")).unwrap();
    registry.activate(&old).unwrap();
    let lease = registry
        .begin_invocation(&old, invocation_request(Some(100_000)))
        .unwrap();
    registry.claim_invocation_dispatch(&lease).unwrap();
    clock.advance(Duration::from_secs(10));
    registry.maintain();
    assert_eq!(
        registry.tombstone(&old.session_id).unwrap().kind,
        TerminationKind::ProviderHardMax
    );
    assert_eq!(
        registry
            .tombstone(&old.session_id)
            .unwrap()
            .unsettled_invocation
            .as_ref(),
        Some(&lease)
    );
    clock.advance(Duration::from_secs(1));
    assert_eq!(
        registry.finish_invocation(&lease, InvocationCompletion::Active),
        Err(RegistryError::NotFound)
    );
    registry
        .record_closure_evidence(&old, &released_evidence(&old, 7))
        .unwrap();
    clock.advance(Duration::from_secs(1));
    let next = registry.admit(request("hard-max-new-generation")).unwrap();
    registry.activate(&next).unwrap();
    let current = registry
        .begin_invocation(&next, invocation_request(Some(200_000)))
        .unwrap();
    registry.claim_invocation_dispatch(&current).unwrap();
    assert!(next.generation > old.generation);
    assert!(registry
        .terminate(&old, TerminationKind::OwnerClosed)
        .is_err());
    assert!(registry
        .finish_invocation(&lease, InvocationCompletion::Faulted)
        .is_err());
    assert!(registry
        .record_closure_evidence(&old, &released_evidence(&old, 7))
        .is_err());
    assert!(registry.invocation_inflight(&next).unwrap());
    assert_eq!(
        registry.invocation_deadline(&next).unwrap(),
        Some(current.deadline())
    );
}
