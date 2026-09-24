use super::*;

#[tokio::test]
async fn local_release_without_peer_ack_allows_only_safe_successor() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::ReleasedWithoutAck]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), transport_config(), clock)
            .unwrap();
    let mut input = invocation_input("release-without-ack", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let old = failed.lease.fence.clone();
    coordinator
        .finish(failed, &Err(transport_error("primary-cursor-error")))
        .await
        .unwrap();
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(
        snapshot.sessions[0]
            .closure_evidence
            .as_ref()
            .unwrap()
            .peer_close_acknowledged,
        Some(false)
    );
    input.previous_response_id = Some("bound-cursor".into());
    let epoch = TransportEpoch::new(17).unwrap();
    let mut directive = recovery_directive(epoch);
    directive.cursor_provenance = Some(CursorProvenance::connection_bound(
        epoch,
        plugin_framework::provider_contract::SocketIncarnation::new(1).unwrap(),
    ));
    input.set_recovery_directive(directive.clone()).unwrap();
    assert!(reason(
        coordinator
            .prepare("runtime-a", &mut input, &context(2_010_000))
            .await
            .err()
            .unwrap()
    )
    .contains("cursor_unreconstructible"));
    directive.cursor_provenance = None;
    input.set_recovery_directive(directive).unwrap();
    let next = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    assert!(next.lease.fence.generation > old.generation);
    assert_eq!(runtime.commands().len(), 1);
}

#[tokio::test]
async fn close_budget_and_deadline_survive_maintenance_and_terminal_upgrade() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(false); 5]));
    let mut config = transport_config();
    config.fault_grace = Duration::from_secs(60);
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
            .unwrap();
    let mut input = invocation_input("bounded-close", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let fence = failed.lease.fence.clone();
    coordinator
        .finish(failed, &Err(transport_error("primary")))
        .await
        .unwrap();
    let deadline = coordinator.pending_commands.lock().unwrap()[0].overall_deadline;
    for _ in 0..20 {
        coordinator.maintain_and_dispatch().await;
    }
    assert_eq!(
        runtime.commands().len(),
        1,
        "polling must not bypass backoff"
    );
    for _ in 1..5 {
        let next_due = coordinator.pending_commands.lock().unwrap()[0].next_due;
        clock.advance(Duration::from_millis(
            next_due.as_millis() - clock.now().as_millis(),
        ));
        coordinator.maintain_and_dispatch().await;
    }
    {
        let tasks = coordinator.pending_commands.lock().unwrap();
        assert_eq!(tasks[0].state, CloseTaskState::Exhausted);
        assert_eq!(tasks[0].attempts, CONTROL_MAX_ATTEMPTS);
        assert_eq!(tasks[0].overall_deadline, deadline);
    }
    coordinator
        .registry
        .lock()
        .await
        .terminate(&fence, TerminationKind::ProviderFault)
        .unwrap();
    coordinator.dispatch_pending_events().await;
    coordinator.maintain_and_dispatch().await;
    let tasks = coordinator.pending_commands.lock().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].attempts, 5);
    assert_eq!(tasks[0].overall_deadline, deadline);
    assert!(tasks[0].terminal_close);
    assert_eq!(runtime.commands().len(), 5);
    assert!(runtime
        .commands()
        .iter()
        .all(|command| command.deadline_unix_ms <= deadline.as_millis() as i64));
}

#[tokio::test]
async fn close_overall_deadline_expires_without_restarting_budget() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(false)]));
    let mut config = transport_config();
    config.fault_grace = Duration::from_secs(60);
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
            .unwrap();
    let mut input = invocation_input("close-expiry", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    coordinator
        .finish(failed, &Err(transport_error("primary")))
        .await
        .unwrap();
    clock.advance(Duration::from_secs(31));
    coordinator.maintain_and_dispatch().await;
    assert_eq!(runtime.commands().len(), 1);
    assert_eq!(
        coordinator.pending_commands.lock().unwrap()[0].state,
        CloseTaskState::Exhausted
    );
    assert!(reason(
        coordinator
            .prepare("runtime-a", &mut input, &context(2_040_000))
            .await
            .err()
            .unwrap()
    )
    .contains("close_exhausted"));
}

#[tokio::test]
async fn primary_failure_and_control_blocker_are_retained_without_provider_body() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([AckBehavior::ControlError]));
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime, transport_config(), clock).unwrap();
    let mut input = invocation_input("dual-diagnostics", ProviderWireOperation::Generate);
    let failed = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .unwrap()
        .unwrap();
    let primary: anyhow::Result<control_plane::ports::ProviderRuntimeInvocationOutput> =
        Err(PluginFrameworkError::runtime(ProviderRuntimeError::new(
            ProviderRuntimeErrorKind::RateLimited,
            "never-log-private-provider-body",
        ))
        .into());
    coordinator.finish(failed, &primary).await.unwrap();
    assert!(
        matches!(primary, Err(error) if error.to_string().contains("never-log-private-provider-body")),
        "original caller error must remain intact"
    );
    let blocked = coordinator
        .prepare("runtime-a", &mut input, &context(2_010_000))
        .await
        .err()
        .unwrap();
    let PluginFrameworkError::RuntimeContract { error } =
        blocked.downcast_ref::<PluginFrameworkError>().unwrap()
    else {
        panic!("typed error required")
    };
    assert_eq!(error.message, "provider_physical_connection_close_pending");
    let details = error.provider_details.as_ref().unwrap();
    assert_eq!(
        details["transport_recovery"]["primary_error"]["kind"],
        serde_json::json!("rate_limited")
    );
    assert_eq!(
        details["transport_recovery"]["primary_error"]["code"],
        serde_json::json!("provider_rate_limited")
    );
    assert_eq!(
        details["transport_recovery"]["first_control_failure"],
        serde_json::json!("control_execution_error")
    );
    assert_eq!(
        details["transport_recovery"]["attempts"],
        serde_json::json!(1)
    );
    assert!(!details.to_string().contains("never-log-private"));
}

#[tokio::test]
async fn inflight_drain_exhaustion_is_explicit_without_replay_or_new_budget() {
    let clock = FakeClock::new(2_000_000);
    let runtime = Arc::new(FakeTransportRuntime::new([]));
    let mut config = transport_config();
    config.logical_max_age = Duration::from_secs(120);
    config.physical_max_age = Duration::from_secs(120);
    let coordinator =
        TransportSessionCoordinator::new_with_clock(runtime.clone(), config, clock.clone())
            .unwrap();
    let mut input = invocation_input("long-inflight", ProviderWireOperation::Generate);
    let active = coordinator
        .prepare("runtime-a", &mut input, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    clock.advance(Duration::from_secs(20));
    coordinator.maintain_and_dispatch().await;
    clock.advance(Duration::from_secs(31));
    coordinator.maintain_and_dispatch().await;
    let error = coordinator
        .prepare("runtime-a", &mut input, &context(2_100_000))
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("provider_physical_connection_close_exhausted"));
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(snapshot.sessions[0].fence, active.lease.fence);
    assert!(snapshot.sessions[0].inflight);
    assert!(runtime.commands().is_empty());
    assert_eq!(coordinator.pending_commands.lock().unwrap()[0].attempts, 0);
}
