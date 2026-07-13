use std::{borrow::Cow, collections::BTreeSet};

use sqlx::{migrate::Migrator, PgPool};
use storage_postgres::{connect, PgControlPlaneStore};
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260713160000;
// Keep the migrator embedded in this target so before/after behavior uses the shipped SQL.
const MEMBERS_VISIBILITY: &str = "settings_route.visible.settings.members";
const ROLES_VISIBILITY: &str = "settings_route.visible.settings.roles";
const MEMBERS_FEATURE: &str = "settings_feature.access.system.members";
const ROLES_FEATURE: &str = "settings_feature.access.system.roles";

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();
    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

fn before_settings_feature_grant_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn historical_pool() -> PgPool {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_settings_feature_grant_migrator()
        .run(&pool)
        .await
        .unwrap();
    pool
}

async fn insert_role(pool: &PgPool, workspace_id: Uuid, code: &str) -> Uuid {
    let role_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction,
            is_builtin, is_editable, auto_grant_new_permissions, is_default_member_role
        )
        values ($1, $2, 'workspace', $2, $3, $3, '', false, true, false, false)
        "#,
    )
    .bind(role_id)
    .bind(workspace_id)
    .bind(code)
    .execute(pool)
    .await
    .unwrap();
    role_id
}

async fn insert_permission(pool: &PgPool, code: &str, resource: &str, action: &str, scope: &str) {
    sqlx::query(
        r#"
        insert into permission_definitions (
            id, scope_id, resource, action, scope, code, name, introduction
        )
        values ($1, $2, $3, $4, $5, $6, $6, '')
        on conflict (code) do nothing
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(domain::SYSTEM_SCOPE_ID)
    .bind(resource)
    .bind(action)
    .bind(scope)
    .bind(code)
    .execute(pool)
    .await
    .unwrap();
}

async fn grant(pool: &PgPool, role_id: Uuid, workspace_id: Uuid, permission_code: &str) {
    sqlx::query(
        r#"
        insert into role_permissions (id, role_id, permission_id, scope_id)
        select $1, $2, id, $3
        from permission_definitions
        where code = $4
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(role_id)
    .bind(workspace_id)
    .bind(permission_code)
    .execute(pool)
    .await
    .unwrap();
}

async fn permission_codes(pool: &PgPool, role_id: Uuid) -> BTreeSet<String> {
    sqlx::query_scalar(
        r#"
        select definitions.code
        from role_permissions grants
        join permission_definitions definitions on definitions.id = grants.permission_id
        where grants.role_id = $1
        order by definitions.code
        "#,
    )
    .bind(role_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .collect()
}

async fn seed_historical_permissions(pool: &PgPool) {
    for (code, resource, action, scope) in [
        (
            MEMBERS_VISIBILITY,
            "settings_route",
            "visible",
            "settings.members",
        ),
        (
            ROLES_VISIBILITY,
            "settings_route",
            "visible",
            "settings.roles",
        ),
        (
            "settings_route.visible.settings.docs",
            "settings_route",
            "visible",
            "settings.docs",
        ),
        ("user.view.all", "user", "view", "all"),
        ("user.manage.all", "user", "manage", "all"),
        ("role_permission.view.all", "role_permission", "view", "all"),
        (
            "role_permission.manage.all",
            "role_permission",
            "manage",
            "all",
        ),
    ] {
        insert_permission(pool, code, resource, action, scope).await;
    }
}

#[tokio::test]
async fn migration_reconciles_members_and_roles_grants_without_touching_other_features() {
    let pool = historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Grant migration")
        .await
        .unwrap();
    seed_historical_permissions(&pool).await;

    let members_role = insert_role(&pool, workspace.id, "legacy_members").await;
    let roles_role = insert_role(&pool, workspace.id, "legacy_roles").await;
    let docs_role = insert_role(&pool, workspace.id, "legacy_docs").await;
    grant(&pool, members_role, workspace.id, MEMBERS_VISIBILITY).await;
    grant(&pool, roles_role, workspace.id, ROLES_VISIBILITY).await;
    grant(&pool, roles_role, workspace.id, "role_permission.view.all").await;
    grant(
        &pool,
        docs_role,
        workspace.id,
        "settings_route.visible.settings.docs",
    )
    .await;

    assert_eq!(
        permission_codes(&pool, members_role).await,
        BTreeSet::from([MEMBERS_VISIBILITY.to_string()])
    );
    assert_eq!(
        permission_codes(&pool, roles_role).await,
        BTreeSet::from([
            ROLES_VISIBILITY.to_string(),
            "role_permission.view.all".to_string(),
        ])
    );

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    assert_eq!(
        permission_codes(&pool, members_role).await,
        BTreeSet::from([
            MEMBERS_FEATURE.to_string(),
            "user.view.all".to_string(),
            "user.manage.all".to_string(),
            "role_permission.view.all".to_string(),
            "role_permission.manage.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, roles_role).await,
        BTreeSet::from([
            ROLES_FEATURE.to_string(),
            "role_permission.view.all".to_string(),
            "role_permission.manage.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, docs_role).await,
        BTreeSet::from(["settings_route.visible.settings.docs".to_string()])
    );

    let removed_legacy_definitions: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = any($1)")
            .bind(&[MEMBERS_VISIBILITY, ROLES_VISIBILITY])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definitions, 0);

    let new_feature_only = insert_role(&pool, workspace.id, "new_feature_only").await;
    grant(&pool, new_feature_only, workspace.id, MEMBERS_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, new_feature_only).await,
        BTreeSet::from([MEMBERS_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn migration_rolls_back_all_grants_when_legacy_cleanup_fails() {
    let pool = historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Grant rollback")
        .await
        .unwrap();
    seed_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace.id, "legacy_members").await;
    grant(&pool, role_id, workspace.id, MEMBERS_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.members' then
                raise exception 'forced legacy cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error.to_string().contains("forced legacy cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([MEMBERS_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = any($1)")
            .bind(&[MEMBERS_FEATURE, ROLES_FEATURE])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}
