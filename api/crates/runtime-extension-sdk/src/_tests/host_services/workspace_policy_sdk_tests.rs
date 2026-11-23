use super::*;
use extension_contracts::{WorkspaceOnboardingModeDto, WorkspacePolicyRequest};
use serde_json::json;

#[test]
fn routes_only_the_policy_handler_and_unwraps_managed_payload() {
    let mut call = json!({"method":"execute","input":{"handler":"resolve-policy","input_payload":{"operation":"inspect_public_policy"}}});
    assert!(matches!(
        workspace_policy_input(&call, "resolve-policy").unwrap(),
        Some(WorkspacePolicyRequest::InspectPublicPolicy)
    ));
    assert!(
        workspace_policy_input(&call, "managed-service")
            .unwrap()
            .is_none()
    );
    call["input"]["input_payload"]["subject"] = json!("forged");
    assert!(workspace_policy_input(&call, "resolve-policy").is_err());
    assert_eq!(
        workspace_policy_output(WorkspacePolicyResponse::PublicPolicy {
            onboarding_mode: WorkspaceOnboardingModeDto::InviteOnly
        })["result"]["onboarding_mode"],
        "invite_only"
    );
}
