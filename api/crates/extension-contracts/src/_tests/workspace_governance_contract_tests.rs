use super::*;
use serde_json::json;

#[test]
fn governance_wire_rejects_identity_and_nested_unknown_fields() {
    for request in [
        json!({"operation":"list_tenants_and_workspaces", "can_manage":true}),
        json!({"operation":"lookup_user", "by":{"kind":"user_id", "user_id":"u", "subject":"forged"}}),
    ] {
        assert!(serde_json::from_value::<WorkspaceGovernanceRequest>(request).is_err());
    }
    assert!(!WorkspaceGovernanceRequest::ListTenantsAndWorkspaces.requires_manage());
    assert!(
        WorkspaceGovernanceRequest::SetTenantStatus {
            tenant_id: "t".into(),
            status: "active".into(),
            actor_user_id: None
        }
        .requires_manage()
    );
}

#[test]
fn policy_wire_preserves_redemption_rollback_and_rejects_unknown_fields() {
    let request = json!({"operation":"finalize_invitation_redemption","redemption_id":"r","committed_user_id":null});
    let typed: crate::WorkspacePolicyRequest = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), request);
    assert!(
        serde_json::from_value::<crate::WorkspacePolicyRequest>(
            json!({"operation":"inspect_public_policy","actor_id":"forged"})
        )
        .is_err()
    );
}

#[test]
fn workspace_governance_caller_is_a_strict_read_only_projection() {
    let request = json!({"operation":"inspect_caller_context"});
    let typed = serde_json::from_value::<WorkspaceGovernanceRequest>(request.clone()).unwrap();
    assert!(!typed.requires_manage());
    assert_eq!(serde_json::to_value(typed).unwrap(), request);
    assert!(
        serde_json::from_value::<WorkspaceGovernanceRequest>(json!({
            "operation":"inspect_caller_context", "caller":{"is_root":true}
        }))
        .is_err()
    );
    let mut response = json!({"operation":"caller_context","caller":null});
    assert!(matches!(
        serde_json::from_value::<WorkspaceGovernanceResponse>(response.clone()).unwrap(),
        WorkspaceGovernanceResponse::CallerContext { caller: None }
    ));
    response["caller"] = json!({"user_id":"u","tenant_id":"t","current_workspace_id":"w","effective_display_role":"root","is_root":true,"session_id":"forbidden"});
    assert!(serde_json::from_value::<WorkspaceGovernanceResponse>(response).is_err());
}
