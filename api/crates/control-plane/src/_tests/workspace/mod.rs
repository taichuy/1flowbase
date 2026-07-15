use crate::_tests::support::MemoryWorkspaceRepository;
use crate::errors::ControlPlaneError;
use crate::workspace::{UpdateWorkspaceCommand, WorkspaceService};
use domain::{ActorContext, WorkspaceRecord};
use uuid::Uuid;

fn workspace_record(workspace_id: Uuid) -> WorkspaceRecord {
    WorkspaceRecord {
        id: workspace_id,
        tenant_id: Uuid::now_v7(),
        name: "Core Workspace".to_string(),
        logo_url: None,
        introduction: "workspace intro".to_string(),
    }
}

fn workspace_update_policy() -> domain::RoleConsolePolicy {
    let group = domain::ConsolePolicyGroup::other("other.workspace")
        .expect("workspace policy group must be valid");
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            group,
            vec![domain::ConsoleOperationPolicy::simple(
                domain::ConsoleOperationId::try_from("workspace.update")
                    .expect("workspace update operation id must be valid"),
                true,
            )],
        )],
    )
}

fn update_command(actor: ActorContext, workspace_id: Uuid, name: &str) -> UpdateWorkspaceCommand {
    UpdateWorkspaceCommand {
        actor,
        workspace_id,
        name: name.to_string(),
        logo_url: Some("https://example.com/logo.png".to_string()),
        introduction: "workspace intro updated".to_string(),
    }
}

#[tokio::test]
async fn get_workspace_returns_not_found_for_unknown_id() {
    let repository = MemoryWorkspaceRepository::default();
    let error = WorkspaceService::new(repository)
        .get_workspace(Uuid::now_v7())
        .await
        .unwrap_err();

    let control_plane_error = error.downcast_ref::<ControlPlaneError>().unwrap();
    assert!(matches!(
        control_plane_error,
        ControlPlaneError::NotFound("workspace")
    ));
}

#[tokio::test]
async fn update_workspace_requires_console_policy() {
    let workspace = workspace_record(Uuid::now_v7());
    let repository = MemoryWorkspaceRepository::default();
    repository.upsert_workspace(workspace.clone()).await;

    let error = WorkspaceService::new(repository)
        .update_workspace(UpdateWorkspaceCommand {
            actor: ActorContext {
                user_id: Uuid::now_v7(),
                tenant_id: workspace.tenant_id,
                current_workspace_id: workspace.id,
                effective_display_role: "member".to_string(),
                is_root: false,
                permissions: Default::default(),
            },
            workspace_id: workspace.id,
            name: "Workspace Updated".to_string(),
            logo_url: Some("https://example.com/logo.png".to_string()),
            introduction: "workspace intro updated".to_string(),
        })
        .await
        .unwrap_err();

    let control_plane_error = error.downcast_ref::<ControlPlaneError>().unwrap();
    assert!(matches!(
        control_plane_error,
        ControlPlaneError::PermissionDenied("permission_denied")
    ));
}

#[tokio::test]
async fn ac_011_workspace_policy_only_allows_current_workspace_update_without_legacy_grant() {
    let workspace = workspace_record(Uuid::now_v7());
    let repository = MemoryWorkspaceRepository::default();
    repository.upsert_workspace(workspace.clone()).await;
    repository
        .set_console_policies(vec![workspace_update_policy()])
        .await;
    let actor = ActorContext {
        user_id: Uuid::now_v7(),
        tenant_id: workspace.tenant_id,
        current_workspace_id: workspace.id,
        effective_display_role: "member".to_string(),
        is_root: false,
        permissions: Default::default(),
    };

    let updated = WorkspaceService::new(repository)
        .update_workspace(update_command(actor, workspace.id, "Policy updated"))
        .await
        .unwrap();

    assert_eq!(updated.name, "Policy updated");
}

#[tokio::test]
async fn ac_011_workspace_legacy_grant_does_not_authorize_or_escape_current_workspace() {
    let workspace = workspace_record(Uuid::now_v7());
    let foreign_workspace = workspace_record(Uuid::now_v7());
    let repository = MemoryWorkspaceRepository::default();
    repository.upsert_workspace(workspace.clone()).await;
    repository.upsert_workspace(foreign_workspace.clone()).await;
    let legacy_actor = ActorContext {
        user_id: Uuid::now_v7(),
        tenant_id: workspace.tenant_id,
        current_workspace_id: workspace.id,
        effective_display_role: "member".to_string(),
        is_root: false,
        permissions: ["workspace.configure.all".to_string()]
            .into_iter()
            .collect(),
    };
    let service = WorkspaceService::new(repository.clone());

    assert!(service
        .update_workspace(update_command(
            legacy_actor.clone(),
            workspace.id,
            "Legacy denied",
        ))
        .await
        .is_err());

    repository
        .set_console_policies(vec![workspace_update_policy()])
        .await;
    assert!(service
        .update_workspace(update_command(
            legacy_actor,
            foreign_workspace.id,
            "Foreign denied",
        ))
        .await
        .is_err());
}
