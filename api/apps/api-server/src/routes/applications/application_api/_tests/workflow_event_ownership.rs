use super::*;

fn unavailable(result: Result<WorkflowEventOutput, ConsoleInterfaceTargetError>) -> bool {
    matches!(result, Err(ConsoleInterfaceTargetError(ApiError(error)))
        if error.is::<crate::error_response::ApiServiceUnavailable>())
}

#[tokio::test]
async fn workflow_event_adapter_borrows_generation_and_rejects_retired_owner() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let probe = Arc::downgrade(boot);
    let owners = Arc::strong_count(boot);
    let bound = port(&state);
    assert_eq!(
        Arc::strong_count(boot),
        owners,
        "registry descendants must not own Boot"
    );

    let principal = UserPrincipal::server_delegation(domain::ActorContext::root(
        Uuid::now_v7(),
        domain::DEFAULT_SCOPE_ID,
        "root",
    ));
    let request = || WorkflowEventInput::Get {
        application_id: Uuid::nil(),
    };
    // A missing application may be rejected by business authorization; a live
    // generation must reach that boundary rather than fail generation admission.
    assert!(!unavailable(bound.execute(&principal, request()).await));

    let mut unbound_state = (*state).clone();
    unbound_state.extension_boot_snapshot = None;
    let unbound = port(&unbound_state);
    assert!(!unavailable(unbound.execute(&principal, request()).await));
    drop(unbound_state);
    drop(state);
    assert!(
        probe.upgrade().is_none(),
        "workflow adapter must release retired Boot"
    );
    assert!(unavailable(bound.execute(&principal, request()).await));
}
