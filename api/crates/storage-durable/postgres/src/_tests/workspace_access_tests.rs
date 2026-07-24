use control_plane::ports::{AuthRepository, WorkspaceRepository};
use domain::SYSTEM_SCOPE_ID;
use storage_postgres::{run_migrations, PgControlPlaneStore};
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

async fn root_tenant_id(store: &PgControlPlaneStore) -> Uuid {
    sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn insert_workspace(store: &PgControlPlaneStore, tenant_id: Uuid, name: &str) -> Uuid {
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(name)
    .execute(store.pool())
    .await
    .unwrap();

    workspace_id
}

async fn insert_user(
    store: &PgControlPlaneStore,
    user_id: Uuid,
    account_prefix: &str,
    default_display_role: &str,
) {
    let account = format!("{account_prefix}-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, phone, password_hash, name, nickname, avatar_url, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status, session_version,
            created_by, updated_by
        )
        values (
            $1, $2, $3, null, 'hash', $4, $5, null, '', $6, true, false, 'active', 1, null, null
        )
        "#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.com"))
    .bind(&account)
    .bind(&account)
    .bind(default_display_role)
    .execute(store.pool())
    .await
    .unwrap();
}

async fn insert_workspace_role(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    role_code: &str,
) -> Uuid {
    let role_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction, is_builtin, is_editable
        )
        values ($1, $2, 'workspace', $2, $3, $4, '', false, true)
        "#,
    )
    .bind(role_id)
    .bind(workspace_id)
    .bind(role_code)
    .bind(role_code)
    .execute(store.pool())
    .await
    .unwrap();

    role_id
}

async fn insert_root_role(store: &PgControlPlaneStore) -> Uuid {
    let role_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction, is_builtin, is_editable
        )
        values ($1, $2, 'system', null, 'root', 'Root', '', true, false)
        "#,
    )
    .bind(role_id)
    .bind(SYSTEM_SCOPE_ID)
    .execute(store.pool())
    .await
    .unwrap();

    role_id
}

async fn insert_membership(store: &PgControlPlaneStore, workspace_id: Uuid, user_id: Uuid) {
    sqlx::query(
        "insert into workspace_memberships (id, workspace_id, user_id, introduction) values ($1, $2, $3, '')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
}

async fn bind_role(store: &PgControlPlaneStore, user_id: Uuid, role_id: Uuid) {
    sqlx::query(
        r#"
        insert into user_role_bindings (id, user_id, role_id, scope_id)
        select $1, $2, roles.id, roles.scope_id
        from roles
        where roles.id = $3
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(role_id)
    .execute(store.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn list_accessible_workspaces_returns_only_memberships_for_non_root() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id = root_tenant_id(&store).await;
    let workspace_a = insert_workspace(&store, tenant_id, "Workspace Access A").await;
    let workspace_b = insert_workspace(&store, tenant_id, "Workspace Access B").await;
    let workspace_c = insert_workspace(&store, tenant_id, "Workspace Access C").await;
    let user_id = Uuid::now_v7();

    insert_user(&store, user_id, "member-access", "member").await;
    insert_membership(&store, workspace_a, user_id).await;
    insert_membership(&store, workspace_c, user_id).await;

    let workspaces =
        <PgControlPlaneStore as WorkspaceRepository>::list_accessible_workspaces(&store, user_id)
            .await
            .unwrap();
    let ids: Vec<Uuid> = workspaces.iter().map(|workspace| workspace.id).collect();

    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&workspace_a));
    assert!(ids.contains(&workspace_c));
    assert!(!ids.contains(&workspace_b));
    assert!(
        <PgControlPlaneStore as WorkspaceRepository>::get_accessible_workspace(
            &store,
            user_id,
            workspace_a
        )
        .await
        .unwrap()
        .is_some()
    );
    assert!(
        <PgControlPlaneStore as WorkspaceRepository>::get_accessible_workspace(
            &store,
            user_id,
            workspace_b
        )
        .await
        .unwrap()
        .is_none()
    );
}

#[tokio::test]
async fn list_accessible_workspaces_returns_all_workspaces_for_root() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id = root_tenant_id(&store).await;
    let workspace_a = insert_workspace(&store, tenant_id, "Workspace Root A").await;
    let workspace_b = insert_workspace(&store, tenant_id, "Workspace Root B").await;
    let workspace_c = insert_workspace(&store, tenant_id, "Workspace Root C").await;
    let user_id = Uuid::now_v7();
    let root_role_id = insert_root_role(&store).await;

    insert_user(&store, user_id, "root-access", "root").await;
    bind_role(&store, user_id, root_role_id).await;

    let workspaces =
        <PgControlPlaneStore as WorkspaceRepository>::list_accessible_workspaces(&store, user_id)
            .await
            .unwrap();
    let ids: Vec<Uuid> = workspaces.iter().map(|workspace| workspace.id).collect();

    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&workspace_a));
    assert!(ids.contains(&workspace_b));
    assert!(ids.contains(&workspace_c));
    assert!(
        <PgControlPlaneStore as WorkspaceRepository>::get_accessible_workspace(
            &store,
            user_id,
            workspace_b
        )
        .await
        .unwrap()
        .is_some()
    );
}

#[tokio::test]
async fn load_actor_context_ignores_display_role_when_role_is_missing_in_target_workspace() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id = root_tenant_id(&store).await;
    let source_workspace_id = insert_workspace(&store, tenant_id, "Workspace Source").await;
    let target_workspace_id = insert_workspace(&store, tenant_id, "Workspace Target").await;
    let user_id = Uuid::now_v7();
    let source_admin_role = insert_workspace_role(&store, source_workspace_id, "admin").await;
    let target_manager_role = insert_workspace_role(&store, target_workspace_id, "member").await;

    insert_user(&store, user_id, "role-fallback", "admin").await;
    insert_membership(&store, source_workspace_id, user_id).await;
    insert_membership(&store, target_workspace_id, user_id).await;
    bind_role(&store, user_id, source_admin_role).await;
    bind_role(&store, user_id, target_manager_role).await;

    let actor = AuthRepository::load_actor_context(
        &store,
        user_id,
        tenant_id,
        target_workspace_id,
        Some("admin"),
    )
    .await
    .unwrap();

    assert_eq!(actor.effective_display_role, "member");
}
