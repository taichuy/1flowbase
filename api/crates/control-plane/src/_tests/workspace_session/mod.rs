use crate::_tests::support::{MemoryAuthRepository, MemorySessionStore, MemoryWorkspaceRepository};
use crate::errors::ControlPlaneError;
use crate::workspace_session::{SwitchWorkspaceCommand, WorkspaceSessionService};
use control_plane::ports::SessionStore;
use domain::{BoundRole, RoleScopeKind, SessionRecord, UserRecord, UserStatus, WorkspaceRecord};
use uuid::Uuid;

fn test_workspace(tenant_id: Uuid, workspace_id: Uuid, name: &str) -> WorkspaceRecord {
    WorkspaceRecord {
        id: workspace_id,
        tenant_id,
        name: name.to_string(),
        logo_url: None,
        introduction: String::new(),
    }
}

fn test_user(source_workspace_id: Uuid, target_workspace_id: Uuid) -> UserRecord {
    UserRecord {
        id: Uuid::now_v7(),
        account: "member".to_string(),
        email: "manager@example.com".to_string(),
        phone: None,
        password_hash: "hash".to_string(),
        name: "Manager".to_string(),
        nickname: "Manager".to_string(),
        avatar_url: None,
        introduction: String::new(),
        preferred_locale: None,
        meta: serde_json::json!({}),
        default_display_role: Some("admin".to_string()),
        email_login_enabled: true,
        phone_login_enabled: false,
        status: UserStatus::Active,
        session_version: 3,
        roles: vec![
            BoundRole {
                code: "admin".to_string(),
                scope_kind: RoleScopeKind::Workspace,
                workspace_id: Some(source_workspace_id),
            },
            BoundRole {
                code: "member".to_string(),
                scope_kind: RoleScopeKind::Workspace,
                workspace_id: Some(target_workspace_id),
            },
        ],
    }
}

fn test_session(user_id: Uuid, tenant_id: Uuid, workspace_id: Uuid) -> SessionRecord {
    SessionRecord {
        session_id: "session-current".to_string(),
        user_id,
        tenant_id,
        current_workspace_id: workspace_id,
        session_version: 7,
        csrf_token: "csrf-before".to_string(),
        expires_at_unix: 1_900_000_000,
    }
}

#[tokio::test]
async fn switch_workspace_rewrites_session_scope_and_rotates_csrf() {
    let tenant_id = Uuid::now_v7();
    let source_workspace_id = Uuid::now_v7();
    let target_workspace_id = Uuid::now_v7();
    let source_workspace = test_workspace(tenant_id, source_workspace_id, "Source Workspace");
    let target_workspace = test_workspace(tenant_id, target_workspace_id, "Target Workspace");
    let user = test_user(source_workspace_id, target_workspace_id);
    let repository = MemoryAuthRepository::new(user.clone());
    let team_repository = MemoryWorkspaceRepository::default();
    let session_store = MemorySessionStore::default();
    let session = test_session(user.id, tenant_id, source_workspace_id);

    team_repository
        .set_accessible_workspaces(user.id, vec![source_workspace, target_workspace])
        .await;
    session_store.put(session.clone()).await.unwrap();

    let result =
        WorkspaceSessionService::new(repository.clone(), team_repository, session_store.clone())
            .switch_workspace(SwitchWorkspaceCommand {
                actor_user_id: user.id,
                session_id: session.session_id.clone(),
                target_workspace_id,
            })
            .await
            .unwrap();

    assert_eq!(result.session.session_id, session.session_id);
    assert_eq!(result.session.tenant_id, tenant_id);
    assert_eq!(result.session.current_workspace_id, target_workspace_id);
    assert_eq!(result.actor.current_workspace_id, target_workspace_id);
    assert_eq!(result.actor.effective_display_role, "member");
    assert_ne!(result.session.csrf_token, session.csrf_token);
    assert_eq!(result.session.expires_at_unix, session.expires_at_unix);
    assert_eq!(repository.audit_events(), vec!["session.switch_workspace"]);
}

#[tokio::test]
async fn switch_workspace_writes_workspace_id_into_audit_log() {
    let tenant_id = Uuid::now_v7();
    let source_workspace_id = Uuid::now_v7();
    let target_workspace_id = Uuid::now_v7();
    let source_workspace = test_workspace(tenant_id, source_workspace_id, "Source Workspace");
    let target_workspace = test_workspace(tenant_id, target_workspace_id, "Target Workspace");
    let user = test_user(source_workspace_id, target_workspace_id);
    let repository = MemoryAuthRepository::new(user.clone());
    let team_repository = MemoryWorkspaceRepository::default();
    let session_store = MemorySessionStore::default();
    let session = test_session(user.id, tenant_id, source_workspace_id);

    team_repository
        .set_accessible_workspaces(user.id, vec![source_workspace, target_workspace])
        .await;
    session_store.put(session.clone()).await.unwrap();

    WorkspaceSessionService::new(repository.clone(), team_repository, session_store)
        .switch_workspace(SwitchWorkspaceCommand {
            actor_user_id: user.id,
            session_id: session.session_id,
            target_workspace_id,
        })
        .await
        .unwrap();

    let audit_log = repository
        .audit_logs()
        .into_iter()
        .last()
        .expect("switch workspace should write audit log");

    assert_eq!(audit_log.workspace_id, Some(target_workspace_id));
    assert_eq!(audit_log.actor_user_id, Some(user.id));
    assert_eq!(audit_log.event_code, "session.switch_workspace");
}

#[tokio::test]
async fn switch_workspace_rejects_inaccessible_target_for_non_root() {
    let tenant_id = Uuid::now_v7();
    let source_workspace_id = Uuid::now_v7();
    let blocked_workspace_id = Uuid::now_v7();
    let source_workspace = test_workspace(tenant_id, source_workspace_id, "Source Workspace");
    let user = test_user(source_workspace_id, blocked_workspace_id);
    let repository = MemoryAuthRepository::new(user.clone());
    let team_repository = MemoryWorkspaceRepository::default();
    let session_store = MemorySessionStore::default();
    let session = test_session(user.id, tenant_id, source_workspace_id);

    team_repository
        .set_accessible_workspaces(user.id, vec![source_workspace])
        .await;
    session_store.put(session.clone()).await.unwrap();

    let error = WorkspaceSessionService::new(repository, team_repository, session_store)
        .switch_workspace(SwitchWorkspaceCommand {
            actor_user_id: user.id,
            session_id: session.session_id,
            target_workspace_id: blocked_workspace_id,
        })
        .await
        .unwrap_err();

    let control_plane_error = error.downcast_ref::<ControlPlaneError>().unwrap();
    assert!(matches!(
        control_plane_error,
        ControlPlaneError::PermissionDenied("workspace_access_denied")
    ));
}

#[tokio::test]
async fn switch_workspace_keeps_session_id_and_expiry() {
    let tenant_id = Uuid::now_v7();
    let source_workspace_id = Uuid::now_v7();
    let target_workspace_id = Uuid::now_v7();
    let source_workspace = test_workspace(tenant_id, source_workspace_id, "Source Workspace");
    let target_workspace = test_workspace(tenant_id, target_workspace_id, "Target Workspace");
    let user = test_user(source_workspace_id, target_workspace_id);
    let repository = MemoryAuthRepository::new(user.clone());
    let team_repository = MemoryWorkspaceRepository::default();
    let session_store = MemorySessionStore::default();
    let session = test_session(user.id, tenant_id, source_workspace_id);

    team_repository
        .set_accessible_workspaces(user.id, vec![source_workspace, target_workspace])
        .await;
    session_store.put(session.clone()).await.unwrap();

    let result = WorkspaceSessionService::new(repository, team_repository, session_store.clone())
        .switch_workspace(SwitchWorkspaceCommand {
            actor_user_id: user.id,
            session_id: session.session_id.clone(),
            target_workspace_id,
        })
        .await
        .unwrap();

    let persisted = session_store
        .get(&session.session_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(result.session.session_id, session.session_id);
    assert_eq!(result.session.session_version, session.session_version);
    assert_eq!(result.session.expires_at_unix, session.expires_at_unix);
    assert_eq!(persisted.session_id, session.session_id);
    assert_eq!(persisted.expires_at_unix, session.expires_at_unix);
    assert_eq!(session_store.deleted_session_ids(), Vec::<String>::new());
}
