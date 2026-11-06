use super::*;

#[tokio::test]
async fn event_catalog_preserves_bound_and_unbound_views_without_retaining_boot() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor = domain::ActorContext::root(Uuid::now_v7(), domain::DEFAULT_SCOPE_ID, "root");
    let principal = UserPrincipal::server_delegation(actor.clone());
    let boot = state.extension_boot_snapshot.as_ref().unwrap();
    let probe = Arc::downgrade(boot);
    let initial_owners = Arc::strong_count(boot);
    let dependencies = Dependencies::new(&state);
    assert_eq!(
        Arc::strong_count(boot),
        initial_owners,
        "registry descendants must not own Boot"
    );
    let registered = crate::event_center::catalog::registered_events(Some(boot), &actor);
    let expected = EventCenterConfigurationService::with_registered_events(
        state.store.clone(),
        actor.current_workspace_id,
        registered,
    )
    .catalog(&actor)
    .await
    .unwrap();
    let Output(value) = dependencies
        .execute(&principal, Input::Catalog)
        .await
        .unwrap_or_else(|error| panic!("catalog failed: {:?}", error.0));
    assert_eq!(value, serde_json::to_value(expected).unwrap());

    let mut unbound_state = (*state).clone();
    unbound_state.extension_boot_snapshot = None;
    let unbound = Dependencies::new(&unbound_state);
    let expected = EventCenterConfigurationService::new(state.store.clone())
        .catalog(&actor)
        .await
        .unwrap();
    let Output(value) = unbound
        .execute(&principal, Input::Catalog)
        .await
        .unwrap_or_else(|error| panic!("unbound catalog failed: {:?}", error.0));
    assert_eq!(value, serde_json::to_value(expected).unwrap());
    drop(unbound_state);
    drop(state);
    assert!(
        probe.upgrade().is_none(),
        "event dependencies must not retain a retired generation"
    );
    let error = match dependencies.execute(&principal, Input::Catalog).await {
        Err(error) => error,
        Ok(_) => panic!("expired Boot must not look like an intentionally unbound catalog"),
    };
    let ConsoleInterfaceTargetError(ApiError(error)) = error;
    assert!(error.is::<crate::error_response::ApiServiceUnavailable>());
}
