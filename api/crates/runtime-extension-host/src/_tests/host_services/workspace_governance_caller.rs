use super::workspace_governance_caller;
use runtime_core::runtime_backend::{RuntimeCallerContext, RuntimeExecutionPrincipal};
use serde_json::json;
use uuid::Uuid;

#[test]
fn workspace_governance_caller_requires_matching_authenticated_identity() {
    let user = Uuid::now_v7();
    let tenant = Uuid::now_v7();
    let workspace = Uuid::now_v7();
    let mut principal = RuntimeExecutionPrincipal {
        record_permissions: None,
        caller: None,
        workspace_id: "system-scope".into(),
        actor_id: Some(user.to_string()),
        deadline_unix_ms: i64::MAX,
    };
    assert!(workspace_governance_caller(&principal).is_none());
    principal.caller = Some(RuntimeCallerContext {
        actor: serde_json::from_value(json!({
            "user_id": user, "tenant_id": tenant, "current_workspace_id": workspace,
            "is_root": false, "effective_display_role": "admin", "permissions": []
        }))
        .unwrap(),
        session_version: 1,
        session_identity: None,
        interface_id: "service.open".into(),
        selected_mcp_instance_ids: None,
    });
    let summary = workspace_governance_caller(&principal).unwrap();
    assert_eq!(summary.user_id, user.to_string());
    assert_eq!(summary.tenant_id, tenant.to_string());
    assert_eq!(summary.current_workspace_id, workspace.to_string());
    assert_eq!(summary.effective_display_role, "admin");
    assert!(!summary.is_root);
    let actor = &mut principal.caller.as_mut().unwrap().actor;
    actor.is_root = true;
    actor.effective_display_role = "root".into();
    assert!(workspace_governance_caller(&principal).unwrap().is_root);
    principal.actor_id = Some(Uuid::now_v7().to_string());
    assert!(workspace_governance_caller(&principal).is_none());
    principal.actor_id = None;
    assert!(workspace_governance_caller(&principal).is_none());
}
