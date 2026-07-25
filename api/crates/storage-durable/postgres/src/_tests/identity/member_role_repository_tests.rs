use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateMemberInput, CreateWorkspaceRoleInput, MemberRepository,
        RoleRepository, UpdateProfileInput, UpdateWorkspaceRoleInput,
    },
};
use domain::{AuditLogRecord, PermissionDefinition, RoleScopeKind, UserStatus};
use serde_json::json;
use storage_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

async fn bootstrapped_store() -> (PgControlPlaneStore, Uuid, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();

    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let root = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "root-hash",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    (store, workspace.id, root.id)
}

async fn create_workspace_role(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    code: &str,
) {
    <PgControlPlaneStore as RoleRepository>::create_team_role(
        store,
        &CreateWorkspaceRoleInput {
            actor_user_id,
            workspace_id,
            code: code.to_string(),
            name: code.to_string(),
            introduction: format!("{code} role"),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        },
    )
    .await
    .unwrap();
}

async fn role_codes_for_user(store: &PgControlPlaneStore, user_id: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        r#"
        select r.code
        from user_role_bindings urb
        join roles r on r.id = urb.role_id
        where urb.user_id = $1
        order by r.code asc
        "#,
    )
    .bind(user_id)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

async fn create_member(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    account: &str,
) -> domain::UserRecord {
    <PgControlPlaneStore as MemberRepository>::create_member_with_default_role(
        store,
        &CreateMemberInput {
            actor_user_id,
            workspace_id,
            account: account.to_string(),
            email: format!("{account}@example.com"),
            phone: None,
            password_hash: "member-hash".to_string(),
            name: account.to_string(),
            nickname: account.to_string(),
            introduction: String::new(),
            email_login_enabled: true,
            phone_login_enabled: false,
        },
    )
    .await
    .unwrap()
}

struct TestMemberLoginOptions<'a> {
    account: &'a str,
    email: &'a str,
    phone: Option<&'a str>,
    email_login_enabled: bool,
    phone_login_enabled: bool,
}

async fn create_member_with_login_options(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    options: TestMemberLoginOptions<'_>,
) -> domain::UserRecord {
    <PgControlPlaneStore as MemberRepository>::create_member_with_default_role(
        store,
        &CreateMemberInput {
            actor_user_id,
            workspace_id,
            account: options.account.to_string(),
            email: options.email.to_string(),
            phone: options.phone.map(str::to_string),
            password_hash: "member-hash".to_string(),
            name: options.account.to_string(),
            nickname: options.account.to_string(),
            introduction: String::new(),
            email_login_enabled: options.email_login_enabled,
            phone_login_enabled: options.phone_login_enabled,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn create_member_with_default_role_assigns_default_role_and_login_identities() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;

    let member = <PgControlPlaneStore as MemberRepository>::create_member_with_default_role(
        &store,
        &CreateMemberInput {
            actor_user_id,
            workspace_id,
            account: "alice".to_string(),
            email: "alice@example.com".to_string(),
            phone: Some("18800001111".to_string()),
            password_hash: "member-hash".to_string(),
            name: "Alice".to_string(),
            nickname: "Alice".to_string(),
            introduction: "workspace member".to_string(),
            email_login_enabled: true,
            phone_login_enabled: true,
        },
    )
    .await
    .unwrap();

    assert_eq!(member.account, "alice");
    assert_eq!(member.status, UserStatus::Active);
    assert_eq!(member.default_display_role.as_deref(), Some("member"));
    assert_eq!(role_codes_for_user(&store, member.id).await, vec!["member"]);
    let member_role_scope: Uuid = sqlx::query_scalar(
        r#"
        select urb.scope_id
        from user_role_bindings urb
        join roles r on r.id = urb.role_id
        where urb.user_id = $1
          and r.code = 'member'
        "#,
    )
    .bind(member.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(member_role_scope, workspace_id);

    let root_role_scope: Uuid = sqlx::query_scalar(
        r#"
        select urb.scope_id
        from user_role_bindings urb
        join roles r on r.id = urb.role_id
        where r.scope_kind = 'system'
          and r.code = 'root'
        "#,
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(root_role_scope, domain::SYSTEM_SCOPE_ID);

    let membership_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_memberships where workspace_id = $1 and user_id = $2",
    )
    .bind(workspace_id)
    .bind(member.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(membership_count, 1);

    let identities: Vec<(String, String)> = sqlx::query_as(
        r#"
        select subject_type, subject_value
        from user_auth_identities
        where user_id = $1
        order by subject_type asc
        "#,
    )
    .bind(member.id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        identities,
        vec![
            ("account".to_string(), "alice".to_string()),
            ("email".to_string(), "alice@example.com".to_string()),
            ("phone".to_string(), "18800001111".to_string()),
        ]
    );
}

#[tokio::test]
async fn password_login_resolves_member_from_auth_identity_subjects() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let member = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "identity-member",
            email: "identity-member@example.com",
            phone: Some("18800002222"),
            email_login_enabled: true,
            phone_login_enabled: true,
        },
    )
    .await;

    sqlx::query(
        r#"
        update users
        set account = 'renamed-identity-member',
            email = 'renamed-identity-member@example.com',
            phone = '18800003333'
        where id = $1
        "#,
    )
    .bind(member.id)
    .execute(store.pool())
    .await
    .unwrap();

    for identifier in [
        "identity-member",
        "identity-member@example.com",
        "18800002222",
    ] {
        let resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
            &store,
            domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            identifier,
        )
        .await
        .unwrap()
        .expect("identity subject should resolve the renamed user");
        assert_eq!(resolved.id, member.id);
    }
}

#[tokio::test]
async fn password_login_does_not_fallback_to_user_fields_without_identity() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let member = create_member(&store, workspace_id, actor_user_id, "missing-identity").await;

    sqlx::query("delete from user_auth_identities where user_id = $1 and subject_type = 'account'")
        .bind(member.id)
        .execute(store.pool())
        .await
        .unwrap();

    let resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "missing-identity",
    )
    .await
    .unwrap();
    assert!(resolved.is_none());
}

#[tokio::test]
async fn password_login_rejects_ambiguous_identity_subjects() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let first = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "shared-login-subject",
            email: "first-shared@example.com",
            phone: None,
            email_login_enabled: true,
            phone_login_enabled: false,
        },
    )
    .await;
    let second = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "second-shared",
            email: "shared-login-subject",
            phone: None,
            email_login_enabled: true,
            phone_login_enabled: false,
        },
    )
    .await;
    assert_ne!(first.id, second.id);

    let resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "shared-login-subject",
    )
    .await
    .unwrap();
    assert!(resolved.is_none());
}

#[tokio::test]
async fn password_login_filters_identity_subjects_by_authenticator_instance() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let default_member = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "default-shared-instance",
            email: "default-shared-instance@example.com",
            phone: None,
            email_login_enabled: true,
            phone_login_enabled: false,
        },
    )
    .await;
    let staff_member = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "staff-shared-instance",
            email: "staff-shared-instance@example.com",
            phone: None,
            email_login_enabled: true,
            phone_login_enabled: false,
        },
    )
    .await;

    let staff_authenticator_id = Uuid::now_v7();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: staff_authenticator_id,
            auth_type: "password-local".into(),
            title: "Staff Password".into(),
            enabled: true,
            is_builtin: false,
            sort_order: 10,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();

    sqlx::query(
        r#"
        update user_auth_identities
        set subject_value = 'shared-instance-subject'
        where user_id = $1
          and authenticator_id = $2
          and subject_type = $3
        "#,
    )
    .bind(default_member.id)
    .bind(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
    .bind(domain::AUTH_SUBJECT_TYPE_ACCOUNT)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into user_auth_identities (
            id, user_id, authenticator_id, subject_type, subject_value, metadata
        )
        values ($1, $2, $3, $4, 'shared-instance-subject', '{}')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(staff_member.id)
    .bind(staff_authenticator_id)
    .bind(domain::AUTH_SUBJECT_TYPE_ACCOUNT)
    .execute(store.pool())
    .await
    .unwrap();

    let default_resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "shared-instance-subject",
    )
    .await
    .unwrap()
    .expect("default instance identity should resolve its own member");
    assert_eq!(default_resolved.id, default_member.id);

    let staff_resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        staff_authenticator_id,
        "shared-instance-subject",
    )
    .await
    .unwrap()
    .expect("staff instance identity should resolve its own member");
    assert_eq!(staff_resolved.id, staff_member.id);
}

#[tokio::test]
async fn password_login_applies_email_and_phone_flags_at_identity_resolution() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let member = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "flagged-identity",
            email: "flagged-identity@example.com",
            phone: Some("18800004444"),
            email_login_enabled: false,
            phone_login_enabled: false,
        },
    )
    .await;

    let account = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "flagged-identity",
    )
    .await
    .unwrap()
    .expect("account identity should not be gated by email/phone flags");
    assert_eq!(account.id, member.id);

    let email = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "flagged-identity@example.com",
    )
    .await
    .unwrap();
    let phone = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "18800004444",
    )
    .await
    .unwrap();
    assert!(email.is_none());
    assert!(phone.is_none());
}

#[tokio::test]
async fn member_profile_update_replaces_password_local_contact_identities() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let member = create_member_with_login_options(
        &store,
        workspace_id,
        actor_user_id,
        TestMemberLoginOptions {
            account: "profile-identity",
            email: "profile-identity@example.com",
            phone: Some("18800005555"),
            email_login_enabled: true,
            phone_login_enabled: true,
        },
    )
    .await;

    <PgControlPlaneStore as MemberRepository>::update_member_profile(
        &store,
        &control_plane::ports::UpdateMemberInput {
            actor_user_id,
            user_id: member.id,
            name: "Profile Identity".to_string(),
            nickname: "Profile Identity".to_string(),
            email: "profile-identity-next@example.com".to_string(),
            phone: Some("18800006666".to_string()),
            introduction: String::new(),
        },
    )
    .await
    .unwrap();

    for identifier in ["profile-identity@example.com", "18800005555"] {
        let resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
            &store,
            domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            identifier,
        )
        .await
        .unwrap();
        assert!(resolved.is_none());
    }

    for identifier in ["profile-identity-next@example.com", "18800006666"] {
        let resolved = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
            &store,
            domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            identifier,
        )
        .await
        .unwrap()
        .expect("updated contact identity should resolve the member");
        assert_eq!(resolved.id, member.id);
    }
}

#[tokio::test]
async fn self_profile_update_replaces_password_local_email_identity() {
    let (store, _workspace_id, actor_user_id) = bootstrapped_store().await;

    <PgControlPlaneStore as AuthRepository>::update_profile(
        &store,
        &UpdateProfileInput {
            actor_user_id,
            user_id: actor_user_id,
            name: "Root Next".to_string(),
            nickname: "Root Next".to_string(),
            email: "root-next@example.com".to_string(),
            phone: None,
            avatar_url: None,
            introduction: String::new(),
            preferred_locale: None,
        },
    )
    .await
    .unwrap();

    let old_email = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root@example.com",
    )
    .await
    .unwrap();
    assert!(old_email.is_none());

    let new_email = <PgControlPlaneStore as AuthRepository>::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root-next@example.com",
    )
    .await
    .unwrap()
    .expect("updated self email identity should resolve root");
    assert_eq!(new_email.id, actor_user_id);
}

#[tokio::test]
async fn replace_member_roles_normalizes_codes_and_replaces_workspace_roles() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    create_workspace_role(&store, workspace_id, actor_user_id, "auditor").await;
    create_workspace_role(&store, workspace_id, actor_user_id, "editor").await;
    let member = create_member(&store, workspace_id, actor_user_id, "bob").await;

    <PgControlPlaneStore as MemberRepository>::replace_member_roles(
        &store,
        actor_user_id,
        workspace_id,
        member.id,
        &[
            " editor ".to_string(),
            "auditor".to_string(),
            "editor".to_string(),
            String::new(),
        ],
    )
    .await
    .unwrap();

    assert_eq!(
        role_codes_for_user(&store, member.id).await,
        vec!["auditor", "editor"]
    );
}

#[tokio::test]
async fn member_status_and_password_updates_reject_root_and_bump_session_version() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let member = create_member(&store, workspace_id, actor_user_id, "carol").await;

    <PgControlPlaneStore as MemberRepository>::reset_member_password(
        &store,
        actor_user_id,
        member.id,
        "new-member-hash",
    )
    .await
    .unwrap();
    <PgControlPlaneStore as MemberRepository>::disable_member(&store, actor_user_id, member.id)
        .await
        .unwrap();

    let updated: (String, i64, String) =
        sqlx::query_as("select status, session_version, password_hash from users where id = $1")
            .bind(member.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        updated,
        ("disabled".to_string(), 3, "new-member-hash".to_string())
    );

    let root_disable = <PgControlPlaneStore as MemberRepository>::disable_member(
        &store,
        actor_user_id,
        actor_user_id,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        root_disable.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("root_user_immutable"))
    ));

    let root_reset = <PgControlPlaneStore as MemberRepository>::reset_member_password(
        &store,
        actor_user_id,
        actor_user_id,
        "root-new-hash",
    )
    .await
    .unwrap_err();
    assert!(matches!(
        root_reset.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied("root_user_immutable"))
    ));
}

#[tokio::test]
async fn replace_member_roles_rejects_unknown_code_without_clearing_existing_bindings() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let member = create_member(&store, workspace_id, actor_user_id, "dana").await;

    let result = <PgControlPlaneStore as MemberRepository>::replace_member_roles(
        &store,
        actor_user_id,
        workspace_id,
        member.id,
        &["missing-role".to_string()],
    )
    .await;

    let error = result.unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::InvalidInput("role_code"))
    ));
    assert_eq!(role_codes_for_user(&store, member.id).await, vec!["member"]);
}

#[tokio::test]
async fn create_and_update_workspace_role_keep_single_default_member_role() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;

    <PgControlPlaneStore as RoleRepository>::create_team_role(
        &store,
        &CreateWorkspaceRoleInput {
            actor_user_id,
            workspace_id,
            code: "operator".to_string(),
            name: "Operator".to_string(),
            introduction: "Ops role".to_string(),
            auto_grant_new_permissions: false,
            is_default_member_role: true,
        },
    )
    .await
    .unwrap();

    let initial_defaults: Vec<String> = sqlx::query_scalar(
        r#"
        select code
        from roles
        where scope_kind = 'workspace'
          and workspace_id = $1
          and is_default_member_role = true
        order by code asc
        "#,
    )
    .bind(workspace_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(initial_defaults, vec!["operator"]);

    create_workspace_role(&store, workspace_id, actor_user_id, "reviewer").await;
    <PgControlPlaneStore as RoleRepository>::update_team_role(
        &store,
        &UpdateWorkspaceRoleInput {
            actor_user_id,
            workspace_id,
            role_code: "reviewer".to_string(),
            name: "Reviewer".to_string(),
            introduction: "Review role".to_string(),
            auto_grant_new_permissions: Some(true),
            is_default_member_role: Some(true),
        },
    )
    .await
    .unwrap();

    let roles = <PgControlPlaneStore as RoleRepository>::list_roles(&store, workspace_id)
        .await
        .unwrap();
    let default_roles: Vec<String> = roles
        .iter()
        .filter(|role| role.is_default_member_role)
        .map(|role| role.code.clone())
        .collect();
    let reviewer = roles
        .iter()
        .find(|role| role.code == "reviewer")
        .expect("reviewer role should exist");

    assert_eq!(default_roles, vec!["reviewer"]);
    assert_eq!(reviewer.scope_kind, RoleScopeKind::Workspace);
    assert!(reviewer.auto_grant_new_permissions);
    assert!(reviewer.is_editable);
}

#[tokio::test]
async fn replace_role_permissions_normalizes_codes_and_replaces_existing_permissions() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    create_workspace_role(&store, workspace_id, actor_user_id, "support").await;
    store
        .upsert_permission_catalog(&[
            PermissionDefinition {
                code: "workspace.support.read".to_string(),
                resource: "workspace".to_string(),
                action: "support.read".to_string(),
                scope: "workspace".to_string(),
                name: "Support read".to_string(),
            },
            PermissionDefinition {
                code: "workspace.support.write".to_string(),
                resource: "workspace".to_string(),
                action: "support.write".to_string(),
                scope: "workspace".to_string(),
                name: "Support write".to_string(),
            },
        ])
        .await
        .unwrap();

    <PgControlPlaneStore as RoleRepository>::replace_role_permissions(
        &store,
        actor_user_id,
        workspace_id,
        "support",
        &[
            " workspace.support.read ".to_string(),
            "workspace.support.write".to_string(),
            "workspace.support.read".to_string(),
            String::new(),
        ],
    )
    .await
    .unwrap();
    <PgControlPlaneStore as RoleRepository>::replace_role_permissions(
        &store,
        actor_user_id,
        workspace_id,
        "support",
        &["workspace.support.write".to_string()],
    )
    .await
    .unwrap();

    let permissions = <PgControlPlaneStore as RoleRepository>::list_role_permissions(
        &store,
        workspace_id,
        "support",
    )
    .await
    .unwrap();

    assert_eq!(permissions, vec!["workspace.support.write"]);

    let workspace_role_permission_scopes: Vec<Uuid> = sqlx::query_scalar(
        r#"
        select rp.scope_id
        from role_permissions rp
        join roles r on r.id = rp.role_id
        where r.code = 'support'
          and r.workspace_id = $1
        "#,
    )
    .bind(workspace_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(workspace_role_permission_scopes, vec![workspace_id]);
}

#[tokio::test]
async fn role_deletion_rejects_default_and_bound_roles_before_deleting_unused_custom_role() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    create_workspace_role(&store, workspace_id, actor_user_id, "temporary").await;
    create_workspace_role(&store, workspace_id, actor_user_id, "assigned").await;
    let member = create_member(&store, workspace_id, actor_user_id, "erin").await;

    <PgControlPlaneStore as MemberRepository>::replace_member_roles(
        &store,
        actor_user_id,
        workspace_id,
        member.id,
        &["assigned".to_string()],
    )
    .await
    .unwrap();

    let default_role_result = <PgControlPlaneStore as RoleRepository>::delete_team_role(
        &store,
        actor_user_id,
        workspace_id,
        "member",
    )
    .await;
    let default_role_error = default_role_result.unwrap_err();
    assert!(matches!(
        default_role_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(
            "builtin_role_immutable"
        )) | Some(ControlPlaneError::InvalidInput(
            "default_member_role_required"
        ))
    ));

    let bound_role_result = <PgControlPlaneStore as RoleRepository>::delete_team_role(
        &store,
        actor_user_id,
        workspace_id,
        "assigned",
    )
    .await;
    let bound_role_error = bound_role_result.unwrap_err();
    assert!(matches!(
        bound_role_error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict("role_in_use"))
    ));

    <PgControlPlaneStore as RoleRepository>::delete_team_role(
        &store,
        actor_user_id,
        workspace_id,
        "temporary",
    )
    .await
    .unwrap();
    let role_count: i64 = sqlx::query_scalar(
        "select count(*) from roles where workspace_id = $1 and code = 'temporary'",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(role_count, 0);
}

#[tokio::test]
async fn append_audit_log_writes_workspace_and_system_scope_routing() {
    let (store, workspace_id, actor_user_id) = bootstrapped_store().await;
    let workspace_event_id = Uuid::now_v7();
    let system_event_id = Uuid::now_v7();

    <PgControlPlaneStore as AuthRepository>::append_audit_log(
        &store,
        &AuditLogRecord {
            id: workspace_event_id,
            workspace_id: Some(workspace_id),
            actor_user_id: Some(actor_user_id),
            target_type: "workspace".into(),
            target_id: Some(workspace_id),
            event_code: "workspace.test".into(),
            payload: json!({"kind": "workspace"}),
            created_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as AuthRepository>::append_audit_log(
        &store,
        &AuditLogRecord {
            id: system_event_id,
            workspace_id: None,
            actor_user_id: None,
            target_type: "system".into(),
            target_id: None,
            event_code: "system.test".into(),
            payload: json!({"kind": "system"}),
            created_at: OffsetDateTime::now_utc(),
        },
    )
    .await
    .unwrap();

    let workspace_scope: (Uuid, Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("select scope_id, created_by, updated_by from audit_logs where id = $1")
            .bind(workspace_event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(
        workspace_scope,
        (workspace_id, Some(actor_user_id), Some(actor_user_id))
    );

    let system_scope: (Uuid, Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("select scope_id, created_by, updated_by from audit_logs where id = $1")
            .bind(system_event_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(system_scope, (domain::SYSTEM_SCOPE_ID, None, None));
}
