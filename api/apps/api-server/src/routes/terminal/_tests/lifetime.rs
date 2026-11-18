use super::*;
#[tokio::test]
async fn terminal_typed_weak_preserves_real_state_identity_clone_expiration_and_lease_abort() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let adapter = TerminalAdapter::test_from_weak(Arc::downgrade(&state));
    let weak = adapter.test_host();
    assert!(std::ptr::addr_eq(weak.as_ptr(), Arc::as_ptr(&state)));
    let lease = weak.upgrade().unwrap();
    assert!(Arc::ptr_eq(&lease.jobs(), &state.terminal_jobs));
    let cloned_value = Arc::new(state.as_ref().clone());
    let principal = interface_runtime::UserPrincipal::server_delegation(
        domain::ActorContext::root(Uuid::nil(), state.bootstrap_workspace_id, "root"),
    );
    drop(state);
    assert!(
        weak.upgrade().is_some(),
        "the actual typed lease retains the original allocation"
    );
    drop(lease);
    assert!(
        weak.upgrade().is_none(),
        "a cloned ApiState value must not retain a different Arc allocation"
    );
    let error = adapter
        .test_run(&principal, TerminalInput::Info(InfoInput {}))
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error
            .0
            .downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::InvalidInput(
            "terminal_host_unavailable"
        ))
    ));
    let cloned_adapter = TerminalAdapter::test_from_weak(Arc::downgrade(&cloned_value));
    let weak_clone = cloned_adapter.test_host();
    assert!(!std::ptr::addr_eq(weak_clone.as_ptr(), weak.as_ptr()));
    let (entered, started) = tokio::sync::oneshot::channel();
    let held = weak_clone.upgrade().unwrap();
    let task = tokio::spawn(async move {
        let _same_host_lease = held;
        entered.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    started.await.unwrap();
    drop(cloned_value);
    assert!(
        weak_clone.upgrade().is_some(),
        "a live async lease owns the same concrete host"
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(
        weak_clone.upgrade().is_none(),
        "aborting the async owner releases the typed host lease"
    );
}
