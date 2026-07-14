use control_plane::application::console_policy_migration::{
    applications_console_policy_catalog, applications_legacy_console_grant_mappings,
    applications_legacy_console_policy_source,
};
use control_plane::ports::{
    CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput,
    RoleConsolePolicyMigrationRehearsalInput, RoleConsolePolicyMigrationRepository, RoleRepository,
};
use control_plane::role::console_policy_migration::project_legacy_role_console_policy;
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

async fn grant_legacy_application_permissions(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    role_code: &str,
    permission_codes: &[&str],
) {
    let role_id: Uuid =
        sqlx::query_scalar("select id from roles where workspace_id = $1 and code = $2")
            .bind(workspace_id)
            .bind(role_code)
            .fetch_one(store.pool())
            .await
            .unwrap();
    for code in permission_codes {
        let parts = code.split('.').collect::<Vec<_>>();
        let (resource, action, scope) = if parts[0] == "settings_feature" {
            ("settings_feature", "access", "system.applications")
        } else {
            (parts[0], parts[1], parts[2])
        };
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
        .execute(store.pool())
        .await
        .unwrap();
        sqlx::query(
            r#"
            insert into role_permissions (id, role_id, permission_id, scope_id)
            select $1, $2, id, $3 from permission_definitions where code = $4
            on conflict (role_id, permission_id) do nothing
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(role_id)
        .bind(workspace_id)
        .bind(code)
        .execute(store.pool())
        .await
        .unwrap();
    }
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

async fn applications_migration_input(
    store: &PgControlPlaneStore,
) -> RoleConsolePolicyMigrationRehearsalInput {
    let source = applications_legacy_console_policy_source();
    let inventories = store
        .list_role_console_policy_migration_grants(&source)
        .await
        .unwrap();
    let previews = inventories
        .iter()
        .map(|inventory| {
            project_legacy_role_console_policy(
                inventory.role_id,
                &inventory.source_grants,
                &applications_console_policy_catalog(),
                &applications_legacy_console_grant_mappings(),
            )
            .unwrap()
        })
        .collect();
    RoleConsolePolicyMigrationRehearsalInput {
        run_id: Uuid::now_v7(),
        source_contract: "applications-legacy/v1".into(),
        catalog_fingerprint: "applications-crud+settings-feature/v1".into(),
        mapping_fingerprint: "applications-known-grants/v1".into(),
        source,
        previews,
    }
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

#[tokio::test]
async fn ac_010_applications_console_policy_sql_rehearsal_preserves_exact_and_partial_profiles() {
    let (store, workspace_id, actor_user_id) = role_store().await;
    RoleRepository::create_team_role(
        &store,
        &CreateWorkspaceRoleInput {
            actor_user_id,
            workspace_id,
            code: "viewer".into(),
            name: "Viewer".into(),
            introduction: String::new(),
            auto_grant_new_permissions: false,
            is_default_member_role: false,
        },
    )
    .await
    .unwrap();
    grant_legacy_application_permissions(
        &store,
        workspace_id,
        "operator",
        &[
            access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION,
            "application.create.all",
            "application.view.all",
            "application.edit.all",
            "application.delete.all",
        ],
    )
    .await;
    grant_legacy_application_permissions(&store, workspace_id, "viewer", &["application.view.own"])
        .await;

    sqlx::raw_sql(include_str!(
        "../migrations/20260714233000_migrate_applications_console_policies.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();

    let exact = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert_eq!(exact.groups()[0].mode(), domain::ConsolePolicyMode::Full);
    assert!(exact.groups()[0].operations().is_empty());

    let partial = RoleRepository::get_role_console_policy(&store, workspace_id, "viewer")
        .await
        .unwrap();
    assert_eq!(
        partial.groups()[0].mode(),
        domain::ConsolePolicyMode::Custom
    );
    assert_eq!(partial.groups()[0].operations().len(), 1);
    assert_eq!(
        partial.groups()[0].operations()[0].operation_id().as_str(),
        access_control::APPLICATIONS_VIEW_OPERATION_ID
    );
    assert_eq!(
        partial.groups()[0].operations()[0].row_scope(),
        Some(ConsoleOperationRowScope::Own)
    );
}

#[tokio::test]
async fn ac_011_rehearsal_fences_apply_and_verified_rollback_restores_pre_apply_policy() {
    let (store, workspace_id, actor_user_id) = role_store().await;
    grant_legacy_application_permissions(
        &store,
        workspace_id,
        "operator",
        &[
            access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION,
            "application.create.all",
            "application.view.all",
            "application.edit.all",
            "application.delete.all",
        ],
    )
    .await;

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

    let input = applications_migration_input(&store).await;

    store
        .rehearse_role_console_policy_migration(&input)
        .await
        .unwrap();
    let deterministic_role_previews: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from role_console_policy_migration_role_previews
        where run_id = $1
          and effective_before = effective_after
          and effective_delta = '[]'::jsonb
          and authorization_delta = '{"added": [], "removed": []}'::jsonb
        "#,
    )
    .bind(input.run_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(deterministic_role_previews, input.previews.len() as i64);
    store
        .apply_role_console_policy_migration(&input, actor_user_id)
        .await
        .unwrap();

    let applied_run_state: (String, String, bool) = sqlx::query_as(
        "select status, cutover_marker, write_fenced from role_console_policy_migration_runs where id = $1",
    )
    .bind(input.run_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        applied_run_state,
        ("applied_fenced".into(), "console_policy".into(), true)
    );

    let applied = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert_eq!(applied.groups()[0].mode(), domain::ConsolePolicyMode::Full);
    let fenced_write = RoleRepository::replace_role_console_policy(
        &store,
        &ReplaceRoleConsolePolicyInput {
            actor_user_id,
            workspace_id,
            role_code: "operator".into(),
            groups: vec![custom_group(ConsoleOperationRowScope::ScopeAll)],
        },
    )
    .await;
    assert!(fenced_write
        .unwrap_err()
        .to_string()
        .contains("console policy migration write fence"));
    let fenced_legacy_write = sqlx::query(
        r#"
        delete from role_permissions grant_row
        using permission_definitions definition, roles role
        where grant_row.permission_id = definition.id
          and grant_row.role_id = role.id
          and role.workspace_id = $1
          and role.code = 'operator'
          and definition.code = 'application.view.all'
        "#,
    )
    .bind(workspace_id)
    .execute(store.pool())
    .await;
    assert!(fenced_legacy_write
        .unwrap_err()
        .to_string()
        .contains("console policy migration write fence"));

    store
        .rollback_role_console_policy_migration(input.run_id, actor_user_id)
        .await
        .unwrap();
    let restored = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert_eq!(
        restored.groups(),
        vec![custom_group(ConsoleOperationRowScope::Own)].as_slice()
    );

    let retained_legacy_grants: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from role_permissions grant_row
        join permission_definitions definition on definition.id = grant_row.permission_id
        join roles role on role.id = grant_row.role_id
        where role.workspace_id = $1
          and role.code = 'operator'
          and (
            definition.resource = 'application'
            or definition.code = $2
          )
        "#,
    )
    .bind(workspace_id)
    .bind(access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(retained_legacy_grants, 5);

    let run_state: (String, String, bool, bool) = sqlx::query_as(
        r#"
        select status, cutover_marker, write_fenced, rollback_verified_at is not null
        from role_console_policy_migration_runs
        where id = $1
        "#,
    )
    .bind(input.run_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        run_state,
        ("rolled_back".into(), "legacy".into(), false, true)
    );
}

#[tokio::test]
async fn ac_011_finalize_releases_fence_and_prevents_destructive_rollback_after_user_edits() {
    let (store, workspace_id, actor_user_id) = role_store().await;
    grant_legacy_application_permissions(
        &store,
        workspace_id,
        "operator",
        &["application.view.all"],
    )
    .await;
    let input = applications_migration_input(&store).await;
    store
        .rehearse_role_console_policy_migration(&input)
        .await
        .unwrap();
    store
        .apply_role_console_policy_migration(&input, actor_user_id)
        .await
        .unwrap();

    let write_while_fenced = RoleRepository::replace_role_console_policy(
        &store,
        &ReplaceRoleConsolePolicyInput {
            actor_user_id,
            workspace_id,
            role_code: "operator".into(),
            groups: vec![custom_group(ConsoleOperationRowScope::Own)],
        },
    )
    .await;
    assert!(write_while_fenced
        .unwrap_err()
        .to_string()
        .contains("console policy migration write fence"));

    store
        .finalize_role_console_policy_migration(input.run_id, actor_user_id)
        .await
        .unwrap();
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

    let rollback_after_finalize = store
        .rollback_role_console_policy_migration(input.run_id, actor_user_id)
        .await;
    assert!(rollback_after_finalize
        .unwrap_err()
        .to_string()
        .contains("console_policy_migration_state"));
    let edited = RoleRepository::get_role_console_policy(&store, workspace_id, "operator")
        .await
        .unwrap();
    assert_eq!(
        edited.groups(),
        vec![custom_group(ConsoleOperationRowScope::Own)].as_slice()
    );

    let run_state: (String, String, bool, bool) = sqlx::query_as(
        r#"
        select status, cutover_marker, write_fenced,
               finalized_by = $2 and finalized_at is not null
        from role_console_policy_migration_runs
        where id = $1
        "#,
    )
    .bind(input.run_id)
    .bind(actor_user_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        run_state,
        ("applied".into(), "console_policy".into(), false, true)
    );
}

#[tokio::test]
async fn ac_011_migration_run_schema_rejects_finalized_actor_before_finalize() {
    let (store, _, actor_user_id) = role_store().await;
    let result = sqlx::query(
        r#"
        insert into role_console_policy_migration_runs (
          id, source_contract, catalog_fingerprint, mapping_fingerprint,
          source_filter, source_snapshot, status, cutover_marker, write_fenced,
          finalized_by
        )
        values (
          $1, 'applications-legacy/v1', 'catalog/v1', 'mapping/v1',
          '{}', '{}', 'previewed', 'legacy', false, $2
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(actor_user_id)
    .execute(store.pool())
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ac_010_application_namespace_inventory_exposes_unknown_and_ignores_unrelated_grants() {
    let (store, workspace_id, _) = role_store().await;
    grant_legacy_application_permissions(
        &store,
        workspace_id,
        "operator",
        &["application.publish.all", "workflow.view.all"],
    )
    .await;
    let source = applications_legacy_console_policy_source();
    let inventories = store
        .list_role_console_policy_migration_grants(&source)
        .await
        .unwrap();
    let operator = inventories
        .iter()
        .find(|inventory| inventory.role_code == "operator")
        .unwrap();
    assert_eq!(operator.source_grants, vec!["application.publish.all"]);

    let error = project_legacy_role_console_policy(
        operator.role_id,
        &operator.source_grants,
        &applications_console_policy_catalog(),
        &applications_legacy_console_grant_mappings(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("unknown legacy grant"));
}
