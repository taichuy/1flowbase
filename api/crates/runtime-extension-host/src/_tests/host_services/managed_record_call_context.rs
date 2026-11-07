use super::actor_workspace_context;
use runtime_core::runtime_backend::RuntimeExecutionPrincipal;

#[test]
fn resource_replay_retains_worker_scope_without_restoring_recursive_callback_delegation() {
    let principal = RuntimeExecutionPrincipal {
        caller: None,
        actor_id: Some("real-actor".into()),
        workspace_id: "system-data-scope".into(),
        deadline_unix_ms: 123,
        record_permissions: Some(extension_contracts::PluginDataRecordPermissionContext {
            caller_workspace_id: "caller-workspace".into(),
            actor_user_id: "real-actor".into(),
            active_role_code: "member".into(),
            is_root: false,
            resources: vec![],
        }),
    };
    let context = actor_workspace_context(&principal).unwrap();
    assert_eq!(context.actor_id, "real-actor");
    assert_eq!(context.workspace_id, "caller-workspace");
    assert!(
        principal.caller.is_none(),
        "recursive file import callback must remain unavailable"
    );
    let mut mismatched = principal.clone();
    mismatched.actor_id = Some("forged".into());
    assert!(actor_workspace_context(&mismatched).is_err());
    mismatched.record_permissions = None;
    assert!(actor_workspace_context(&mismatched).is_err());
}
