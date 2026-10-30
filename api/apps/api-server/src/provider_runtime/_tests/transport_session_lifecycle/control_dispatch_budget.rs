use super::*;

type Coordinator = TransportSessionCoordinator<FakeClock>;

async fn queue_events(coordinator: &Coordinator) {
    let mut registry = coordinator.registry.lock().await;
    let commands: Vec<_> = registry
        .drain_events()
        .into_iter()
        .filter_map(|event| lifecycle_command(&registry, event))
        .collect();
    coordinator
        .pending_commands
        .lock()
        .unwrap()
        .extend(commands);
}

async fn fixture(
    mode: u8,
    inflight: bool,
    claimed: bool,
    config: TransportRegistryConfig,
) -> (
    Coordinator,
    FakeClock,
    Arc<FakeTransportRuntime>,
    InvocationLease,
) {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(false); 8]));
    let coordinator = Coordinator::new_with_clock(runtime.clone(), config, clock.clone()).unwrap();
    let mut input = invocation_input("queued-control-budget", ProviderWireOperation::Generate);
    let mut prepared = coordinator
        .prepare("runtime-a", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    if claimed {
        coordinator.claim_dispatch(&prepared).await.unwrap();
    }
    let lease = prepared.lease.clone();
    // The fixture owns settlement explicitly, without running coordinator dispatch yet.
    prepared.on_abandon = None;
    let mut registry = coordinator.registry.lock().await;
    if !inflight {
        registry
            .finish_invocation(&lease, InvocationCompletion::IdleAffinity)
            .unwrap();
    }
    match mode {
        0 => registry
            .transition(&lease.fence, TransportSessionState::Draining)
            .unwrap(),
        1 => registry
            .transition(&lease.fence, TransportSessionState::Faulted)
            .unwrap(),
        _ => {
            registry
                .terminate(&lease.fence, TerminationKind::OwnerClosed)
                .unwrap();
        }
    }
    drop(registry);
    queue_events(&coordinator).await;
    (coordinator, clock, runtime, lease)
}

fn config() -> TransportRegistryConfig {
    TransportRegistryConfig {
        logical_max_age: Duration::from_secs(120),
        physical_soft_drain_age: Duration::from_secs(90),
        physical_max_age: Duration::from_secs(120),
        ..Default::default()
    }
}

#[tokio::test]
async fn queued_drain_fault_close_and_terminal_close_start_budget_at_first_rpc() {
    for mode in 0..3 {
        let (coordinator, clock, runtime, _) = fixture(mode, false, true, config()).await;
        assert!(runtime.commands().is_empty());
        clock.advance(Duration::from_secs(30));
        coordinator.dispatch_pending_events().await;
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert_eq!(
            tasks[0].attempts, 1,
            "ordinary queue delay must not exhaust attempt zero"
        );
        assert_eq!(tasks[0].state, CloseTaskState::Pending);
        assert_eq!(tasks[0].overall_deadline.as_millis(), 2_060_000);
        assert_eq!(runtime.commands()[0].deadline_unix_ms, 2_035_000);
    }
}

#[tokio::test]
async fn deferred_drain_with_zero_logical_ttl_gets_independent_cleanup_budget() {
    let mut settings = config();
    settings.logical_max_age = Duration::from_secs(10);
    let (coordinator, clock, runtime, lease) = fixture(0, true, true, settings).await;
    let original_hard = coordinator
        .registry
        .lock()
        .await
        .physical_hard_deadline(&lease.fence)
        .unwrap();
    let unstarted = coordinator.pending_commands.lock().unwrap()[0].overall_deadline;
    clock.advance(Duration::from_secs(30));
    coordinator.dispatch_pending_events().await;
    {
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert_eq!(tasks[0].attempts, 0);
        assert_eq!(tasks[0].state, CloseTaskState::Pending);
        assert_eq!(tasks[0].overall_deadline, unstarted);
    }
    assert!(runtime.commands().is_empty());
    coordinator
        .registry
        .lock()
        .await
        .finish_invocation(&lease, InvocationCompletion::IdleAffinity)
        .unwrap();
    let before = coordinator.safe_snapshot().await;
    assert_eq!(before.sessions[0].logical_ttl, Duration::ZERO);
    coordinator.dispatch_pending_events().await;
    {
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert_eq!(tasks[0].attempts, 1);
        assert_eq!(tasks[0].overall_deadline.as_millis(), 2_060_000);
    }
    assert_eq!(runtime.commands()[0].deadline_unix_ms, 2_035_000);
    let mut registry = coordinator.registry.lock().await;
    assert_eq!(
        registry.physical_hard_deadline(&lease.fence).unwrap(),
        original_hard
    );
    assert!(
        registry
            .begin_invocation(&lease.fence, InvocationRequest::default())
            .is_err(),
        "cleanup must not renew expired business admission"
    );
}

#[tokio::test]
async fn physical_and_logical_expiry_while_queued_still_allow_fenced_cleanup() {
    for logical_age in [10, 40] {
        let mut settings = config();
        settings.logical_max_age = Duration::from_secs(logical_age);
        settings.physical_soft_drain_age = Duration::from_secs(5);
        settings.physical_max_age = Duration::from_secs(20);
        let (coordinator, clock, runtime, lease) = fixture(0, false, true, settings).await;
        let hard = coordinator
            .registry
            .lock()
            .await
            .physical_hard_deadline(&lease.fence)
            .unwrap();
        clock.advance(Duration::from_secs(30));
        coordinator.maintain_and_dispatch().await;
        {
            let tasks = coordinator.pending_commands.lock().unwrap();
            assert!(tasks[0].terminal_close);
            assert_eq!(tasks[0].attempts, 1);
            assert_eq!(tasks[0].overall_deadline.as_millis(), 2_060_000);
            assert_eq!(
                runtime.commands()[0].action,
                ProviderTransportSessionAction::Close
            );
            assert_eq!(runtime.commands()[0].deadline_unix_ms, 2_035_000);
        }
        assert_eq!(hard.as_millis(), 2_020_000);
        let snapshot = coordinator.safe_snapshot().await;
        assert!(snapshot.sessions.is_empty());
        assert_eq!(
            snapshot.tombstones[0].kind,
            if logical_age == 10 {
                TerminationKind::DeadlineExceeded(DeadlineKind::LogicalAbsolute)
            } else {
                TerminationKind::ProviderHardMax
            }
        );
        assert_eq!(
            snapshot.tombstones[0].connection_age,
            Duration::from_secs(30)
        );
        assert!(coordinator
            .registry
            .lock()
            .await
            .begin_invocation(&lease.fence, InvocationRequest::default())
            .is_err());
    }
}

#[tokio::test]
async fn release_never_dispatched_stale_and_next_due_do_not_start_cleanup_budget() {
    for (mode, claimed) in [(0, false), (0, true), (2, false), (2, true)] {
        let (coordinator, clock, runtime, lease) = fixture(mode, false, claimed, config()).await;
        let before = coordinator.pending_commands.lock().unwrap()[0].overall_deadline;
        if claimed {
            coordinator.registry.lock().await.record_closure_evidence(&lease.fence, &ProviderTransportClosureEvidence {
                identity: ProviderTransportSessionIdentity { logical_session_id: lease.fence.session_id.as_str().into(), generation: lease.fence.generation.get(), worker_incarnation: 1 },
                source: plugin_framework::provider_contract::ProviderTransportClosureSource::ProviderLocalRelease,
                local_released: true, peer_close_acknowledged: Some(true), no_ack_reason: None,
            }).unwrap();
        }
        clock.advance(Duration::from_secs(30));
        coordinator.dispatch_pending_events().await;
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert_eq!(tasks[0].state, CloseTaskState::Released);
        assert_eq!(tasks[0].attempts, 0);
        assert_eq!(tasks[0].overall_deadline, before);
        assert!(runtime.commands().is_empty());
    }
    let (coordinator, _clock, runtime, lease) = fixture(0, false, true, config()).await;
    {
        let mut registry = coordinator.registry.lock().await;
        registry.record_closure_evidence(&lease.fence, &ProviderTransportClosureEvidence {
            identity: ProviderTransportSessionIdentity { logical_session_id: lease.fence.session_id.as_str().into(), generation: lease.fence.generation.get(), worker_incarnation: 1 },
            source: plugin_framework::provider_contract::ProviderTransportClosureSource::ProviderLocalRelease,
            local_released: true, peer_close_acknowledged: Some(true), no_ack_reason: None,
        }).unwrap();
        let next = registry.rotate_generation(&lease.fence).unwrap();
        registry.activate(&next).unwrap();
    }
    coordinator.dispatch_pending_events().await;
    assert!(coordinator.pending_commands.lock().unwrap().is_empty());
    assert!(runtime.commands().is_empty());
    let (coordinator, clock, runtime, _) = fixture(2, false, true, config()).await;
    let before = {
        let mut tasks = coordinator.pending_commands.lock().unwrap();
        tasks[0].next_due = TransportInstant::from_millis(2_040_000);
        tasks[0].overall_deadline
    };
    clock.advance(Duration::from_secs(30));
    coordinator.dispatch_pending_events().await;
    assert_eq!(coordinator.pending_commands.lock().unwrap()[0].attempts, 0);
    assert_eq!(
        coordinator.pending_commands.lock().unwrap()[0].overall_deadline,
        before
    );
    assert!(runtime.commands().is_empty());
}

#[tokio::test]
async fn started_drain_upgrade_retries_and_exhaustion_keep_original_window() {
    let (coordinator, clock, runtime, lease) = fixture(0, false, true, config()).await;
    coordinator.dispatch_pending_events().await;
    let deadline = coordinator.pending_commands.lock().unwrap()[0].overall_deadline;
    coordinator
        .registry
        .lock()
        .await
        .terminate(&lease.fence, TerminationKind::OwnerClosed)
        .unwrap();
    for _ in 0..3 {
        coordinator.maintain_and_dispatch().await;
    }
    assert_eq!(runtime.commands().len(), 1);
    assert_eq!(
        coordinator.pending_commands.lock().unwrap()[0].overall_deadline,
        deadline
    );
    clock.advance(Duration::from_secs(30));
    coordinator.maintain_and_dispatch().await;
    {
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert!(tasks[0].terminal_close);
        assert_eq!(tasks[0].attempts, 1);
        assert_eq!(tasks[0].state, CloseTaskState::Exhausted);
        assert_eq!(tasks[0].overall_deadline, deadline);
    }
    coordinator.maintain_and_dispatch().await;
    assert_eq!(runtime.commands().len(), 1);
}
