use domain::{
    password_local_identity_claims, ActorContext, BoundRole, RoleScopeKind, UserRecord, UserStatus,
    AUTH_SUBJECT_TYPE_ACCOUNT, AUTH_SUBJECT_TYPE_EMAIL, AUTH_SUBJECT_TYPE_PHONE,
    PASSWORD_LOCAL_AUTHENTICATOR_ID,
};
use uuid::Uuid;

fn sample_user(default_display_role: Option<&str>, roles: &[&str]) -> UserRecord {
    UserRecord {
        id: Uuid::now_v7(),
        account: "root".into(),
        email: "root@example.com".into(),
        phone: None,
        password_hash: "hash".into(),
        name: "Root".into(),
        nickname: "Root".into(),
        avatar_url: None,
        introduction: String::new(),
        preferred_locale: None,
        meta: serde_json::json!({}),
        default_display_role: default_display_role.map(str::to_string),
        email_login_enabled: true,
        phone_login_enabled: false,
        status: UserStatus::Active,
        session_version: 1,
        roles: roles
            .iter()
            .map(|code| BoundRole {
                code: (*code).into(),
                scope_kind: RoleScopeKind::Workspace,
                workspace_id: Some(Uuid::nil()),
            })
            .collect(),
    }
}

#[test]
fn resolved_display_role_falls_back_to_first_bound_role() {
    let user = sample_user(Some("deleted-role"), &["manager", "admin"]);

    assert_eq!(user.resolved_display_role().as_deref(), Some("manager"));
}

#[test]
fn root_actor_short_circuits_permission_checks() {
    let actor = ActorContext::root(Uuid::now_v7(), Uuid::now_v7(), "root");

    assert!(actor.has_permission("role_permission.manage.all"));
}

#[test]
fn password_local_identity_claims_cover_account_email_and_phone() {
    let claims = password_local_identity_claims("alice", "alice@example.com", Some("18800001111"));

    assert_eq!(claims.len(), 3);
    assert!(claims.iter().all(|claim| {
        claim.authenticator_id == PASSWORD_LOCAL_AUTHENTICATOR_ID && claim.verified
    }));
    assert_eq!(claims[0].subject_type, AUTH_SUBJECT_TYPE_ACCOUNT);
    assert_eq!(claims[0].subject_value, "alice");
    assert_eq!(claims[1].subject_type, AUTH_SUBJECT_TYPE_EMAIL);
    assert_eq!(claims[1].subject_value, "alice@example.com");
    assert_eq!(claims[2].subject_type, AUTH_SUBJECT_TYPE_PHONE);
    assert_eq!(claims[2].subject_value, "18800001111");
}
