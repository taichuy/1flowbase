use super::*;
use plugin_framework::provider_contract::{
    ProviderTransportClosureSource, ProviderTransportNoAckReason, SocketIncarnation,
};

fn worker_exit(fence: &TransportFence) -> ProviderTransportClosureEvidence {
    ProviderTransportClosureEvidence {
        identity: ProviderTransportSessionIdentity {
            logical_session_id: fence.session_id.as_str().into(),
            generation: fence.generation.get(),
            worker_incarnation: 37,
        },
        source: ProviderTransportClosureSource::ConfirmedWorkerExit,
        local_released: true,
        peer_close_acknowledged: None,
        no_ack_reason: Some(ProviderTransportNoAckReason::Unknown),
    }
}

async fn idle_fixture() -> (
    TransportSessionCoordinator<FakeClock>,
    Arc<FakeTransportRuntime>,
    FakeClock,
    ProviderInvocationInput,
    InvocationLease,
) {
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let clock = FakeClock::new(2_000_000);
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        clock.clone(),
    )
    .unwrap();
    let mut input = invocation_input("worker-exit-next-turn", ProviderWireOperation::Generate);
    input
        .set_recovery_directive(recovery_directive(TransportEpoch::new(91).unwrap()))
        .unwrap();
    let prepared = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let lease = prepared.lease.clone();
    coordinator
        .finish(prepared, &successful_output(lease.fence.generation.get()))
        .await
        .unwrap();
    (coordinator, runtime, clock, input, lease)
}

#[tokio::test]
async fn live_worker_next_turn_retains_transport_generation_without_control() {
    let (coordinator, runtime, _, mut input, old) = idle_fixture().await;
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next.lease.fence, old.fence);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert_eq!(
        runtime.probes.lock().unwrap().as_slice(),
        &[(
            "runtime-a".into(),
            old.fence.session_id.as_str().into(),
            old.fence.generation.get(),
        )]
    );
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn worker_exit_between_turns_rotates_once_preserving_logical_deadline_and_budget() {
    let (coordinator, runtime, clock, mut input, old) = idle_fixture().await;
    let before = coordinator.safe_snapshot().await;
    let recovery = input.recovery_directive().unwrap();
    input.messages = serde_json::from_value(serde_json::json!([
        {"role":"user", "content":"first question"},
        {"role":"assistant", "content":"first answer"},
        {"role":"user", "content":"follow-up using the full history"},
    ]))
    .unwrap();
    let complete_history = input.messages.clone();
    input.previous_response_id = Some("transferable-continuation".into());
    *runtime.worker_exit_evidence.lock().unwrap() = Some(worker_exit(&old.fence));
    clock.advance(Duration::from_secs(1));
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let after = coordinator.safe_snapshot().await;
    assert!(next.lease.fence.generation > old.fence.generation);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert_eq!(next.lease.fence.session_id, old.fence.session_id);
    assert_eq!(
        after.sessions[0].logical_ttl + Duration::from_secs(1),
        before.sessions[0].logical_ttl
    );
    assert_eq!(
        next.lease.deadline(),
        TransportInstant::from_millis(2_010_000)
    );
    assert_eq!(input.recovery_directive().unwrap(), recovery);
    assert_eq!(input.messages, complete_history);
    assert_eq!(
        input.previous_response_id.as_deref(),
        Some("transferable-continuation")
    );
    assert_eq!(
        input
            .recovery_directive()
            .unwrap()
            .unwrap()
            .transport_epoch
            .get(),
        91
    );
    assert!(
        runtime.commands().is_empty(),
        "proof must not send Close to a successor worker"
    );
    assert!(after.sessions[0].closure_evidence.is_none());
    assert!(matches!(
        coordinator.registry.lock().await.fence_status(&old.fence),
        TransportFenceStatus::Stale { .. }
    ));
}

#[tokio::test]
async fn worker_exit_rejects_connection_bound_cursor_before_next_dispatch() {
    let (coordinator, runtime, _, mut input, old) = idle_fixture().await;
    *runtime.worker_exit_evidence.lock().unwrap() = Some(worker_exit(&old.fence));
    let epoch = TransportEpoch::new(91).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        SocketIncarnation::new(37).unwrap(),
    ));
    input.previous_response_id = Some("old-socket-only-cursor".into());
    input.set_recovery_directive(directive).unwrap();
    let original = input.clone();
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    let after = coordinator.safe_snapshot().await;
    assert_eq!(after.sessions[0].fence, old.fence);
    assert_eq!(after.sessions[0].state, TransportSessionState::Faulted);
    assert!(!after.sessions[0].inflight);
    assert!(
        after.sessions[0]
            .closure_evidence
            .as_ref()
            .unwrap()
            .local_released
    );
    assert_eq!(input.previous_response_id, original.previous_response_id);
    assert_eq!(
        input.recovery_directive().unwrap(),
        original.recovery_directive().unwrap()
    );
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn wrong_stale_or_unconfirmed_exit_proof_cannot_fault_or_rotate_current_transport() {
    for fault in 0..4 {
        let (coordinator, runtime, _, mut input, old) = idle_fixture().await;
        let before = coordinator.safe_snapshot().await;
        let mut evidence = worker_exit(&old.fence);
        match fault {
            0 => evidence.identity.generation += 1,
            1 => evidence.identity.logical_session_id = "other-session".into(),
            2 => evidence.source = ProviderTransportClosureSource::ProviderLocalRelease,
            _ => evidence.local_released = false,
        }
        *runtime.worker_exit_evidence.lock().unwrap() = Some(evidence);
        let error = coordinator
            .prepare("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap();
        assert!(reason(error).contains("provider_transport_worker_exit_evidence_invalid"));
        assert_eq!(coordinator.safe_snapshot().await, before);
        assert!(runtime.commands().is_empty());
    }
}

#[tokio::test]
async fn worker_exit_probe_never_marks_an_active_invocation_dead() {
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let coordinator = TransportSessionCoordinator::new_with_clock(
        runtime.clone(),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let mut input = invocation_input("active-worker-exit", ProviderWireOperation::Generate);
    let first = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    *runtime.worker_exit_evidence.lock().unwrap() = Some(worker_exit(&first.lease.fence));
    let before = coordinator.safe_snapshot().await;
    assert!(coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .is_err());
    assert!(runtime.probes.lock().unwrap().is_empty());
    assert_eq!(coordinator.safe_snapshot().await, before);
    assert!(runtime.commands().is_empty());
}
