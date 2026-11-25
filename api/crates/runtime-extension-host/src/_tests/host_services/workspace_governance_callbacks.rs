use super::*;
use extension_contracts::*;
use serde_json::json;
use std::sync::Mutex;

struct Governance(Mutex<Vec<WorkspaceGovernanceBinding>>);
impl WorkspaceGovernancePort for Governance {
    fn execute<'a>(
        &'a self,
        binding: &'a WorkspaceGovernanceBinding,
        request: &'a WorkspaceGovernanceRequest,
    ) -> WorkspaceGovernanceFuture<'a> {
        Box::pin(async move {
            assert!(!request.requires_manage());
            self.0.lock().unwrap().push(binding.clone());
            if matches!(request, WorkspaceGovernanceRequest::InspectCallerContext) {
                return Ok(WorkspaceGovernanceResponse::CallerContext {
                    caller: binding.caller.clone(),
                });
            }
            Ok(WorkspaceGovernanceResponse::QuotaCounts {
                active_member_count: 2,
                tenant_workspace_count: 3,
                workspace_application_count: 4,
            })
        })
    }
}

#[tokio::test]
async fn workspace_governance_callback_does_not_accept_worker_authority() {
    let mut context = super::credential_tests::context(Arc::new(
        super::credential_tests::Credentials(Mutex::new(vec![])),
    ));
    let port = Arc::new(Governance(Mutex::new(vec![])));
    context.workspace_governance = Some((
        WorkspaceGovernanceBinding {
            subject: ManagedContributionSubject::new(
                ManagedInstallationId::new("installation").unwrap(),
                ManagedWorkspaceId::new("system").unwrap(),
                ContributionId::new("workspace-policy").unwrap(),
            ),
            publisher_namespace: "host-bound".into(),
            plugin_code: "workspaces".into(),
            plugin_version: "1.0.0".into(),
            caller: Some(WorkspaceGovernanceCallerDto {
                user_id: "admin-user".into(),
                tenant_id: "workspace-tenant".into(),
                current_workspace_id: "current".into(),
                is_root: false,
                effective_display_role: "admin".into(),
            }),
            can_read: true,
            can_manage: false,
            deadline_unix_ms: now_unix_ms() + 1000,
        },
        port.clone(),
    ));
    let request =
        json!({"operation":"inspect_quota_counts","tenant_id":"tenant","workspace_id":"workspace"});
    let denied = execute_callback(
        MultiplexHostService::WorkspaceGovernanceV1,
        request.clone(),
        None,
    )
    .await;
    assert_eq!(denied["error"]["code"], "workspace_governance_not_granted");
    let mut forged = request.clone();
    forged["can_manage"] = json!(true);
    let invalid = execute_callback(
        MultiplexHostService::WorkspaceGovernanceV1,
        forged,
        Some(context.clone()),
    )
    .await;
    assert_eq!(
        invalid["error"]["code"],
        "workspace_governance_request_invalid"
    );
    let response = execute_callback(
        MultiplexHostService::WorkspaceGovernanceV1,
        request,
        Some(context.clone()),
    )
    .await;
    assert_eq!(response["result"]["workspace_application_count"], 4);
    let forged = execute_callback(
        MultiplexHostService::WorkspaceGovernanceV1,
        json!({"operation":"inspect_caller_context","caller":{"is_root":true}}),
        Some(context.clone()),
    )
    .await;
    assert_eq!(
        forged["error"]["code"],
        "workspace_governance_request_invalid"
    );
    let caller = execute_callback(
        MultiplexHostService::WorkspaceGovernanceV1,
        json!({"operation":"inspect_caller_context"}),
        Some(context),
    )
    .await;
    assert_eq!(caller["result"]["caller"]["is_root"], false);
    assert_eq!(caller["result"]["caller"]["user_id"], "admin-user");
    let seen = port.0.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].publisher_namespace, "host-bound");
    assert!(!seen[0].can_manage);
}
