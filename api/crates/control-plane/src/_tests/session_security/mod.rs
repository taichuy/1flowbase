use crate::_tests::support::{password_hash, MemoryAuthRepository, MemorySessionStore};
use crate::errors::ControlPlaneError;
use crate::session_security::{
    ChangeOwnPasswordCommand, RevokeAllSessionsCommand, SessionSecurityService,
};
use control_plane::ports::SessionStore;
use domain::{BoundRole, RoleScopeKind, SessionRecord, UserRecord, UserStatus};
use uuid::Uuid;

fn test_user() -> UserRecord {
    UserRecord {
        id: Uuid::now_v7(),
        account: "root".to_string(),
        email: "root@example.com".to_string(),
        phone: None,
        password_hash: password_hash("change-me"),
        name: "Root".to_string(),
        nickname: "Root".to_string(),
        avatar_url: None,
        introduction: String::new(),
        preferred_locale: None,
        meta: serde_json::json!({}),
        default_display_role: Some("root".to_string()),
        email_login_enabled: true,
        phone_login_enabled: false,
        status: UserStatus::Active,
        session_version: 1,
        roles: vec![BoundRole {
            code: "root".to_string(),
            name: "Root".to_string(),
            scope_kind: RoleScopeKind::System,
            workspace_id: None,
        }],
    }
}

fn test_session(user_id: Uuid) -> SessionRecord {
    SessionRecord {
        session_id: "session-current".to_string(),
        user_id,
        tenant_id: Uuid::nil(),
        current_workspace_id: Uuid::nil(),
        active_role_code: "member".into(),
        session_version: 1,
        csrf_token: "csrf-token".to_string(),
        expires_at_unix: 1_800_000_000,
    }
}

#[tokio::test]
async fn change_password_rejects_wrong_old_password() {
    let repository = MemoryAuthRepository::new(test_user());
    let session_store = MemorySessionStore::default();
    let service = SessionSecurityService::new(repository.clone(), session_store);

    let error = service
        .change_own_password(ChangeOwnPasswordCommand {
            actor_user_id: repository.user().id,
            session_id: "session-current".to_string(),
            old_password: "wrong-password".to_string(),
            new_password_hash: "new-password-hash".to_string(),
        })
        .await
        .unwrap_err();

    let control_plane_error = error.downcast_ref::<ControlPlaneError>().unwrap();
    assert!(matches!(
        control_plane_error,
        ControlPlaneError::InvalidInput("old_password")
    ));
}

#[tokio::test]
async fn change_password_updates_hash_and_deletes_current_session() {
    let repository = MemoryAuthRepository::new(test_user());
    let session_store = MemorySessionStore::default();
    let service = SessionSecurityService::new(repository.clone(), session_store.clone());
    let session = test_session(repository.user().id);
    session_store.put(session.clone()).await.unwrap();

    service
        .change_own_password(ChangeOwnPasswordCommand {
            actor_user_id: repository.user().id,
            session_id: session.session_id.clone(),
            old_password: "change-me".to_string(),
            new_password_hash: "new-password-hash".to_string(),
        })
        .await
        .unwrap();

    let user = repository.user();
    assert_eq!(user.password_hash, "new-password-hash");
    assert_eq!(user.session_version, 2);
    assert_eq!(
        session_store.deleted_session_ids(),
        vec![session.session_id]
    );
    assert_eq!(repository.audit_events(), vec!["user.password_changed"]);
}

#[tokio::test]
async fn revoke_all_bumps_session_version_and_deletes_current_session() {
    let repository = MemoryAuthRepository::new(test_user());
    let session_store = MemorySessionStore::default();
    let service = SessionSecurityService::new(repository.clone(), session_store.clone());
    let session = test_session(repository.user().id);
    session_store.put(session.clone()).await.unwrap();

    service
        .revoke_all_sessions(RevokeAllSessionsCommand {
            actor_user_id: repository.user().id,
            session_id: session.session_id.clone(),
        })
        .await
        .unwrap();

    assert_eq!(repository.user().session_version, 2);
    assert_eq!(
        repository.bump_session_version_calls(),
        vec![(repository.user().id, repository.user().id)]
    );
    assert_eq!(
        session_store.deleted_session_ids(),
        vec![session.session_id]
    );
    assert_eq!(repository.audit_events(), vec!["session.revoke_all"]);
}
