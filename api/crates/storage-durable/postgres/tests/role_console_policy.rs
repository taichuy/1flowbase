use control_plane::ports::{
    CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput, RoleRepository,
};
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    RoleConsoleGroupPolicy,
};
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

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

async fn role_store() -> (PgControlPlaneStore, Uuid, Uuid) {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "console-policy")
        .await
        .unwrap();
    let actor_user_id = Uuid::now_v7();
    RoleRepository::create_team_role(
        &store,
        &CreateWorkspaceRoleInput {
            actor_user_id,
            workspace_id: workspace.id,
            code: "operator".into(),
            name: "Operator".into(),
            introduction: String::new(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        },
    )
    .await
    .unwrap();
    (store, workspace.id, actor_user_id)
}

fn group() -> ConsolePolicyGroup {
    ConsolePolicyGroup::settings_feature("system.applications").unwrap()
}

fn custom_group(scope: ConsoleOperationRowScope) -> RoleConsoleGroupPolicy {
    RoleConsoleGroupPolicy::custom(
        group(),
        vec![ConsoleOperationPolicy::row(
            ConsoleOperationId::try_from("applications.view").unwrap(),
            scope,
        )],
    )
}

#[tokio::test]
async fn ac_004_console_policy_repository_round_trips_custom_and_full_without_materializing_full() {
    let (store, workspace_id, actor_user_id) = role_store().await;
    RoleRepository::replace_role_console_policy(
        &store,
        &ReplaceRoleConsolePolicyInput {
            actor_user_id,
            workspace_id,
            role_code: "operator".into(),
            groups: vec![custom_group(ConsoleOperationRowScope::Own)],
        },
    )
    .await
    .unwrap();
    let custom = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert_eq!(custom.groups()[0].operations().len(), 1);

    RoleRepository::replace_role_console_policy(
        &store,
        &ReplaceRoleConsolePolicyInput {
            actor_user_id,
            workspace_id,
            role_code: "operator".into(),
            groups: vec![RoleConsoleGroupPolicy::full(group())],
        },
    )
    .await
    .unwrap();
    let full = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert!(full.groups()[0].operations().is_empty());

    let operation_rows: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from role_console_operation_policies operation_policy
        join roles role on role.id = operation_policy.role_id
        where role.workspace_id = $1 and role.code = 'operator'
        "#,
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(operation_rows, 0);

    let group_policy_id: Uuid = sqlx::query_scalar(
        r#"
        select group_policy.id
        from role_console_group_policies group_policy
        join roles role on role.id = group_policy.role_id
        where role.workspace_id = $1 and role.code = 'operator'
        "#,
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let materialized_full = sqlx::query(
        r#"
        insert into role_console_operation_policies (
          id, role_id, group_policy_id, group_mode, operation_id, policy_kind,
          simple_enabled, row_scope
        )
        values ($1, $2, $3, 'custom', 'applications.create', 'simple', true, null)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(full.role_id())
    .bind(group_policy_id)
    .execute(store.pool())
    .await;
    assert!(materialized_full.is_err());
}

#[tokio::test]
async fn ac_006_console_policy_schema_rejects_system_all() {
    let (store, workspace_id, _) = role_store().await;
    let role_id: Uuid =
        sqlx::query_scalar("select id from roles where workspace_id = $1 and code = 'operator'")
            .bind(workspace_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let group_policy_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into role_console_group_policies
          (id, role_id, group_kind, group_id, mode)
        values ($1, $2, 'settings_feature', 'system.applications', 'custom')
        "#,
    )
    .bind(group_policy_id)
    .bind(role_id)
    .execute(store.pool())
    .await
    .unwrap();

    let result = sqlx::query(
        r#"
        insert into role_console_operation_policies
          (id, role_id, group_policy_id, group_mode, operation_id, policy_kind, simple_enabled, row_scope)
        values ($1, $2, $3, 'custom', 'applications.view', 'row', null, 'system_all')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(role_id)
    .bind(group_policy_id)
    .execute(store.pool())
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ac_011_console_policy_replacement_rolls_back_on_constraint_failure() {
    let (store, workspace_id, actor_user_id) = role_store().await;
    let initial = ReplaceRoleConsolePolicyInput {
        actor_user_id,
        workspace_id,
        role_code: "operator".into(),
        groups: vec![custom_group(ConsoleOperationRowScope::Own)],
    };
    RoleRepository::replace_role_console_policy(&store, &initial)
        .await
        .unwrap();

    let duplicate_group = ReplaceRoleConsolePolicyInput {
        actor_user_id,
        workspace_id,
        role_code: "operator".into(),
        groups: vec![
            RoleConsoleGroupPolicy::full(group()),
            RoleConsoleGroupPolicy::full(group()),
        ],
    };
    assert!(
        RoleRepository::replace_role_console_policy(&store, &duplicate_group)
            .await
            .is_err()
    );

    let persisted = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert_eq!(persisted.groups(), initial.groups.as_slice());
}

#[tokio::test]
async fn ac_010_console_policy_migration_ledger_blocks_unsafe_apply() {
    let (store, workspace_id, _) = role_store().await;
    let role_id: Uuid =
        sqlx::query_scalar("select id from roles where workspace_id = $1 and code = 'operator'")
            .bind(workspace_id)
            .fetch_one(store.pool())
            .await
            .unwrap();

    for (catalog_complete, authorization_delta) in [
        (false, serde_json::json!({"added": [], "removed": []})),
        (
            true,
            serde_json::json!({
                "added": [{"operation_id": "applications.view"}],
                "removed": []
            }),
        ),
    ] {
        let result = sqlx::query(
            r#"
            insert into role_console_policy_migration_ledger (
              id, role_id, source_contract, catalog_fingerprint, mapping_fingerprint,
              catalog_complete, source_grants, projected_policy, authorization_delta,
              status, applied_at
            )
            values ($1, $2, 'legacy/v1', $3, 'mapping/v1', $4, '[]', '{}', $5, 'applied', now())
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(role_id)
        .bind(Uuid::now_v7().to_string())
        .bind(catalog_complete)
        .bind(authorization_delta)
        .execute(store.pool())
        .await;
        assert!(result.is_err());
    }

    let safe_apply = sqlx::query(
        r#"
        insert into role_console_policy_migration_ledger (
          id, role_id, source_contract, catalog_fingerprint, mapping_fingerprint,
          catalog_complete, source_grants, projected_policy, authorization_delta,
          status, applied_at
        )
        values (
          $1, $2, 'legacy/v1', 'catalog/complete', 'mapping/v1', true,
          '[]', '{}', '{"added": [], "removed": []}', 'applied', now()
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(role_id)
    .execute(store.pool())
    .await;
    assert!(safe_apply.is_ok());
}
