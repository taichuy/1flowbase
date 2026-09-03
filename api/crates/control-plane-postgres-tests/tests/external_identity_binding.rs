use control_plane::{
    auth::{AuthKernel, SessionIssuer},
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
