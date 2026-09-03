use domain::{
    password_local_identity_claims, ActorContext, BoundRole, ExternalIdentityClaim,
    LoginEntryRecord, RoleScopeKind, UserRecord, UserStatus, VerifiedExternalIdentity,
    AUTH_SUBJECT_TYPE_ACCOUNT, AUTH_SUBJECT_TYPE_EMAIL, AUTH_SUBJECT_TYPE_PHONE,
    BUILTIN_PASSWORD_LOGIN_ENTRY_ID, PASSWORD_LOCAL_CONNECTION_ID,
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
                name: (*code).into(),
                scope_kind: RoleScopeKind::Workspace,
                workspace_id: Some(Uuid::nil()),
            })
            .collect(),
    }
}

#[test]
fn resolved_display_role_falls_back_to_first_bound_role() {
    let user = sample_user(Some("deleted-role"), &["member", "admin"]);

    assert_eq!(user.resolved_display_role().as_deref(), Some("member"));
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
    assert!(claims
        .iter()
        .all(|claim| { claim.connection_id == PASSWORD_LOCAL_CONNECTION_ID && claim.verified }));
    assert_eq!(claims[0].subject_type, AUTH_SUBJECT_TYPE_ACCOUNT);
    assert_eq!(claims[0].subject_value, "alice");
    assert_eq!(claims[1].subject_type, AUTH_SUBJECT_TYPE_EMAIL);
    assert_eq!(claims[1].subject_value, "alice@example.com");
    assert_eq!(claims[2].subject_type, AUTH_SUBJECT_TYPE_PHONE);
    assert_eq!(claims[2].subject_value, "18800001111");
}

#[test]
fn login_entry_serialization_keeps_public_ui_block_as_first_class_truth() {
    // Issue #1444 AC-002/AC-009: public UI content must not be hidden inside
    // private options, because settings and the public projection have distinct DTOs.
    let login_entry = LoginEntryRecord {
        id: BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
        connection_id: PASSWORD_LOCAL_CONNECTION_ID,
        auth_type: "password-local".into(),
        title: "Password".into(),
        enabled: true,
        is_builtin: true,
        sort_order: 0,
        public_ui_block: "export default { main } satisfies BlockModule;".into(),
        options: serde_json::json!({}),
    };

    let serialized = serde_json::to_value(login_entry).unwrap();

    assert!(serialized["public_ui_block"]
        .as_str()
        .is_some_and(|source| source.contains("satisfies BlockModule")));
}

#[test]
fn verified_external_identity_rejects_unverified_and_local_claims() {
    let external_connection_id = Uuid::now_v7();
    let claim = |connection_id, verified| ExternalIdentityClaim {
        connection_id,
        subject_type: "oidc_sub".into(),
        subject_value: "CaseSensitiveSubject".into(),
        issuer: Some("https://issuer.example".into()),
        realm: Some("employees".into()),
        profile: serde_json::json!({ "email": "same@example.com" }),
        verified,
        metadata: serde_json::json!({}),
    };

    assert!(VerifiedExternalIdentity::try_from(claim(external_connection_id, false)).is_err());
    assert!(VerifiedExternalIdentity::try_from(claim(PASSWORD_LOCAL_CONNECTION_ID, true)).is_err());
    let verified = VerifiedExternalIdentity::try_from(claim(external_connection_id, true)).unwrap();
    assert_eq!(verified.subject_value(), "CaseSensitiveSubject");
}
