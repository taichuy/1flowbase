use super::*;

#[test]
fn settlement_and_dispatch_are_mutually_exclusive_for_exact_generation() {
    let mut registry = TransportSessionRegistry::new(FakeClock::default(), config(2)).unwrap();
    let fence = registry.admit(request("reserved")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .finish_invocation(&lease, InvocationCompletion::Faulted)
        .unwrap();
    assert!(registry.settle_never_dispatched(&fence).unwrap());
    assert!(registry.claim_invocation_dispatch(&lease).is_err());
    assert!(registry.safe_snapshot().sessions[0]
        .closure_evidence
        .is_none());
    let next = registry.rotate_generation(&fence).unwrap();
    registry.activate(&next).unwrap();
    let successor = registry
        .begin_invocation(&next, invocation_request(None))
        .unwrap();
    registry.claim_invocation_dispatch(&successor).unwrap();
    assert!(registry.claim_invocation_dispatch(&successor).is_err());
    assert!(!registry.settle_never_dispatched(&next).unwrap());
    assert!(registry.settle_never_dispatched(&fence).is_err());
    assert!(registry.safe_snapshot().sessions[0].inflight);
    registry
        .finish_invocation(&successor, InvocationCompletion::Faulted)
        .unwrap();
    assert!(
        registry.rotate_generation(&next).is_err(),
        "real handoff still requires Host proof"
    );
}

#[test]
fn never_dispatched_tombstone_settlement_clears_reservation_and_can_expire() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(2)).unwrap();
    let fence = registry.admit(request("never-handed-off")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry
        .terminate(&fence, TerminationKind::OwnerOrphaned)
        .unwrap();
    assert!(registry.settle_never_dispatched(&fence).unwrap());
    assert!(registry.claim_invocation_dispatch(&lease).is_err());
    let receipt = registry.tombstone(&fence.session_id).unwrap();
    assert!(receipt.unsettled_invocation.is_none());
    assert!(receipt.never_dispatched_settled);
    assert!(receipt.closure_evidence.is_none());
    clock.advance(Duration::from_secs(301));
    registry.maintain();
    assert!(registry.tombstone(&fence.session_id).is_none());
}

#[test]
fn real_dispatch_tombstone_cannot_be_settled_by_absence_or_stale_owner() {
    let clock = FakeClock::default();
    let mut registry = TransportSessionRegistry::new(clock.clone(), config(2)).unwrap();
    let fence = registry.admit(request("real-handoff")).unwrap();
    registry.activate(&fence).unwrap();
    let lease = registry
        .begin_invocation(&fence, invocation_request(None))
        .unwrap();
    registry.claim_invocation_dispatch(&lease).unwrap();
    registry
        .terminate(&fence, TerminationKind::OwnerOrphaned)
        .unwrap();
    assert!(!registry.settle_never_dispatched(&fence).unwrap());
    clock.advance(Duration::from_secs(301));
    registry.maintain();
    let receipt = registry.tombstone(&fence.session_id).unwrap();
    assert!(receipt.dispatch_claimed);
    assert!(!receipt.never_dispatched_settled);
    assert!(receipt.closure_evidence.is_none());
}
