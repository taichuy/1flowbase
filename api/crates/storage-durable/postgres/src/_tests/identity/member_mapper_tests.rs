use domain::{RoleScopeKind, UserStatus};
use storage_postgres::mappers::member_mapper::{PgMemberMapper, StoredMemberRow};
use uuid::Uuid;

#[test]
fn member_mapper_preserves_roles_and_status() {
    let row = StoredMemberRow {
        id: Uuid::now_v7(),
        account: "manager-1".into(),
        email: "manager-1@example.com".into(),
        phone: None,
        password_hash: "hash".into(),
        name: "Manager".into(),
        nickname: "Manager".into(),
        introduction: String::new(),
        preferred_locale: None,
        meta: serde_json::json!({}),
        default_display_role: Some("member".into()),
        avatar_url: None,
        email_login_enabled: true,
        phone_login_enabled: false,
        status: "active".into(),
        session_version: 1,
        roles: vec![("member".into(), RoleScopeKind::Workspace, Some(Uuid::nil()))],
    };

    let user = PgMemberMapper::to_user_record(row);

    assert!(matches!(user.status, UserStatus::Active));
    assert_eq!(user.roles.len(), 1);
    assert_eq!(user.roles[0].code, "member");
}
