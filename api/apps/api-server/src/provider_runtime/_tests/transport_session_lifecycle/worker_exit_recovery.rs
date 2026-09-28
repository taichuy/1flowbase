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
        TransportRegistryConfig {
            fault_grace: Duration::from_secs(2),
            ..transport_config()
        },
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
async fn confirmed_exit_satisfies_exhausted_close_without_another_control_call() {
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(false); 5]));
    let clock = FakeClock::new(2_000_000);
    let mut config = transport_config();
    config.fault_grace = Duration::from_secs(60);
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
            .unwrap();
    let mut input = invocation_input("exited-exhausted-close", ProviderWireOperation::Generate);
    input
        .set_recovery_directive(recovery_directive(TransportEpoch::new(91).unwrap()))
        .unwrap();
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let old = failed.lease.fence.clone();
    coordinator
        .finish(failed, &Err(transport_error("primary")))
        .await
        .unwrap();
    for _ in 1..5 {
        let due = coordinator.pending_commands.lock().unwrap()[0].next_due;
        clock.advance(due.saturating_duration_since(clock.now()));
        coordinator.maintain_and_dispatch().await;
    }
    assert_eq!(runtime.commands().len(), 5);
    assert!(coordinator.close_task_exhausted(&old));
    let mut evidence = worker_exit(&old);
    evidence.identity.worker_incarnation = 1;
    coordinator
        .registry
        .lock()
        .await
        .record_worker_exit_evidence(&old, &evidence)
        .unwrap();
    coordinator.dispatch_pending_events().await;
    {
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert_eq!(tasks[0].state, CloseTaskState::Released);
        assert_eq!(tasks[0].attempts, 5);
        assert_eq!(tasks[0].command.worker_incarnation, Some(1));
        assert!(tasks[0].first_control_failure.is_some());
    }
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_055_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > old.generation);
    assert_eq!(runtime.commands().len(), 5);
}

#[tokio::test]
async fn confirmed_exit_terminal_notice_survives_close_elision_once() {
    let (coordinator, runtime, _, _, old) = idle_fixture().await;
    let mut notices = coordinator.notices.subscribe();
    {
        let mut registry = coordinator.registry.lock().await;
        registry
            .record_worker_exit_evidence(&old.fence, &worker_exit(&old.fence))
            .unwrap();
        registry
            .terminate(&old.fence, TerminationKind::ProviderFault)
            .unwrap();
    }
    coordinator.dispatch_pending_events().await;
    let notice = notices.try_recv().unwrap();
    assert_eq!(notice.fence, old.fence);
    assert_eq!(
        notice.code,
        termination_code(TerminationKind::ProviderFault)
    );
    assert!(runtime.commands().is_empty());
    coordinator.dispatch_pending_events().await;
    assert!(matches!(
        notices.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
    let tasks = coordinator.pending_commands.lock().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].state, CloseTaskState::Released);
    assert_eq!(tasks[0].attempts, 0);
    assert_eq!(tasks[0].command.worker_incarnation, Some(37));
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
    let (coordinator, runtime, clock, mut input, old) = idle_fixture().await;
    let before = coordinator.safe_snapshot().await;
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

    // The rejected cursor leaves a released Faulted identity. A later full-history
    // successor must survive both the old fault lease and the dead physical age.
    clock.advance(Duration::from_secs(41));
    input.previous_response_id = None;
    input
        .set_recovery_directive(recovery_directive(epoch))
        .unwrap();
    input.messages = serde_json::from_value(serde_json::json!([
        {"role":"user", "content":"first question"},
        {"role":"assistant", "content":"first answer"},
        {"role":"user", "content":"continue using the complete history"},
    ]))
    .unwrap();
    let history = input.messages.clone();
    let recovery = input.recovery_directive().unwrap();
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_055_000))
        .await
        .unwrap()
        .unwrap();
    let after = coordinator.safe_snapshot().await;
    assert_eq!(next.lease.fence.session_id, old.fence.session_id);
    assert!(next.lease.fence.generation > old.fence.generation);
    assert_eq!(next.lease.sequence(), old.sequence() + 1);
    assert_eq!(
        next.lease.deadline(),
        TransportInstant::from_millis(2_055_000)
    );
    assert_eq!(
        after.sessions[0].logical_ttl + Duration::from_secs(41),
        before.sessions[0].logical_ttl
    );
    assert_eq!(input.messages, history);
    assert_eq!(input.recovery_directive().unwrap(), recovery);
    assert!(input.previous_response_id.is_none());
    assert!(runtime.commands().is_empty());
    assert!(after.sessions[0].closure_evidence.is_none());
    assert!(matches!(
        coordinator.registry.lock().await.fence_status(&old.fence),
        TransportFenceStatus::Stale { .. }
    ));
    assert!(coordinator
        .registry
        .lock()
        .await
        .finish_invocation(&old, InvocationCompletion::Active)
        .is_err());
}

#[tokio::test]
async fn released_fault_delayed_successor_preserves_expired_call_budget_guard() {
    let (coordinator, runtime, clock, mut input, old) = idle_fixture().await;
    *runtime.worker_exit_evidence.lock().unwrap() = Some(worker_exit(&old.fence));
    let epoch = TransportEpoch::new(91).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        SocketIncarnation::new(37).unwrap(),
    ));
    input.set_recovery_directive(directive).unwrap();
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_transport_cursor_unreconstructible"));
    clock.advance(Duration::from_secs(41));
    let mut directive = recovery_directive(epoch);
    directive.policy = RecoveryPolicy::SemanticMapped {
        budget: RecoveryBudget {
            max_inner_attempts: 2,
            absolute_deadline_unix_ms: 2_041_000,
        },
    };
    input.set_recovery_directive(directive).unwrap();
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_055_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_invocation_deadline_exceeded"));
    let after = coordinator.safe_snapshot().await;
    assert_eq!(after.sessions[0].fence, old.fence);
    assert_eq!(after.sessions[0].state, TransportSessionState::Faulted);
    assert!(!after.sessions[0].inflight);
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
