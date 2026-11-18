use super::*;
use extension_contracts::{
    ContributionId, ManagedContributionSubject, ManagedInstallationId, ManagedWorkspaceId,
    PluginExecutionGateAuthority, PluginExecutionGateAuthorityFuture, PluginExecutionGateBinding,
};
use serde_json::json;
struct Authority;
impl PluginExecutionGateAuthority for Authority {
    fn authorize<'a>(
        &'a self,
        _: &'a PluginExecutionGateBinding,
    ) -> PluginExecutionGateAuthorityFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}
fn context(registry: crate::execution_gate_host::ExecutionGateRegistry) -> ProviderHostCallContext {
    let mut context = super::credential_tests::context(Arc::new(
        super::credential_tests::Credentials(std::sync::Mutex::new(vec![])),
    ));
    context.execution_gate = Some(crate::execution_gate_host::ExecutionGateCall::new(
        registry,
        PluginExecutionGateBinding {
            subject: ManagedContributionSubject::new(
                ManagedInstallationId::new("installation").unwrap(),
                ManagedWorkspaceId::new("system").unwrap(),
                ContributionId::new("source").unwrap(),
            ),
            publisher_namespace: "trusted".into(),
            plugin_code: "fixture".into(),
            plugin_version: "1".into(),
            workspace_id: "workspace".into(),
            deadline_unix_ms: i64::MAX,
        },
        Arc::new(Authority),
    ));
    context
}
fn active(context: ProviderHostCallContext) -> ActiveCall {
    ActiveCall {
        events: mpsc::unbounded_channel().0,
        host_calls: Some(context),
        callbacks: HashMap::new(),
        max_callback_id: 0,
        cancelling_since: None,
        written: false,
        pending_acks: Default::default(),
        max_event_id: 0,
        live: Arc::new(AtomicBool::new(true)),
        _lease: None,
    }
}
#[tokio::test]
async fn execution_gate_dispatch_requires_context_and_call_cleanup_closes_retained_clones() {
    let request = json!({"operation":"acquire","key":"entry"});
    let denied = execute_callback(
        MultiplexHostService::PluginExecutionGateV1,
        request.clone(),
        None,
    )
    .await;
    assert_eq!(denied["error"]["code"], "execution_gate_not_granted");
    let registry = crate::execution_gate_host::ExecutionGateRegistry::default();
    for cancel in [true, false] {
        let ctx = context(registry.clone());
        let retained = ctx.clone();
        let mut active = active(ctx.clone());
        let result = execute_callback(
            MultiplexHostService::PluginExecutionGateV1,
            request.clone(),
            Some(ctx),
        )
        .await;
        assert!(result["result"]["token"].is_string());
        if cancel {
            request_cancel("1", &mut active, &mpsc::channel(1).0);
        }
        drop(active); // Normal delimiter, crash and drain all drop this same owner.
        let closed = execute_callback(
            MultiplexHostService::PluginExecutionGateV1,
            request.clone(),
            Some(retained),
        )
        .await;
        assert_eq!(closed["error"]["code"], "execution_gate_call_closed");
        let follower = context(registry.clone());
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            execute_callback(
                MultiplexHostService::PluginExecutionGateV1,
                request.clone(),
                Some(follower.clone()),
            ),
        )
        .await
        .unwrap();
        assert!(result["result"]["token"].is_string());
        follower.execution_gate.as_ref().unwrap().close();
    }
}
