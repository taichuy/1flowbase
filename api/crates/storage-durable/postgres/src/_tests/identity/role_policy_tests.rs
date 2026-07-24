use domain::PermissionDefinition;
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

async fn bootstrapped_store() -> (PgControlPlaneStore, Uuid) {
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

    (store, workspace.id)
}

#[tokio::test]
async fn upsert_permission_catalog_grants_new_permissions_only_to_auto_grant_roles() {
    let (store, workspace_id) = bootstrapped_store().await;

    store
        .upsert_permission_catalog(&[PermissionDefinition {
            code: "workspace.audit.all".to_string(),
            resource: "workspace".to_string(),
            action: "audit".to_string(),
            scope: "all".to_string(),
            name: "workspace:audit:all".to_string(),
        }])
        .await
        .unwrap();

    let granted_roles: Vec<String> = sqlx::query_scalar(
        r#"
        select r.code
        from role_permissions rp
        join roles r on r.id = rp.role_id
        join permission_definitions pd on pd.id = rp.permission_id
        where pd.code = $1
          and ((r.scope_kind = 'workspace' and r.workspace_id = $2) or r.scope_kind = 'system')
        order by r.code asc
        "#,
    )
    .bind("workspace.audit.all")
    .bind(workspace_id)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(granted_roles, vec!["admin"]);
}

#[tokio::test]
async fn upsert_builtin_roles_sets_admin_auto_grant_and_member_default_role() {
    let (store, workspace_id) = bootstrapped_store().await;

    let role_flags: Vec<(String, bool, bool)> = sqlx::query_as(
        r#"
        select code, auto_grant_new_permissions, is_default_member_role
        from roles
        where (scope_kind = 'workspace' and workspace_id = $1) or scope_kind = 'system'
        order by scope_kind asc, code asc
        "#,
    )
    .bind(workspace_id)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(
        role_flags,
        vec![
            ("root".to_string(), false, false),
            ("admin".to_string(), true, false),
            ("member".to_string(), false, true),
        ]
    );
}

#[tokio::test]
async fn role_data_policy_migration_seeds_builtin_roles_and_new_roles_get_restricted_default() {
    let (store, workspace_id) = bootstrapped_store().await;

    type BuiltinPolicyRow = (String, bool, bool, bool, bool, String, String, String);
    let builtin_policies: Vec<BuiltinPolicyRow> = sqlx::query_as(
        r#"
            select
              r.code,
              p.can_view,
              p.can_create,
              p.can_update,
              p.can_delete,
              p.default_view_scope,
              p.default_update_scope,
              p.default_delete_scope
            from roles r
            join role_data_policies p on p.role_id = r.id
            where (r.scope_kind = 'workspace' and r.workspace_id = $1) or r.scope_kind = 'system'
            order by r.scope_kind asc, r.code asc
            "#,
    )
    .bind(workspace_id)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(
        builtin_policies,
        vec![
            (
                "root".to_string(),
                true,
                true,
                true,
                true,
                "system_all".to_string(),
                "system_all".to_string(),
                "system_all".to_string(),
            ),
            (
                "admin".to_string(),
                true,
                true,
                true,
                true,
                "scope_all".to_string(),
                "scope_all".to_string(),
                "scope_all".to_string(),
            ),
            (
                "member".to_string(),
                true,
                true,
                true,
                true,
                "own".to_string(),
                "own".to_string(),
                "own".to_string(),
            ),
        ]
    );

    let actor_user_id = Uuid::now_v7();
    <PgControlPlaneStore as control_plane::ports::RoleRepository>::create_team_role(
        &store,
        &control_plane::ports::CreateWorkspaceRoleInput {
            actor_user_id,
            workspace_id,
            code: "auditor".into(),
            name: "Auditor".into(),
            introduction: String::new(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        },
    )
    .await
    .unwrap();

    let auditor_policy: (bool, bool, bool, bool, String, String, String) = sqlx::query_as(
        r#"
        select
          p.can_view,
          p.can_create,
          p.can_update,
          p.can_delete,
          p.default_view_scope,
          p.default_update_scope,
          p.default_delete_scope
        from role_data_policies p
        join roles r on r.id = p.role_id
        where r.workspace_id = $1 and r.code = 'auditor'
        "#,
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        auditor_policy,
        (
            false,
            false,
            false,
            false,
            "own".to_string(),
            "own".to_string(),
            "own".to_string(),
        )
    );

    let auditor_role_id: Uuid =
        sqlx::query_scalar("select id from roles where workspace_id = $1 and code = 'auditor'")
            .bind(workspace_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let invalid_scope =
        sqlx::query("update role_data_policies set default_view_scope = 'bad' where role_id = $1")
            .bind(auditor_role_id)
            .execute(store.pool())
            .await;
    assert!(invalid_scope.is_err());

    let duplicate_default = sqlx::query(
        r#"
        insert into role_data_policies (
          id, role_id, can_view, can_create, can_update, can_delete,
          default_view_scope, default_update_scope, default_delete_scope
        )
        values ($1, $2, false, false, false, false, 'own', 'own', 'own')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(auditor_role_id)
    .execute(store.pool())
    .await;
    assert!(duplicate_default.is_err());
}
