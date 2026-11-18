use super::*;
use extension_contracts::{
    ContributionId, ManagedContributionSubject, ManagedInstallationId, ManagedWorkspaceId,
};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

struct Authority {
    allowed: AtomicBool,
    checked: AtomicUsize,
}
impl PluginExecutionGateAuthority for Authority {
    fn authorize<'a>(
        &'a self,
        _: &'a PluginExecutionGateBinding,
    ) -> extension_contracts::PluginExecutionGateAuthorityFuture<'a> {
        Box::pin(async move {
            self.checked.fetch_add(1, Ordering::SeqCst);
            if self.allowed.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(error("execution_gate_authority_denied"))
            }
        })
    }
}
fn authority() -> Arc<Authority> {
    Arc::new(Authority {
        allowed: AtomicBool::new(true),
        checked: AtomicUsize::new(0),
    })
}
fn call(
    registry: &ExecutionGateRegistry,
    authority: Arc<Authority>,
    installation: &str,
    workspace: &str,
) -> ExecutionGateCall {
    ExecutionGateCall::new(
        registry.clone(),
        PluginExecutionGateBinding {
            subject: ManagedContributionSubject::new(
                ManagedInstallationId::new(installation).unwrap(),
                ManagedWorkspaceId::new("system").unwrap(),
                ContributionId::new("source").unwrap(),
            ),
            publisher_namespace: "trusted".into(),
            plugin_code: "fixture".into(),
            plugin_version: "1".into(),
            workspace_id: workspace.into(),
            deadline_unix_ms: i64::MAX,
        },
        authority,
    )
}
async fn acquire(call: &ExecutionGateCall, key: &str) -> String {
    match call
        .execute(json!({"operation":"acquire", "key":key}))
        .await
        .unwrap()
    {
        PluginExecutionGateResponse::Acquire { token } => token,
        _ => panic!("expected acquire"),
    }
}
async fn release(call: &ExecutionGateCall, key: &str, token: String) {
    assert_eq!(
        call.execute(json!({"operation":"release", "key":key, "token":token}))
            .await
            .unwrap(),
        PluginExecutionGateResponse::Release
    );
}
#[tokio::test]
async fn same_group_serializes_across_calls_distinct_groups_and_scopes_are_independent() {
    let registry = ExecutionGateRegistry::default();
    let authority = authority();
    let first = call(&registry, authority.clone(), "installation", "workspace");
    let second = call(&registry, authority.clone(), "installation", "workspace");
    let token = acquire(&first, "entry").await;
    let mut waiting = Box::pin(second.execute(json!({"operation":"acquire", "key":"entry"})));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut waiting)
            .await
            .is_err()
    );
    // Established work can continue in parallel after releasing its handshake permit.
    let other = acquire(&second, "other-entry").await;
    let isolated = call(
        &registry,
        authority.clone(),
        "other-installation",
        "workspace",
    );
    let isolated_token = acquire(&isolated, "entry").await;
    let other_workspace = call(&registry, authority, "installation", "other-workspace");
    let workspace_token = acquire(&other_workspace, "entry").await;
    release(&first, "entry", token).await;
    let PluginExecutionGateResponse::Acquire { token } =
        tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .unwrap()
            .unwrap()
    else {
        panic!()
    };
    release(&second, "entry", token).await;
    release(&second, "other-entry", other).await;
    release(&isolated, "entry", isolated_token).await;
    release(&other_workspace, "entry", workspace_token).await;
    assert!(registry.0.groups.lock().unwrap().is_empty());
}
#[tokio::test]
async fn failed_cancelled_and_dropped_waiters_do_not_block_followers_or_leak_registry() {
    let registry = ExecutionGateRegistry::default();
    let auth = authority();
    let first = call(&registry, auth.clone(), "installation", "workspace");
    let waiter = call(&registry, auth.clone(), "installation", "workspace");
    let token = acquire(&first, "entry").await;
    let mut pending = Box::pin(waiter.execute(json!({"operation":"acquire", "key":"entry"})));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut pending)
            .await
            .is_err()
    );
    drop(pending); // callback future cancellation removes Tokio's queue waiter.
    let mut pending = Box::pin(waiter.execute(json!({"operation":"acquire", "key":"entry"})));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut pending)
            .await
            .is_err()
    );
    waiter.close();
    assert_eq!(
        pending.await.unwrap_err().code,
        "execution_gate_call_closed"
    );
    let denied_auth = authority();
    let denied = call(&registry, denied_auth.clone(), "installation", "workspace");
    let mut pending = Box::pin(denied.execute(json!({"operation":"acquire", "key":"entry"})));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut pending)
            .await
            .is_err()
    );
    denied_auth.allowed.store(false, Ordering::SeqCst);
    release(&first, "entry", token).await;
    assert_eq!(
        pending.await.unwrap_err().code,
        "execution_gate_authority_denied"
    );
    let follower = call(&registry, auth, "installation", "workspace");
    let token = tokio::time::timeout(Duration::from_secs(1), acquire(&follower, "entry"))
        .await
        .unwrap();
    release(&follower, "entry", token).await;
    assert!(registry.0.groups.lock().unwrap().is_empty());
}
#[tokio::test]
async fn token_is_bound_to_exact_call_and_group_and_teardown_releases_held_permits() {
    let registry = ExecutionGateRegistry::default();
    let auth = authority();
    let first = call(&registry, auth.clone(), "installation", "workspace");
    let other = call(&registry, auth, "installation", "workspace");
    let token = acquire(&first, "entry").await;
    for (call, key) in [(&other, "entry"), (&first, "wrong-group")] {
        assert_eq!(
            call.execute(json!({"operation":"release","key":key,"token":token}))
                .await
                .unwrap_err()
                .code,
            "execution_gate_token_invalid"
        );
    }
    let retained_context = first.clone();
    first.close();
    let token = acquire(&other, "entry").await;
    assert_eq!(
        retained_context
            .execute(json!({"operation":"acquire","key":"elsewhere"}))
            .await
            .unwrap_err()
            .code,
        "execution_gate_call_closed"
    );
    drop(other); // process/crash/future owner loss has synchronous RAII cleanup.
    assert!(registry.0.groups.lock().unwrap().is_empty());
    assert!(!token.is_empty());
}
#[tokio::test]
async fn rejects_missing_authority_forged_identity_and_deadline_before_queue_access() {
    let registry = ExecutionGateRegistry::default();
    let auth = authority();
    let mut call = call(&registry, auth.clone(), "installation", "workspace");
    assert!(call
        .execute(json!({"operation":"acquire","key":"entry","workspace_id":"forged"}))
        .await
        .is_err());
    auth.allowed.store(false, Ordering::SeqCst);
    assert_eq!(
        call.execute(json!({"operation":"acquire","key":"entry"}))
            .await
            .unwrap_err()
            .code,
        "execution_gate_authority_denied"
    );
    call.binding.deadline_unix_ms = 0;
    assert_eq!(
        call.execute(json!({"operation":"acquire","key":"entry"}))
            .await
            .unwrap_err()
            .code,
        "execution_gate_deadline_exceeded"
    );
    assert!(registry.0.groups.lock().unwrap().is_empty());
}

#[tokio::test]
async fn execution_gate_queued_deadline_drops_waiter_and_follower_can_acquire() {
    let registry = ExecutionGateRegistry::default();
    let auth = authority();
    let first = call(&registry, auth.clone(), "installation", "workspace");
    let token = acquire(&first, "entry").await;
    let mut timed = call(&registry, auth.clone(), "installation", "workspace");
    timed.binding.deadline_unix_ms =
        i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap()
            + 15;
    let error = timed
        .execute(json!({"operation":"acquire", "key":"entry"}))
        .await
        .unwrap_err();
    assert_eq!(error.code, "execution_gate_deadline_exceeded");
    release(&first, "entry", token).await;
    let follower = call(&registry, auth, "installation", "workspace");
    let token = tokio::time::timeout(Duration::from_secs(1), acquire(&follower, "entry"))
        .await
        .unwrap();
    release(&follower, "entry", token).await;
    assert!(registry.0.groups.lock().unwrap().is_empty());
}

#[tokio::test]
async fn execution_gate_admitted_future_guard_releases_before_carrier_cleanup() {
    let registry = ExecutionGateRegistry::default();
    let auth = authority();
    let owner = call(&registry, auth.clone(), "installation", "workspace");
    let guard = owner.cleanup_guard();
    let _token = acquire(&owner, "entry").await;
    let retained = owner.clone();
    drop(guard);
    assert_eq!(
        retained
            .execute(json!({"operation":"acquire","key":"entry"}))
            .await
            .unwrap_err()
            .code,
        "execution_gate_call_closed"
    );
    let follower = call(&registry, auth, "installation", "workspace");
    let token = tokio::time::timeout(Duration::from_secs(1), acquire(&follower, "entry"))
        .await
        .unwrap();
    release(&follower, "entry", token).await;
    assert!(registry.0.groups.lock().unwrap().is_empty());
}

#[tokio::test]
async fn poisoned_call_state_denies_operations_and_cleanup_releases_permit() {
    let registry = ExecutionGateRegistry::default();
    let grant = authority();
    let active = call(&registry, grant.clone(), "installation", "workspace");
    acquire(&active, "group").await;
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _state = active.inner.state.lock().unwrap();
        panic!("poison active call state");
    }))
    .is_err());

    let result = active
        .execute(json!({"operation":"acquire", "key":"other-group"}))
        .await;
    assert_eq!(result.unwrap_err().code, "execution_gate_state_unavailable");
    active.close();
    assert!(*active.inner.closed.borrow());

    let follower = call(&registry, grant, "installation", "workspace");
    tokio::time::timeout(Duration::from_secs(1), acquire(&follower, "group"))
        .await
        .expect("cleanup releases the active call's permit even after poisoning");
}
