use control_plane::{
    auth::{AuthKernel, CompleteExternalLoginCommand, SessionIssuer},
    ports::AuthRepository,
};
use domain::{ExternalIdentityClaim, VerifiedExternalIdentity};
use storage_ephemeral::MemorySessionStore;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn seed_store() -> (
    storage_durable_postgres::PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    let store = storage_durable_postgres::PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "External identity binding tests")
        .await
        .unwrap();
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();
    control_plane_test_support::upsert_builtin_roles(&store, workspace.id)
        .await
        .unwrap();
    store
        .upsert_login_entry(&domain::LoginEntryRecord {
            id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
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
    let user = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();
    (store, workspace, user)
}

fn verified_identity(connection_id: Uuid) -> VerifiedExternalIdentity {
    VerifiedExternalIdentity::try_from(ExternalIdentityClaim {
        connection_id,
        subject_type: "oidc-subject".to_string(),
        subject_value: "CaseSensitiveSubject".to_string(),
        issuer: Some("https://issuer.example.test".to_string()),
        realm: Some("employees".to_string()),
        profile: serde_json::json!({ "display_name": "Root" }),
        verified: true,
        metadata: serde_json::json!({ "source": "test" }),
    })
    .expect("fixture identity must be verified")
}

#[tokio::test]
async fn explicit_external_binding_is_audited_idempotent_and_conflict_safe() {
    let (store, _workspace, user) = seed_store().await;
    let connection_id = Uuid::now_v7();
    store
        .upsert_login_entry(&domain::LoginEntryRecord {
            id: connection_id,
            connection_id,
            auth_type: "oidc-test".to_string(),
            title: "OIDC test".to_string(),
            enabled: true,
            is_builtin: false,
            sort_order: 10,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&store, user.id)
        .await
        .unwrap();
    let identity = verified_identity(connection_id);
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(MemorySessionStore::new("external-binding-test"), 7),
    );

    let first = kernel
        .bind_external_identity(&actor, identity.clone())
        .await
        .unwrap();
    let second = kernel
        .bind_external_identity(&actor, identity.clone())
        .await
        .unwrap();

    assert_eq!(first, second);
    let resolved = AuthRepository::find_user_for_verified_external_identity(&store, &identity)
        .await
        .unwrap()
        .expect("bound identity must resolve");
    assert_eq!(resolved.id, user.id);
    let identity_count: i64 =
        sqlx::query_scalar("select count(*) from user_auth_identities where connection_id = $1")
            .bind(connection_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where event_code = 'user.external_identity_bound'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(identity_count, 1);
    assert_eq!(audit_count, 1);

    let conflict = AuthRepository::bind_verified_external_identity(
        &store,
        Uuid::now_v7(),
        &identity,
        &domain::AuditLogRecord {
            id: Uuid::now_v7(),
            workspace_id: Some(actor.current_workspace_id),
            actor_user_id: Some(actor.user_id),
            target_type: "user_auth_identity".to_string(),
            target_id: Some(actor.user_id),
            event_code: "user.external_identity_bound".to_string(),
            payload: serde_json::json!({}),
            created_at: time::OffsetDateTime::now_utc(),
        },
    )
    .await
    .expect_err("the same external identity cannot be rebound to another user");
    assert!(conflict
        .to_string()
        .contains("external_identity_already_bound"));
}

async fn external_entry(
    store: &storage_durable_postgres::PgControlPlaneStore,
) -> domain::LoginEntryRecord {
    let connection_id = Uuid::now_v7();
    let entry = domain::LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id,
        auth_type: "github".into(),
        title: "GitHub".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 20,
        public_ui_block: String::new(),
        options: serde_json::json!({"extension_config":{"self_registration_enabled":true}}),
    };
    store.upsert_login_entry(&entry).await.unwrap();
    entry
}

fn registration_identity(
    connection_id: Uuid,
    subject: &str,
    profile: serde_json::Value,
) -> VerifiedExternalIdentity {
    VerifiedExternalIdentity::try_from(ExternalIdentityClaim {
        connection_id,
        subject_type: "github".into(),
        subject_value: subject.into(),
        issuer: Some("https://github.com".into()),
        realm: None,
        profile,
        verified: true,
        metadata: serde_json::json!({"source":"github-test"}),
    })
    .unwrap()
}

#[tokio::test]
async fn external_first_registration_creates_only_member_and_existing_login_ignores_signup_gates() {
    let (store, workspace, _) = seed_store().await;
    let mut entry = external_entry(&store).await;
    let identity = registration_identity(
        entry.connection_id,
        "42",
        serde_json::json!({"email":"External@Example.COM","email_verified":true}),
    );
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(MemorySessionStore::new("external-registration-test"), 7),
    );
    let result = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity.clone(),
            registration_eligible: true,
        })
        .await
        .unwrap();
    assert!(!result.actor.is_root);
    assert_eq!(result.actor.effective_display_role, "member");
    assert_eq!(result.session.current_workspace_id, workspace.id);
    let user = AuthRepository::find_user_by_id(&store, result.actor.user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.email, "external@example.com");
    assert!(user.account.starts_with("external_"));
    assert!(!user.email_login_enabled && !user.phone_login_enabled);
    assert!(user.password_hash.starts_with('!'));
    assert!(user
        .roles
        .iter()
        .all(|role| role.scope_kind == domain::RoleScopeKind::Workspace && role.code == "member"));
    let identity_count: i64 =
        sqlx::query_scalar("select count(*) from user_auth_identities where user_id = $1")
            .bind(user.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let local_count: i64 = sqlx::query_scalar(
        "select count(*) from user_auth_identities where user_id = $1 and connection_id = $2",
    )
    .bind(user.id)
    .bind(domain::PASSWORD_LOCAL_CONNECTION_ID)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let audit_count: i64 = sqlx::query_scalar("select count(*) from audit_logs where target_id = $1 and event_code = 'member.external_registered'").bind(user.id).fetch_one(store.pool()).await.unwrap();
    assert_eq!((identity_count, local_count, audit_count), (1, 0, 1));
    assert!(AuthRepository::find_user_for_password_login(
        &store,
        domain::PASSWORD_LOCAL_CONNECTION_ID,
        &user.email
    )
    .await
    .unwrap()
    .is_none());

    // Existing binding needs neither email nor registration eligibility, even after signup closes.
    entry.options = serde_json::json!({"extension_config":{"self_registration_enabled":false}});
    store.upsert_login_entry(&entry).await.unwrap();
    let no_email = registration_identity(entry.connection_id, "42", serde_json::json!({}));
    let again = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: no_email.clone(),
            registration_eligible: false,
        })
        .await
        .unwrap();
    assert_eq!(again.actor.user_id, user.id);
    entry.enabled = false;
    store.upsert_login_entry(&entry).await.unwrap();
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: no_email.clone(),
            registration_eligible: false,
        })
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("login_entry_disabled"));
    entry.enabled = true;
    store.upsert_login_entry(&entry).await.unwrap();
    sqlx::query("update users set status = 'disabled' where id = $1")
        .bind(user.id)
        .execute(store.pool())
        .await
        .unwrap();
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: no_email,
            registration_eligible: false,
        })
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("user_disabled"));
}

#[tokio::test]
async fn concurrent_external_registration_is_one_atomic_identity_and_email_cannot_take_over_root() {
    let (store, _, root) = seed_store().await;
    let entry = external_entry(&store).await;
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new("external-registration-concurrency"),
            7,
        ),
    );
    let conflict_identity = registration_identity(
        entry.connection_id,
        "email-collision",
        serde_json::json!({"email":root.email,"email_verified":true}),
    );
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: conflict_identity.clone(),
            registration_eligible: true,
        })
        .await
        .err()
        .unwrap();
    assert!(error
        .to_string()
        .contains("external_registration_identity_exists"));
    assert!(
        AuthRepository::find_user_for_verified_external_identity(&store, &conflict_identity)
            .await
            .unwrap()
            .is_none()
    );
    let before: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(before, 1, "email collision must leave no provisional user");

    let identity = registration_identity(
        entry.connection_id,
        "concurrent",
        serde_json::json!({"email":"concurrent@example.com","email_verified":true}),
    );
    let (first, second) = tokio::join!(
        kernel.complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity.clone(),
            registration_eligible: true
        }),
        kernel.complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity.clone(),
            registration_eligible: true
        }),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.actor.user_id, second.actor.user_id);
    assert_ne!(first.actor.user_id, root.id);
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let identities: i64 =
        sqlx::query_scalar("select count(*) from user_auth_identities where connection_id = $1")
            .bind(entry.connection_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let memberships: i64 =
        sqlx::query_scalar("select count(*) from workspace_memberships where user_id = $1")
            .bind(first.actor.user_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let roles: i64 =
        sqlx::query_scalar("select count(*) from user_role_bindings where user_id = $1")
            .bind(first.actor.user_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where event_code = 'member.external_registered'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        (users, identities, memberships, roles, audits),
        (2, 1, 1, 1, 1)
    );
}

#[tokio::test]
async fn external_registration_rolls_back_user_membership_role_and_identity_when_audit_fails() {
    use control_plane::ports::{
        CreateExternalRegisteredMemberInput, ExternalRegistrationRepository,
    };
    let (store, workspace, _) = seed_store().await;
    let entry = external_entry(&store).await;
    let identity = registration_identity(
        entry.connection_id,
        "rollback",
        serde_json::json!({"email":"rollback@example.com","email_verified":true}),
    );
    let audit = domain::AuditLogRecord {
        id: Uuid::now_v7(),
        workspace_id: Some(workspace.id),
        actor_user_id: None,
        target_type: "user".into(),
        target_id: None,
        event_code: "member.external_registered".into(),
        payload: serde_json::json!({}),
        created_at: time::OffsetDateTime::now_utc(),
    };
    AuthRepository::append_audit_log(&store, &audit)
        .await
        .unwrap();
    let before = sqlx::query_as::<_, (i64, i64, i64)>("select (select count(*) from users), (select count(*) from workspace_memberships), (select count(*) from user_role_bindings)").fetch_one(store.pool()).await.unwrap();
    let result = store
        .create_external_registered_member(&CreateExternalRegisteredMemberInput {
            identity: identity.clone(),
            email: "rollback@example.com".into(),
            audit,
        })
        .await;
    assert!(
        result.is_err(),
        "duplicate audit identity must abort the registration transaction"
    );
    let after = sqlx::query_as::<_, (i64, i64, i64)>("select (select count(*) from users), (select count(*) from workspace_memberships), (select count(*) from user_role_bindings)").fetch_one(store.pool()).await.unwrap();
    assert_eq!(before, after);
    assert!(
        AuthRepository::find_user_for_verified_external_identity(&store, &identity)
            .await
            .unwrap()
            .is_none()
    );
}
