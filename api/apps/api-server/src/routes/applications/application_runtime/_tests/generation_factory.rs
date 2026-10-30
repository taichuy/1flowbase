use super::*;

#[tokio::test]
async fn expired_debug_state_factory_returns_error() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let factory = StateRuntimeDebugMcpFactory {
        state: Arc::downgrade(&state),
    };
    let actor = domain::ActorContext::root(Uuid::nil(), state.bootstrap_workspace_id, "root");
    drop(state);
    assert!(factory.for_actor(&actor).await.is_err());
}
