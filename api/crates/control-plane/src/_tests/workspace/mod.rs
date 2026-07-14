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
async fn update_workspace_requires_workspace_configure_permission() {
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
