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
// This target embeds the shipped explicit Settings namespace migration as well.
const EXPLICIT_SETTINGS_MIGRATION_VERSION: i64 = 20260714125600;
const AUTH_CENTER_VISIBILITY: &str = "settings_route.visible.settings.auth-center";
const HOST_INFRASTRUCTURE_VISIBILITY: &str = "settings_route.visible.settings.host-infrastructure";
const MEMORY_OBSERVATION_VISIBILITY: &str = "settings_route.visible.settings.memory-observation";
const APPLICATIONS_VISIBILITY: &str = "settings_route.visible.settings.applications";
const AUTH_CENTER_FEATURE: &str = "settings_feature.access.system.auth-center";
const HOST_INFRASTRUCTURE_FEATURE: &str = "settings_feature.access.system.host-infrastructure";
const MEMORY_OBSERVATION_FEATURE: &str = "settings_feature.access.system.memory-observation";
const APPLICATIONS_FEATURE: &str = "settings_feature.access.system.applications";
const FILES_SETTINGS_MIGRATION_VERSION: i64 = 20260714170000;
const FILES_VISIBILITY: &str = "settings_route.visible.settings.files";
const FILES_FEATURE: &str = "settings_feature.access.system.files";
const DATA_MODELS_SETTINGS_MIGRATION_VERSION: i64 = 20260714180000;
const DATA_MODELS_VISIBILITY: &str = "settings_route.visible.settings.data-models";
const DATA_MODELS_FEATURE: &str = "settings_feature.access.system.data-models";
const MODEL_PROVIDERS_SETTINGS_MIGRATION_VERSION: i64 = 20260714190000;
const MODEL_PROVIDERS_VISIBILITY: &str = "settings_route.visible.settings.model-providers";
const MODEL_PROVIDERS_FEATURE: &str = "settings_feature.access.system.model-providers";
// Final cutover removes every remaining settings_route definition.
const FINAL_SETTINGS_MIGRATION_VERSION: i64 = 20260714213000;
const DOCS_VISIBILITY: &str = "settings_route.visible.settings.docs";
const API_KEY_AUTHENTICATION_VISIBILITY: &str =
    "settings_route.visible.settings.api-key-authentication";
const SYSTEM_RUNTIME_VISIBILITY: &str = "settings_route.visible.settings.system-runtime";
const MCP_MANAGEMENT_VISIBILITY: &str = "settings_route.visible.settings.mcp-management";
const DOCS_FEATURE: &str = "settings_feature.access.system.docs";
const API_KEY_AUTHENTICATION_FEATURE: &str =
    "settings_feature.access.system.api-key-authentication";
const SYSTEM_RUNTIME_FEATURE: &str = "settings_feature.access.system.system-runtime";
const MCP_MANAGEMENT_FEATURE: &str = "settings_feature.access.system.mcp-management";
const MODEL_PROVIDER_ALL_PERMISSIONS: &[&str] = &[
    "state_model.view.all",
    "state_model.view.own",
    "state_model.create.all",
    "state_model.edit.all",
    "state_model.edit.own",
    "state_model.delete.all",
    "state_model.delete.own",
    "state_model.manage.all",
    "state_model.manage.own",
    "plugin_config.view.all",
    "plugin_config.configure.all",
];
const DATA_MODEL_ALL_PERMISSIONS: &[&str] = &[
    "api_reference.view.all",
    "state_model.view.all",
    "state_model.view.own",
    "state_model.create.all",
    "state_model.edit.all",
    "state_model.edit.own",
    "state_model.delete.all",
    "state_model.delete.own",
    "state_model.manage.all",
    "state_model.manage.own",
    "external_data_source.view.all",
    "external_data_source.view.own",
    "external_data_source.create.all",
    "external_data_source.edit.all",
    "external_data_source.edit.own",
    "external_data_source.delete.all",
    "external_data_source.delete.own",
    "external_data_source.configure.all",
    "external_data_source.configure.own",
    "external_data_source.use.all",
    "external_data_source.use.own",
];
const UNMIGRATED_VISIBILITIES: &[&str] = &[
    "settings_route.visible.settings.docs",
    "settings_route.visible.settings.api-key-authentication",
    "settings_route.visible.settings.system-runtime",
    "settings_route.visible.settings.files",
    "settings_route.visible.settings.data-models",
    "settings_route.visible.settings.model-providers",
    "settings_route.visible.settings.mcp-management",
];
const REMAINING_VISIBILITIES_AFTER_FILES: &[&str] = &[
    "settings_route.visible.settings.docs",
    "settings_route.visible.settings.api-key-authentication",
    "settings_route.visible.settings.system-runtime",
    "settings_route.visible.settings.data-models",
    "settings_route.visible.settings.model-providers",
    "settings_route.visible.settings.mcp-management",
];
const REMAINING_VISIBILITIES_AFTER_DATA_MODELS: &[&str] = &[
    "settings_route.visible.settings.docs",
    "settings_route.visible.settings.api-key-authentication",
    "settings_route.visible.settings.system-runtime",
    "settings_route.visible.settings.model-providers",
    "settings_route.visible.settings.mcp-management",
];
const REMAINING_VISIBILITIES_AFTER_MODEL_PROVIDERS: &[&str] = &[
    "settings_route.visible.settings.docs",
    "settings_route.visible.settings.api-key-authentication",
    "settings_route.visible.settings.system-runtime",
    "settings_route.visible.settings.mcp-management",
];

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

fn before_explicit_settings_feature_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < EXPLICIT_SETTINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn explicit_settings_historical_pool() -> PgPool {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_explicit_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();
    pool
}

fn before_files_settings_feature_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FILES_SETTINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn files_settings_historical_pool() -> PgPool {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_files_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();
    pool
}

fn before_data_models_settings_feature_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < DATA_MODELS_SETTINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn data_models_settings_historical_pool() -> PgPool {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_data_models_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();
    pool
}

fn before_model_providers_settings_feature_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < MODEL_PROVIDERS_SETTINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn model_providers_settings_historical_pool() -> PgPool {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_model_providers_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();
    pool
}

fn before_final_settings_feature_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < FINAL_SETTINGS_MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn final_settings_historical_pool() -> PgPool {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    before_final_settings_feature_migrator()
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

async fn seed_explicit_settings_historical_permissions(pool: &PgPool) {
    for (code, resource, action, scope) in [
        (
            AUTH_CENTER_VISIBILITY,
            "settings_route",
            "visible",
            "settings.auth-center",
        ),
        (
            HOST_INFRASTRUCTURE_VISIBILITY,
            "settings_route",
            "visible",
            "settings.host-infrastructure",
        ),
        (
            MEMORY_OBSERVATION_VISIBILITY,
            "settings_route",
            "visible",
            "settings.memory-observation",
        ),
        (
            APPLICATIONS_VISIBILITY,
            "settings_route",
            "visible",
            "settings.applications",
        ),
        ("user.view.all", "user", "view", "all"),
        ("user.manage.all", "user", "manage", "all"),
        ("plugin_config.view.all", "plugin_config", "view", "all"),
        (
            "plugin_config.configure.all",
            "plugin_config",
            "configure",
            "all",
        ),
        ("application.view.all", "application", "view", "all"),
    ] {
        insert_permission(pool, code, resource, action, scope).await;
    }
    for code in UNMIGRATED_VISIBILITIES {
        insert_permission(
            pool,
            code,
            "settings_route",
            "visible",
            code.trim_start_matches("settings_route.visible."),
        )
        .await;
    }
}

async fn seed_files_settings_historical_permissions(pool: &PgPool) {
    for (code, resource, action, scope) in [
        (
            FILES_VISIBILITY,
            "settings_route",
            "visible",
            "settings.files",
        ),
        ("file_storage.view.all", "file_storage", "view", "all"),
        ("file_storage.manage.all", "file_storage", "manage", "all"),
        ("file_table.view.all", "file_table", "view", "all"),
        ("file_table.view.own", "file_table", "view", "own"),
        ("file_table.create.all", "file_table", "create", "all"),
        ("file_table.delete.all", "file_table", "delete", "all"),
        ("file_table.delete.own", "file_table", "delete", "own"),
        ("file_table.bind.all", "file_table", "bind", "all"),
    ] {
        insert_permission(pool, code, resource, action, scope).await;
    }
    for code in REMAINING_VISIBILITIES_AFTER_FILES {
        insert_permission(
            pool,
            code,
            "settings_route",
            "visible",
            code.trim_start_matches("settings_route.visible."),
        )
        .await;
    }
}

async fn seed_data_models_settings_historical_permissions(pool: &PgPool) {
    insert_permission(
        pool,
        DATA_MODELS_VISIBILITY,
        "settings_route",
        "visible",
        "settings.data-models",
    )
    .await;
    for code in DATA_MODEL_ALL_PERMISSIONS {
        let (resource, action, scope) = if *code == "api_reference.view.all" {
            ("api_reference", "view", "all")
        } else {
            let mut parts = code.rsplitn(3, '.');
            let scope = parts.next().unwrap();
            let action = parts.next().unwrap();
            let resource = parts.next().unwrap();
            (resource, action, scope)
        };
        insert_permission(pool, code, resource, action, scope).await;
    }
    for code in REMAINING_VISIBILITIES_AFTER_DATA_MODELS {
        insert_permission(
            pool,
            code,
            "settings_route",
            "visible",
            code.trim_start_matches("settings_route.visible."),
        )
        .await;
    }
}

async fn seed_model_providers_settings_historical_permissions(pool: &PgPool) {
    insert_permission(
        pool,
        MODEL_PROVIDERS_VISIBILITY,
        "settings_route",
        "visible",
        "settings.model-providers",
    )
    .await;
    for code in MODEL_PROVIDER_ALL_PERMISSIONS {
        let mut parts = code.rsplitn(3, '.');
        let scope = parts.next().unwrap();
        let action = parts.next().unwrap();
        let resource = parts.next().unwrap();
        insert_permission(pool, code, resource, action, scope).await;
    }
    for code in REMAINING_VISIBILITIES_AFTER_MODEL_PROVIDERS {
        insert_permission(
            pool,
            code,
            "settings_route",
            "visible",
            code.trim_start_matches("settings_route.visible."),
        )
        .await;
    }
}

async fn seed_final_settings_historical_permissions(pool: &PgPool) {
    for (code, resource, action, scope) in [
        (
            DOCS_VISIBILITY,
            "settings_route",
            "visible",
            "settings.docs",
        ),
        (
            API_KEY_AUTHENTICATION_VISIBILITY,
            "settings_route",
            "visible",
            "settings.api-key-authentication",
        ),
        (
            SYSTEM_RUNTIME_VISIBILITY,
            "settings_route",
            "visible",
            "settings.system-runtime",
        ),
        (
            MCP_MANAGEMENT_VISIBILITY,
            "settings_route",
            "visible",
            "settings.mcp-management",
        ),
        ("api_reference.view.all", "api_reference", "view", "all"),
        ("system_runtime.view.all", "system_runtime", "view", "all"),
        ("mcp_management.view.all", "mcp_management", "view", "all"),
        (
            "mcp_management.manage.all",
            "mcp_management",
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
            .bind([MEMBERS_VISIBILITY, ROLES_VISIBILITY])
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
            .bind([MEMBERS_FEATURE, ROLES_FEATURE])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn migration_reconciles_four_explicit_settings_features_and_preserves_other_seven() {
    let pool = explicit_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Explicit settings migration")
        .await
        .unwrap();
    seed_explicit_settings_historical_permissions(&pool).await;

    let auth_role = insert_role(&pool, workspace.id, "legacy_auth_center").await;
    let host_role = insert_role(&pool, workspace.id, "legacy_host_infrastructure").await;
    let memory_role = insert_role(&pool, workspace.id, "legacy_memory_observation").await;
    let applications_role = insert_role(&pool, workspace.id, "legacy_applications").await;
    let untouched_role = insert_role(&pool, workspace.id, "legacy_unmigrated_settings").await;
    grant(&pool, auth_role, workspace.id, AUTH_CENTER_VISIBILITY).await;
    grant(
        &pool,
        host_role,
        workspace.id,
        HOST_INFRASTRUCTURE_VISIBILITY,
    )
    .await;
    grant(
        &pool,
        memory_role,
        workspace.id,
        MEMORY_OBSERVATION_VISIBILITY,
    )
    .await;
    grant(
        &pool,
        applications_role,
        workspace.id,
        APPLICATIONS_VISIBILITY,
    )
    .await;
    for code in UNMIGRATED_VISIBILITIES {
        grant(&pool, untouched_role, workspace.id, code).await;
    }

    before_files_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();

    assert_eq!(
        permission_codes(&pool, auth_role).await,
        BTreeSet::from([
            AUTH_CENTER_FEATURE.to_string(),
            "user.view.all".to_string(),
            "user.manage.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, host_role).await,
        BTreeSet::from([
            HOST_INFRASTRUCTURE_FEATURE.to_string(),
            "plugin_config.view.all".to_string(),
            "plugin_config.configure.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, memory_role).await,
        BTreeSet::from([
            MEMORY_OBSERVATION_FEATURE.to_string(),
            "plugin_config.view.all".to_string(),
            "plugin_config.configure.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, applications_role).await,
        BTreeSet::from([
            APPLICATIONS_FEATURE.to_string(),
            "application.view.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        UNMIGRATED_VISIBILITIES
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );

    let removed_legacy_definitions: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = any($1)")
            .bind([
                AUTH_CENTER_VISIBILITY,
                HOST_INFRASTRUCTURE_VISIBILITY,
                MEMORY_OBSERVATION_VISIBILITY,
                APPLICATIONS_VISIBILITY,
            ])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definitions, 0);

    for (role_code, feature_code) in [
        ("new_auth_center_feature_only", AUTH_CENTER_FEATURE),
        (
            "new_host_infrastructure_feature_only",
            HOST_INFRASTRUCTURE_FEATURE,
        ),
        (
            "new_memory_observation_feature_only",
            MEMORY_OBSERVATION_FEATURE,
        ),
        ("new_applications_feature_only", APPLICATIONS_FEATURE),
    ] {
        let new_feature_only = insert_role(&pool, workspace.id, role_code).await;
        grant(&pool, new_feature_only, workspace.id, feature_code).await;
        assert_eq!(
            permission_codes(&pool, new_feature_only).await,
            BTreeSet::from([feature_code.to_string()])
        );
    }
}

#[tokio::test]
async fn explicit_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = explicit_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Explicit settings rollback")
        .await
        .unwrap();
    seed_explicit_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace.id, "legacy_auth_center").await;
    grant(&pool, role_id, workspace.id, AUTH_CENTER_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_explicit_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.auth-center' then
                raise exception 'forced explicit settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_explicit_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_explicit_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = before_files_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("forced explicit settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([AUTH_CENTER_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = any($1)")
            .bind([
                AUTH_CENTER_FEATURE,
                HOST_INFRASTRUCTURE_FEATURE,
                MEMORY_OBSERVATION_FEATURE,
                APPLICATIONS_FEATURE,
            ])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn migration_reconciles_files_grants_and_preserves_remaining_six_visibilities() {
    let pool = files_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Files settings migration")
        .await
        .unwrap();
    seed_files_settings_historical_permissions(&pool).await;

    let files_role = insert_role(&pool, workspace.id, "legacy_files").await;
    let untouched_role = insert_role(&pool, workspace.id, "legacy_remaining_settings").await;
    grant(&pool, files_role, workspace.id, FILES_VISIBILITY).await;
    grant(&pool, files_role, workspace.id, "file_table.view.all").await;
    for code in REMAINING_VISIBILITIES_AFTER_FILES {
        grant(&pool, untouched_role, workspace.id, code).await;
    }

    before_data_models_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();

    assert_eq!(
        permission_codes(&pool, files_role).await,
        BTreeSet::from([
            FILES_FEATURE.to_string(),
            "file_storage.view.all".to_string(),
            "file_storage.manage.all".to_string(),
            "file_table.view.all".to_string(),
            "file_table.view.own".to_string(),
            "file_table.create.all".to_string(),
            "file_table.delete.all".to_string(),
            "file_table.delete.own".to_string(),
            "file_table.bind.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        REMAINING_VISIBILITIES_AFTER_FILES
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );

    let removed_legacy_definitions: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(FILES_VISIBILITY)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definitions, 0);

    let new_feature_only = insert_role(&pool, workspace.id, "new_files_feature_only").await;
    grant(&pool, new_feature_only, workspace.id, FILES_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, new_feature_only).await,
        BTreeSet::from([FILES_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn files_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = files_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Files settings rollback")
        .await
        .unwrap();
    seed_files_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace.id, "legacy_files").await;
    grant(&pool, role_id, workspace.id, FILES_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_files_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.files' then
                raise exception 'forced files settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_files_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_files_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("forced files settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([FILES_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(FILES_FEATURE)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn migration_reconciles_data_models_grants_and_preserves_remaining_five_visibilities() {
    let pool = data_models_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Data models settings migration")
        .await
        .unwrap();
    seed_data_models_settings_historical_permissions(&pool).await;

    let data_models_role = insert_role(&pool, workspace.id, "legacy_data_models").await;
    let untouched_role = insert_role(&pool, workspace.id, "legacy_remaining_five").await;
    grant(
        &pool,
        data_models_role,
        workspace.id,
        DATA_MODELS_VISIBILITY,
    )
    .await;
    grant(
        &pool,
        data_models_role,
        workspace.id,
        "state_model.view.all",
    )
    .await;
    for code in REMAINING_VISIBILITIES_AFTER_DATA_MODELS {
        grant(&pool, untouched_role, workspace.id, code).await;
    }

    before_model_providers_settings_feature_migrator()
        .run(&pool)
        .await
        .unwrap();

    let mut expected = DATA_MODEL_ALL_PERMISSIONS
        .iter()
        .map(|code| (*code).to_string())
        .collect::<BTreeSet<_>>();
    expected.insert(DATA_MODELS_FEATURE.to_string());
    assert_eq!(permission_codes(&pool, data_models_role).await, expected);
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        REMAINING_VISIBILITIES_AFTER_DATA_MODELS
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );
    let removed_legacy_definition: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(DATA_MODELS_VISIBILITY)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definition, 0);

    let feature_only = insert_role(&pool, workspace.id, "new_data_models_feature_only").await;
    grant(&pool, feature_only, workspace.id, DATA_MODELS_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, feature_only).await,
        BTreeSet::from([DATA_MODELS_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn data_models_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = data_models_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Data models settings rollback")
        .await
        .unwrap();
    seed_data_models_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace.id, "legacy_data_models").await;
    grant(&pool, role_id, workspace.id, DATA_MODELS_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_data_models_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.data-models' then
                raise exception 'forced data models settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_data_models_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_data_models_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("forced data models settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([DATA_MODELS_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(DATA_MODELS_FEATURE)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn migration_reconciles_model_providers_grants_and_preserves_remaining_four_visibilities() {
    let pool = model_providers_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Model providers settings migration")
        .await
        .unwrap();
    seed_model_providers_settings_historical_permissions(&pool).await;

    let legacy_role = insert_role(&pool, workspace.id, "legacy_model_providers").await;
    let untouched_role = insert_role(&pool, workspace.id, "legacy_remaining_four").await;
    grant(&pool, legacy_role, workspace.id, MODEL_PROVIDERS_VISIBILITY).await;
    grant(&pool, legacy_role, workspace.id, "state_model.view.all").await;
    for code in REMAINING_VISIBILITIES_AFTER_MODEL_PROVIDERS {
        grant(&pool, untouched_role, workspace.id, code).await;
    }

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let mut expected = MODEL_PROVIDER_ALL_PERMISSIONS
        .iter()
        .map(|code| (*code).to_string())
        .collect::<BTreeSet<_>>();
    expected.insert(MODEL_PROVIDERS_FEATURE.to_string());
    assert_eq!(permission_codes(&pool, legacy_role).await, expected);
    assert_eq!(
        permission_codes(&pool, untouched_role).await,
        REMAINING_VISIBILITIES_AFTER_MODEL_PROVIDERS
            .iter()
            .map(|code| (*code).to_string())
            .collect()
    );
    let removed_legacy_definition: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(MODEL_PROVIDERS_VISIBILITY)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(removed_legacy_definition, 0);

    let feature_only = insert_role(&pool, workspace.id, "new_model_providers_feature_only").await;
    grant(&pool, feature_only, workspace.id, MODEL_PROVIDERS_FEATURE).await;
    assert_eq!(
        permission_codes(&pool, feature_only).await,
        BTreeSet::from([MODEL_PROVIDERS_FEATURE.to_string()])
    );
}

#[tokio::test]
async fn model_providers_settings_migration_rolls_back_when_legacy_cleanup_fails() {
    let pool = model_providers_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Model providers settings rollback")
        .await
        .unwrap();
    seed_model_providers_settings_historical_permissions(&pool).await;
    let role_id = insert_role(&pool, workspace.id, "legacy_model_providers").await;
    grant(&pool, role_id, workspace.id, MODEL_PROVIDERS_VISIBILITY).await;

    sqlx::raw_sql(
        r#"
        create function reject_model_providers_settings_visibility_delete() returns trigger language plpgsql as $$
        begin
            if old.code = 'settings_route.visible.settings.model-providers' then
                raise exception 'forced model providers settings cleanup failure';
            end if;
            return old;
        end;
        $$;
        create trigger reject_model_providers_settings_visibility_delete
        before delete on permission_definitions
        for each row execute function reject_model_providers_settings_visibility_delete();
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(error
        .to_string()
        .contains("forced model providers settings cleanup failure"));
    assert_eq!(
        permission_codes(&pool, role_id).await,
        BTreeSet::from([MODEL_PROVIDERS_VISIBILITY.to_string()])
    );
    let new_definition_count: i64 =
        sqlx::query_scalar("select count(*) from permission_definitions where code = $1")
            .bind(MODEL_PROVIDERS_FEATURE)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(new_definition_count, 0);
}

#[tokio::test]
async fn final_settings_migration_reconciles_all_remaining_legacy_grants() {
    let pool = final_settings_historical_pool().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Final settings migration")
        .await
        .unwrap();
    seed_final_settings_historical_permissions(&pool).await;

    let docs_role = insert_role(&pool, workspace.id, "legacy_docs").await;
    let api_key_role = insert_role(&pool, workspace.id, "legacy_api_key").await;
    let system_runtime_role = insert_role(&pool, workspace.id, "legacy_system_runtime").await;
    let mcp_role = insert_role(&pool, workspace.id, "legacy_mcp").await;
    grant(&pool, docs_role, workspace.id, DOCS_VISIBILITY).await;
    grant(
        &pool,
        api_key_role,
        workspace.id,
        API_KEY_AUTHENTICATION_VISIBILITY,
    )
    .await;
    grant(
        &pool,
        system_runtime_role,
        workspace.id,
        SYSTEM_RUNTIME_VISIBILITY,
    )
    .await;
    grant(&pool, mcp_role, workspace.id, MCP_MANAGEMENT_VISIBILITY).await;

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    assert_eq!(
        permission_codes(&pool, docs_role).await,
        BTreeSet::from([
            DOCS_FEATURE.to_string(),
            "api_reference.view.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, api_key_role).await,
        BTreeSet::from([API_KEY_AUTHENTICATION_FEATURE.to_string()])
    );
    assert_eq!(
        permission_codes(&pool, system_runtime_role).await,
        BTreeSet::from([
            SYSTEM_RUNTIME_FEATURE.to_string(),
            "system_runtime.view.all".to_string(),
        ])
    );
    assert_eq!(
        permission_codes(&pool, mcp_role).await,
        BTreeSet::from([
            MCP_MANAGEMENT_FEATURE.to_string(),
            "mcp_management.manage.all".to_string(),
            "mcp_management.view.all".to_string(),
        ])
    );

    let legacy_definition_count: i64 = sqlx::query_scalar(
        "select count(*) from permission_definitions where code like 'settings_route.visible.%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_definition_count, 0);
}
