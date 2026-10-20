use super::*;

#[tokio::test]
async fn scoped_termination_matches_actual_model_fence_and_invocation_only() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([AckBehavior::Matching(true)])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let a = coordinator.open_connection_scope();
    let b = coordinator.open_connection_scope();
    let mut input_a = scope_input(&a, "model-a");
    input_a.native_transport = Some(
        plugin_framework::provider_contract::ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: serde_json::json!({"model":"model-a","input":[]}),
            digest: "fixture".into(),
            size_bytes: 0,
        },
    );
    let first = coordinator
        .prepare_dispatched("runtime-a", &mut input_a, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input_a.client_transport,
        Some(ProviderClientTransport::Websocket)
    );
    assert!(!input_a
        .client_protocol_envelope
        .as_ref()
        .unwrap()
        .headers
        .contains_key(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER
        ));
    let mut input_b = scope_input(&b, "model-b");
    let second = coordinator
        .prepare_dispatched("runtime-a", &mut input_b, &context(2_100_000))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        input_b.client_transport,
        Some(ProviderClientTransport::Websocket)
    );
    let mut notices = coordinator.subscribe();
    coordinator
        .registry
        .lock()
        .await
        .terminate(&first.lease.fence, TerminationKind::ProviderHardMax)
        .unwrap();
    coordinator.dispatch_pending_events().await;
    let notice = notices.try_recv().unwrap();
    assert!(a.accepts_termination(&notice));
    assert!(!b.accepts_termination(&notice));
    assert_eq!(notice.fence, first.lease.fence);
    assert_eq!(notice.invocation_sequence, Some(first.lease.sequence()));
    coordinator.close_connection_scope(&a).await;
    assert_eq!(
        coordinator
            .registry
            .lock()
            .await
            .state(&second.lease.fence)
            .unwrap(),
        TransportSessionState::Active
    );
}

#[tokio::test]
async fn completed_connection_close_and_old_notices_do_not_orphan_successor() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let old = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let old_lease = first.lease.clone();
    coordinator
        .finish(first, &successful_output(old_lease.fence.generation.get()))
        .await
        .unwrap();
    assert!(old.state.lock().unwrap().leases.is_empty());
    let current = coordinator.open_connection_scope();
    let second = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&current, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.lease.fence, old_lease.fence);
    assert!(second.lease.sequence() > old_lease.sequence());
    let stale_notice = TransportTerminationNotice {
        owner_id: "shared-thread".into(),
        fence: old_lease.fence.clone(),
        invocation_sequence: Some(old_lease.sequence()),
        connection_scope_id: Some(current.id().into()),
        code: "provider_connection_max_age",
    };
    assert!(
        !current.accepts_termination(&stale_notice),
        "same physical generation does not identify the new invocation"
    );
    coordinator.close_connection_scope(&old).await;
    assert_eq!(
        coordinator
            .registry
            .lock()
            .await
            .state(&second.lease.fence)
            .unwrap(),
        TransportSessionState::Active
    );
    let fence = second.lease.fence.clone();
    coordinator
        .finish(second, &successful_output(fence.generation.get()))
        .await
        .unwrap();
    coordinator.close_connection_scope(&current).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::IdleAffinity
    );
    assert!(coordinator.connection_scopes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn active_scope_disconnect_orphans_only_its_current_lease_and_rejects_forgery() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let mut forged = scope_input(&scope, "model-a");
    forged
        .client_protocol_envelope
        .as_mut()
        .unwrap()
        .headers
        .insert(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER.into(),
            vec!["client-forged".into()],
        );
    assert!(coordinator
        .prepare_dispatched("runtime-a", &mut forged, &context(2_100_000))
        .await
        .is_err());
    assert!(!forged
        .client_protocol_envelope
        .as_ref()
        .unwrap()
        .headers
        .contains_key(
            control_plane::orchestration_runtime::HOST_TRANSPORT_CONNECTION_SCOPE_HEADER
        ));
    assert!(coordinator.safe_snapshot().await.sessions.is_empty());
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    coordinator.close_connection_scope(&scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
    coordinator
        .finish(first, &successful_output(fence.generation.get()))
        .await
        .unwrap();
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );
}

#[tokio::test]
async fn completed_waiting_tool_retains_business_state_after_socket_close() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        TransportRegistryConfig::default(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    let mut output = successful_output(fence.generation.get()).unwrap();
    output.result.tool_calls = vec![plugin_framework::provider_contract::ProviderToolCall {
        id: "committed-tool".into(),
        name: "read".into(),
        arguments: serde_json::json!({}),
        provider_metadata: serde_json::json!({}),
    }];
    coordinator.finish(first, &Ok(output)).await.unwrap();
    assert!(scope.state.lock().unwrap().leases.is_empty());
    coordinator.close_connection_scope(&scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::WaitingTool
    );
}

/// #2085 orphan/mailbox: a delivery unbind must not permanently mark the call as
/// orphaned. This is the exact incident shape - a mailbox-style client
/// disconnect while the model call runs, the original call completing
/// successfully afterwards, and the next round arriving on a new connection.
#[tokio::test]
async fn unbound_delivery_admits_the_successor_round_after_the_original_completes() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    let first_sequence = first.lease.sequence();

    // The client drops the delivery while the model call is still running.
    coordinator.close_connection_scope(&scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Orphaned
    );

    // Kernel completion is independent of the delivery and still succeeds.
    let mut output = successful_output(fence.generation.get()).unwrap();
    output.result.tool_calls = vec![plugin_framework::provider_contract::ProviderToolCall {
        id: "committed-tool".into(),
        name: "read".into(),
        arguments: serde_json::json!({}),
        provider_metadata: serde_json::json!({}),
    }];
    coordinator.finish(first, &Ok(output)).await.unwrap();
    let snapshot = coordinator.safe_snapshot().await;
    assert_eq!(
        snapshot.sessions[0].state,
        TransportSessionState::Orphaned,
        "the unbound delivery stays visible as a fact"
    );
    assert!(
        !snapshot.sessions[0].inflight,
        "but the execution itself is terminal"
    );

    // The tool result returns on a new connection: same owner identity, same
    // logical session, same physical generation, and no second model call.
    let next_scope = coordinator.open_connection_scope();
    let successor = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&next_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .expect("a legitimate successor must be admitted")
        .unwrap();
    assert_eq!(successor.lease.fence, fence);
    assert!(successor.lease.sequence() > first_sequence);
}

/// Ordering coverage: the successor is admitted on the reclaimed session first,
/// and only then does the *old* delivery finish closing. A late close must not
/// rebind the delivery fact onto the running successor.
#[tokio::test]
async fn a_late_close_of_the_old_delivery_cannot_orphan_the_admitted_successor() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let old_scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&old_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    coordinator.close_connection_scope(&old_scope).await;
    coordinator
        .finish(first, &successful_output(fence.generation.get()))
        .await
        .unwrap();

    let new_scope = coordinator.open_connection_scope();
    let successor = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&new_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Active
    );

    // The old connection's close completes last, with no leases left to detach.
    coordinator.close_connection_scope(&old_scope).await;
    assert_eq!(
        coordinator.registry.lock().await.state(&fence).unwrap(),
        TransportSessionState::Active,
        "a late old-delivery close must not orphan the running successor"
    );
    assert!(coordinator
        .registry
        .lock()
        .await
        .invocation_inflight(&successor.lease.fence)
        .unwrap());
    coordinator
        .finish(successor, &successful_output(fence.generation.get()))
        .await
        .unwrap();
}

#[tokio::test]
async fn unbound_inflight_execution_without_replacement_scope_is_refused() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();
    let scope = coordinator.open_connection_scope();
    let first = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    let fence = first.lease.fence.clone();
    let first_sequence = first.lease.sequence();
    coordinator.close_connection_scope(&scope).await;

    let next_scope = coordinator.open_connection_scope();
    let mut unscoped = scope_input(&next_scope, "model-a");
    take_transport_connection_scope(&mut unscoped);
    let error = coordinator
        .prepare_dispatched("runtime-a", &mut unscoped, &context(2_100_000))
        .await
        .err()
        .expect("an unbound in-flight execution must not start a second call");
    let details = admission_diagnostics(&error);
    assert!(reason(error).contains("transport_session_inflight_unbound"));
    assert_eq!(details["state"], serde_json::json!("orphaned"));
    assert_eq!(details["inflight"], serde_json::json!(true));
    assert!(details["generation"].as_u64().unwrap() >= 1);

    // The refused call consumed nothing: the real successor still gets the very
    // next invocation sequence after the original call finishes.
    let snapshot = coordinator.safe_snapshot().await;
    assert!(snapshot.sessions[0].inflight);
    coordinator
        .finish(first, &successful_output(fence.generation.get()))
        .await
        .unwrap();
    let successor = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&next_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(successor.lease.sequence(), first_sequence + 1);
}

#[tokio::test]
async fn admission_rejections_are_locatable_by_their_exact_branch() {
    let coordinator = TransportSessionCoordinator::new_with_clock(
        Arc::new(FakeTransportRuntime::new([])),
        transport_config(),
        FakeClock::new(2_000_000),
    )
    .unwrap();

    // A delivery scope this host no longer owns: an expired/closed reference is
    // its own branch, not the same fact as a registry orphan.
    let stale_scope = coordinator.open_connection_scope();
    coordinator.close_connection_scope(&stale_scope).await;
    let error = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&stale_scope, "model-a"),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    let details = admission_diagnostics(&error);
    assert!(reason(error).contains("transport_session_scope_unknown"));
    assert_eq!(details["connection_scope_bound"], serde_json::json!(true));
    assert_eq!(details["state"], serde_json::Value::Null);

    // Host shutdown is its own branch as well.
    coordinator.shutdown.store(true, Ordering::Release);
    let error = coordinator
        .prepare_dispatched(
            "runtime-a",
            &mut scope_input(&coordinator.open_connection_scope(), "model-a"),
            &context(2_100_000),
        )
        .await
        .err()
        .unwrap();
    assert!(reason(error).contains("transport_session_shutdown"));
}
