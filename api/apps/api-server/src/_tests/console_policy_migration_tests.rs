use std::borrow::Cow;

use access_control::{ConsoleAuthorization, ConsolePolicyGroup, SettingsFeatureOwnerKind};
use control_plane::ports::{
    RoleConsolePolicyMigrationCutoverMarker, RoleConsolePolicyMigrationRepository,
};
use control_plane::role::console_policy_migration::compile_console_policy_migration_probes;
use control_plane::role::console_policy_migration::ConsolePolicyMigrationLegacyGrantProjection;
use sqlx::{migrate::Migrator, PgPool};
use uuid::Uuid;

use crate::{
    app_state::{compile_core_console_operation_registry, compile_core_settings_feature_registry},
    console_policy_migration::{
        compile_core_console_policy_migration_plan, parse_command, preview_live_migration,
        require_runtime_console_policy_cutover, write_evidence_report,
        ConsolePolicyMigrationCommand, ConsolePolicyMigrationEvidenceReport,
    },
};

const HISTORICAL_SETTINGS_VISIBILITIES: &[&str] = &[
    "settings_route.visible.settings.members",
    "settings_route.visible.settings.roles",
    "settings_route.visible.settings.auth-center",
    "settings_route.visible.settings.host-infrastructure",
    "settings_route.visible.settings.memory-observation",
    "settings_route.visible.settings.applications",
    "settings_route.visible.settings.files",
    "settings_route.visible.settings.data-models",
    "settings_route.visible.settings.model-providers",
    "settings_route.visible.settings.docs",
    "settings_route.visible.settings.api-key-authentication",
    "settings_route.visible.settings.system-runtime",
    "settings_route.visible.settings.mcp-management",
];
const HISTORICAL_SETTINGS_MIGRATION_TARGETS: &[&str] = &[
    "api_reference.view.all",
    "application.view.all",
    "external_data_source.configure.all",
    "external_data_source.configure.own",
    "external_data_source.create.all",
    "external_data_source.delete.all",
    "external_data_source.delete.own",
    "external_data_source.edit.all",
    "external_data_source.edit.own",
    "external_data_source.use.all",
    "external_data_source.use.own",
    "external_data_source.view.all",
    "external_data_source.view.own",
    "file_storage.manage.all",
    "file_storage.view.all",
    "file_table.bind.all",
    "file_table.create.all",
    "file_table.delete.all",
    "file_table.delete.own",
    "file_table.view.all",
    "file_table.view.own",
    "mcp_management.manage.all",
    "mcp_management.view.all",
    "plugin_config.configure.all",
    "plugin_config.view.all",
    "role_permission.manage.all",
    "role_permission.view.all",
    "state_model.create.all",
    "state_model.delete.all",
    "state_model.delete.own",
    "state_model.edit.all",
    "state_model.edit.own",
    "state_model.manage.all",
    "state_model.manage.own",
    "state_model.view.all",
    "state_model.view.own",
    "system_runtime.view.all",
    "user.manage.all",
    "user.view.all",
];

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_historical_pool(cutoff: i64) -> PgPool {
    let base_url = base_database_url();
    let pool = postgres_test_support::PostgresTestSchema::create(&base_url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    let historical = Migrator {
        migrations: Cow::Owned(
            sqlx::migrate!("../../crates/storage-durable/postgres/migrations")
                .iter()
                .filter(|migration| migration.version <= cutoff)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    historical.run(&pool).await.unwrap();
    pool
}

async fn seed_historical_console_fixture(
    pool: &PgPool,
    migration: &crate::console_policy_migration::CompiledCoreConsolePolicyMigration,
) {
    let tenant_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let role_id = Uuid::now_v7();
    let partial_role_id = Uuid::now_v7();
    let has_scoped_acl_columns: bool = sqlx::query_scalar(
        r#"
        select exists (
          select 1 from information_schema.columns
          where table_schema = current_schema()
            and table_name = 'roles'
            and column_name = 'scope_id'
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("insert into tenants (id, code, name, is_root) values ($1, $2, $2, true)")
        .bind(tenant_id)
        .bind(format!("tenant-{}", tenant_id.simple()))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("insert into workspaces (id, tenant_id, name) values ($1, $2, 'historical')")
        .bind(workspace_id)
        .bind(tenant_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'fixture', 'Fixture', 'Fixture', 'active')",
    )
    .bind(user_id)
    .bind(format!("actor-{}", user_id.simple()))
    .bind(format!("{}@example.com", user_id.simple()))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into workspace_memberships (id, workspace_id, user_id) values ($1, $2, $3)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
    let role_insert = if has_scoped_acl_columns {
        r#"
        insert into roles (
          id, scope_id, scope_kind, workspace_id, code, name, is_builtin, is_editable,
          auto_grant_new_permissions, is_default_member_role
        ) values ($1, $2, 'workspace', $2, $3, $3,
                  false, true, false, false)
        "#
    } else {
        r#"
        insert into roles (
          id, scope_kind, workspace_id, code, name, is_builtin, is_editable,
          auto_grant_new_permissions, is_default_member_role
        ) values ($1, 'workspace', $2, $3, $3,
                  false, true, false, false)
        "#
    };
    sqlx::query(role_insert)
        .bind(role_id)
        .bind(workspace_id)
        .bind("historical_operator")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(role_insert)
        .bind(partial_role_id)
        .bind(workspace_id)
        .bind("historical_partial")
        .execute(pool)
        .await
        .unwrap();
    let binding_insert = if has_scoped_acl_columns {
        "insert into user_role_bindings (id, scope_id, user_id, role_id) values ($1, $4, $2, $3)"
    } else {
        "with ignored as (select $4::uuid) insert into user_role_bindings (id, user_id, role_id) values ($1, $2, $3)"
    };
    sqlx::query(binding_insert)
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(role_id)
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(binding_insert)
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(partial_role_id)
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();

    let grants = migration
        .legacy_mappings()
        .iter()
        .map(|mapping| mapping.legacy_grant.as_str())
        .chain(HISTORICAL_SETTINGS_VISIBILITIES.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    let definitions = grants
        .iter()
        .copied()
        .chain(HISTORICAL_SETTINGS_MIGRATION_TARGETS.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    for code in definitions {
        let mut parts = code.splitn(3, '.');
        let resource = parts.next().unwrap();
        let action = parts.next().unwrap_or("access");
        let scope = parts.next().unwrap_or("all");
        let permission_id = Uuid::now_v7();
        let definition_insert = if has_scoped_acl_columns {
            "insert into permission_definitions (id, scope_id, resource, action, scope, code, name) values ($1, $6, $2, $3, $4, $5, $5) on conflict (code) do nothing"
        } else {
            "with ignored as (select $6::uuid) insert into permission_definitions (id, resource, action, scope, code, name) values ($1, $2, $3, $4, $5, $5) on conflict (code) do nothing"
        };
        sqlx::query(definition_insert)
            .bind(permission_id)
            .bind(resource)
            .bind(action)
            .bind(scope)
            .bind(code)
            .bind(domain::SYSTEM_SCOPE_ID)
            .execute(pool)
            .await
            .unwrap();
    }
    for code in grants {
        let grant_insert = if has_scoped_acl_columns {
            "insert into role_permissions (id, scope_id, role_id, permission_id) select $1, $4, $2, id from permission_definitions where code = $3 on conflict (role_id, permission_id) do nothing"
        } else {
            "with ignored as (select $4::uuid) insert into role_permissions (id, role_id, permission_id) select $1, $2, id from permission_definitions where code = $3 on conflict (role_id, permission_id) do nothing"
        };
        sqlx::query(grant_insert)
            .bind(Uuid::now_v7())
            .bind(role_id)
            .bind(code)
            .bind(workspace_id)
            .execute(pool)
            .await
            .unwrap();
    }
    let partial_grant_insert = if has_scoped_acl_columns {
        "insert into role_permissions (id, scope_id, role_id, permission_id) select $1, $4, $2, id from permission_definitions where code = $3"
    } else {
        "with ignored as (select $4::uuid) insert into role_permissions (id, role_id, permission_id) select $1, $2, id from permission_definitions where code = $3"
    };
    sqlx::query(partial_grant_insert)
        .bind(Uuid::now_v7())
        .bind(partial_role_id)
        .bind("application.view.own")
        .bind(workspace_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn verify_release_cohort(
    label: &str,
    cutoff: i64,
    releases: &[&str],
    migration: &crate::console_policy_migration::CompiledCoreConsolePolicyMigration,
) -> serde_json::Value {
    let pool = isolated_historical_pool(cutoff).await;
    seed_historical_console_fixture(&pool, migration).await;
    sqlx::migrate!("../../crates/storage-durable/postgres/migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|error| panic!("{label} must upgrade through current migrations: {error}"));
    let store = storage_durable::MainDurableStore::new(pool.clone());
    let actor_user_id: Uuid =
        sqlx::query_scalar("select id from users where account like 'actor-%' order by id limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    let rollback_run_id = Uuid::now_v7();
    let rollback_preview = preview_live_migration(&store, migration, rollback_run_id)
        .await
        .unwrap_or_else(|error| panic!("{label} live preview must execute: {error}"));
    assert!(
        rollback_preview.validation_errors.is_empty(),
        "unexpected {label} validation errors: {:?}; unknown grants: {:?}",
        rollback_preview.validation_errors,
        rollback_preview.unknown_grants
    );
    assert!(!rollback_preview.actor_previews.is_empty());
    let expected_probes = compile_console_policy_migration_probes(migration.plan()).unwrap();
    assert!(rollback_preview.actor_previews.iter().all(|preview| {
        preview
            .get("probes")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|probes| probes.len() == expected_probes.len())
            && preview
                .pointer("/binding/role_ids")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|role_ids| role_ids.len() == 2)
    }));
    let role_preview_count = rollback_preview.role_projections.len();
    let actor_preview_count = rollback_preview.actor_previews.len();
    let rollback_rehearsal = rollback_preview.rehearsal.unwrap();
    store
        .rehearse_role_console_policy_migration(&rollback_rehearsal)
        .await
        .unwrap();
    let (historical_role_id, historical_workspace_id): (Uuid, Uuid) =
        sqlx::query_as("select id, workspace_id from roles where code = 'historical_operator'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let policy_count_before_failed_apply: i64 =
        sqlx::query_scalar("select count(*) from role_console_group_policies where role_id = $1")
            .bind(historical_role_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let drift_permission_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into permission_definitions (
          id, scope_id, resource, action, scope, code, name
        ) values ($1, $2, 'external_data_source', 'fixture_drift', 'all',
                  'external_data_source.fixture_drift.all', 'fixture drift')
        "#,
    )
    .bind(drift_permission_id)
    .bind(domain::SYSTEM_SCOPE_ID)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into role_permissions (id, scope_id, role_id, permission_id) values ($1, $2, $3, $4)",
    )
    .bind(Uuid::now_v7())
    .bind(historical_workspace_id)
    .bind(historical_role_id)
    .bind(drift_permission_id)
    .execute(&pool)
    .await
    .unwrap();
    let failed_apply = store
        .apply_role_console_policy_migration(&rollback_rehearsal, actor_user_id)
        .await;
    assert!(failed_apply
        .expect_err("source drift must abort apply")
        .to_string()
        .contains("console_policy_migration_source_drift"));
    assert_eq!(
        store
            .role_console_policy_migration_cutover_state()
            .await
            .unwrap()
            .marker,
        RoleConsolePolicyMigrationCutoverMarker::Legacy
    );
    let policy_count: i64 =
        sqlx::query_scalar("select count(*) from role_console_group_policies where role_id = $1")
            .bind(historical_role_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        policy_count, policy_count_before_failed_apply,
        "failed apply must preserve the exact pre-apply policy count"
    );
    sqlx::query("delete from permission_definitions where id = $1")
        .bind(drift_permission_id)
        .execute(&pool)
        .await
        .unwrap();
    store
        .apply_role_console_policy_migration(&rollback_rehearsal, actor_user_id)
        .await
        .unwrap();
    let runtime_error = require_runtime_console_policy_cutover(&store)
        .await
        .expect_err("the fenced runtime marker must stop startup");
    assert!(runtime_error.to_string().contains("migration is fenced"));
    store
        .rollback_role_console_policy_migration(rollback_run_id, actor_user_id)
        .await
        .unwrap();
    assert_eq!(
        store
            .role_console_policy_migration_cutover_state()
            .await
            .unwrap()
            .marker,
        RoleConsolePolicyMigrationCutoverMarker::Legacy
    );

    let finalize_run_id = Uuid::now_v7();
    let finalize_preview = preview_live_migration(&store, migration, finalize_run_id)
        .await
        .unwrap();
    assert!(finalize_preview.validation_errors.is_empty());
    assert!(finalize_preview.actor_previews.iter().all(|preview| {
        preview.get("effective_before") == preview.get("effective_after")
            && preview
                .get("effective_delta")
                .and_then(serde_json::Value::as_array)
                .is_some_and(Vec::is_empty)
    }));
    let finalize_rehearsal = finalize_preview.rehearsal.unwrap();
    store
        .rehearse_role_console_policy_migration(&finalize_rehearsal)
        .await
        .unwrap();
    store
        .apply_role_console_policy_migration(&finalize_rehearsal, actor_user_id)
        .await
        .unwrap();
    store
        .finalize_role_console_policy_migration(finalize_run_id, actor_user_id)
        .await
        .unwrap();
    require_runtime_console_policy_cutover(&store)
        .await
        .expect("the finalized marker must allow the new runtime");
    assert_eq!(
        store
            .role_console_policy_migration_cutover_state()
            .await
            .unwrap()
            .marker,
        RoleConsolePolicyMigrationCutoverMarker::ConsolePolicy
    );

    pool.close().await;
    serde_json::json!({
        "cohort": label,
        "migration_cutoff": cutoff,
        "released_tags": releases,
        "role_previews": role_preview_count,
        "actor_previews": actor_preview_count,
        "probes_per_actor": expected_probes.len(),
        "multi_role_union_role_count": 2,
        "effective_delta": [],
        "rollback_marker": "legacy",
        "finalized_marker": "console_policy",
        "runtime_fenced_rejected": true,
        "runtime_finalized_allowed": true,
        "failed_apply_rolled_back": true,
    })
}

#[tokio::test]
async fn ac_010_ac_011_supported_release_schema_cohorts_rehearse_apply_finalize_and_rollback() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let cohorts: &[(&str, i64, &[&str])] = &[
        (
            "v0.1.6-v0.1.9",
            20260604120000,
            &["v0.1.6", "v0.1.7", "v0.1.8", "v0.1.9"],
        ),
        (
            "v0.1.10-v0.1.12",
            20260608110000,
            &["v0.1.10", "v0.1.11", "v0.1.12"],
        ),
        (
            "v0.2.0-v0.2.3",
            20260613102000,
            &["v0.2.0", "v0.2.1", "v0.2.2", "v0.2.3"],
        ),
        ("v0.2.4-v0.2.5", 20260619100000, &["v0.2.4", "v0.2.5"]),
        ("v0.2.6", 20260626103000, &["v0.2.6"]),
    ];
    let mut evidence = Vec::new();
    for (label, cutoff, releases) in cohorts {
        evidence.push(verify_release_cohort(label, *cutoff, releases, &migration).await);
    }

    let report = serde_json::json!({
        "schema_version": "1flowbase.console-policy-release-rehearsal/v1",
        "supported_release_count": 14,
        "schema_cohort_count": cohorts.len(),
        "catalog_fingerprint": migration.plan().catalog_fingerprint(),
        "mapping_fingerprint": migration.plan().mapping_fingerprint(),
        "cohorts": evidence,
        "ac_010": "green",
        "ac_011": "green",
    });
    let output_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("tmp/test-governance");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(
        output_dir.join("issue-1279-release-cohort-rehearsal.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    std::fs::write(
        output_dir.join("issue-1279-release-cohort-rehearsal.md"),
        "# Issue #1279 release cohort rehearsal\n\n- Released tags: 14\n- Schema cohorts: 5\n- Effective delta: empty for every role/actor probe\n- Rollback: verified to `legacy`\n- Finalize: verified to `console_policy`\n- Runtime: rejects `fenced`, accepts finalized marker\n",
    )
    .unwrap();
}

#[test]
fn ac_010_live_core_crosswalk_disposes_each_of_187_operations() {
    let settings = compile_core_settings_feature_registry()
        .expect("the Core settings feature registry must compile before migration planning");
    let registry = compile_core_console_operation_registry(&settings)
        .expect("the live Core console registry must compile before migration planning");

    let migration = compile_core_console_policy_migration_plan(registry.inventory())
        .expect("the audited Core crosswalk must compile against the live registry");

    assert_eq!(migration.dispositions().len(), 187);
    assert!(migration
        .dispositions()
        .iter()
        .all(|disposition| disposition.operation_id() != "system_all"));
    assert!(migration
        .disposition("roles.console_policy.replace")
        .is_some_and(|disposition| disposition.is_default_disabled_new_operation()));
    assert!(migration
        .disposition("data_sources.secret.rotate")
        .is_some_and(|disposition| disposition
            .has_legacy_grant("settings_feature.access.system.data-models")));
    assert!(registry.inventory().operations.iter().any(|operation| {
        operation.operation_id == "core.authenticated"
            && operation.authorization == ConsoleAuthorization::Authenticated
    }));
}

#[test]
fn ac_010_compiled_catalog_generates_every_actor_operation_and_row_probe() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let probes = compile_console_policy_migration_probes(migration.plan()).unwrap();
    let configurable_operation_count = migration
        .plan()
        .catalog()
        .groups
        .iter()
        .map(|group| group.full_operations.len())
        .sum::<usize>();

    assert!(probes.len() >= configurable_operation_count);
    for group in &migration.plan().catalog().groups {
        for operation in &group.full_operations {
            let operation_probes = probes
                .iter()
                .filter(|probe| probe.operation_id == *operation.operation_id())
                .collect::<Vec<_>>();
            match operation {
                domain::ConsoleOperationPolicy::Simple { .. } => {
                    assert_eq!(operation_probes.len(), 1);
                }
                domain::ConsoleOperationPolicy::Row { .. } => {
                    assert_eq!(operation_probes.len(), 3);
                }
            }
        }
    }
}

#[test]
fn ac_010_feature_to_other_regroup_preserves_data_source_secret_rotation() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();

    let preview = migration
        .plan()
        .project_legacy_role(
            Uuid::now_v7(),
            &["settings_feature.access.system.data-models".to_string()],
        )
        .expect("the audited feature-to-Other regroup must project without an authorization delta");

    assert!(preview.authorization_delta.added.is_empty());
    assert!(preview.authorization_delta.removed.is_empty());
    assert!(preview.effective_delta.is_empty());
    assert!(preview.effective_after.iter().any(|entry| {
        entry.operation_id.as_str() == "data_sources.secret.rotate"
            && entry.simple_enabled == Some(true)
    }));
}

#[test]
fn ac_010_new_role_console_policy_operations_are_default_disabled() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();

    let preview = migration
        .plan()
        .project_legacy_role(
            Uuid::now_v7(),
            &["settings_feature.access.system.roles".to_string()],
        )
        .expect("the historic roles feature grant must remain projectable");

    for operation_id in [
        "roles.console_policy_catalog.view",
        "roles.console_policy.view",
        "roles.console_policy.replace",
    ] {
        assert!(preview
            .effective_after
            .iter()
            .all(|entry| entry.operation_id.as_str() != operation_id));
    }
}

#[test]
fn ac_010_new_i18n_catalog_operations_are_default_disabled() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let operation_ids = [
        "i18n_catalog.bundle.get",
        "i18n_catalog.custom_keys.delete",
        "i18n_catalog.custom_translations.upsert",
        "i18n_catalog.entries.detail",
        "i18n_catalog.entries.list",
        "i18n_catalog.overrides.restore",
        "i18n_catalog.overrides.restore_all",
        "i18n_catalog.overrides.upsert",
        "i18n_catalog.state.get",
        "i18n_catalog.update.activate",
        "i18n_catalog.update.check",
    ];

    for operation_id in operation_ids {
        let disposition = migration
            .disposition(operation_id)
            .expect("every i18n catalog operation must have a migration disposition");
        assert_eq!(disposition.policy_group_id, "system.i18n-catalog");
        assert!(disposition.is_default_disabled_new_operation());
    }

    for mapping in migration.legacy_mappings() {
        if let ConsolePolicyMigrationLegacyGrantProjection::Operations(operations) =
            &mapping.projection
        {
            assert!(operations
                .iter()
                .all(|operation| !operation_ids.contains(&operation.operation_id().as_str())));
        }
    }
}

#[test]
fn ac_010_group_or_operation_mapping_drift_hard_stops() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let mut inventory = registry.inventory().clone();
    inventory
        .operations
        .iter_mut()
        .find(|operation| operation.operation_id == "data_sources.secret.rotate")
        .expect("the live inventory must contain the audited regrouped operation")
        .policy_group = ConsolePolicyGroup::SettingsFeature("system.data-models".to_string());

    let error = compile_core_console_policy_migration_plan(&inventory)
        .expect_err("an operation group drift must not silently migrate grants");

    assert!(error
        .to_string()
        .contains("Core migration policy-group mismatch for data_sources.secret.rotate"));
}

#[test]
fn ac_010_dispositions_and_mappings_never_offer_system_all() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let serialized = serde_json::to_string(migration.dispositions()).unwrap();

    assert!(!serialized.contains("system_all"));
    assert!(migration
        .legacy_mappings()
        .iter()
        .all(|mapping| !mapping.legacy_grant.contains("system_all")));
}

#[test]
fn ac_010_external_data_source_edit_implied_grants_have_audited_zero_projection() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();

    for legacy_grant in [
        "external_data_source.edit.own",
        "external_data_source.edit.all",
    ] {
        let mapping = migration
            .legacy_mappings()
            .iter()
            .find(|mapping| mapping.legacy_grant == legacy_grant)
            .expect("the old data-models SettingsRoute implied grant must be audited");
        let ConsolePolicyMigrationLegacyGrantProjection::NoProjection { evidence } =
            &mapping.projection
        else {
            panic!("{legacy_grant} must not expand into a live console operation");
        };
        assert!(evidence.contains("SettingsRoute implied this grant"));
        assert!(evidence.contains("would expand authority"));
    }
}

#[test]
fn ac_010_catalog_only_legacy_grants_do_not_expand_console_authority() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();

    for legacy_grant in [
        "embedded_app.use.own",
        "embedded_app.use.all",
        "plugin_config.edit.all",
    ] {
        let mapping = migration
            .legacy_mappings()
            .iter()
            .find(|mapping| mapping.legacy_grant == legacy_grant)
            .expect("a baseline catalog-only grant must have an audited disposition");
        let ConsolePolicyMigrationLegacyGrantProjection::NoProjection { evidence } =
            &mapping.projection
        else {
            panic!("{legacy_grant} must not expand into a live console operation");
        };
        assert!(evidence.contains("console"));

        let preview = migration
            .plan()
            .project_legacy_role(Uuid::now_v7(), &[legacy_grant.to_string()])
            .expect("an audited catalog-only grant must not block migration");
        assert!(preview.authorization_delta.added.is_empty());
        assert!(preview.authorization_delta.removed.is_empty());
        assert!(preview.effective_delta.is_empty());
        assert!(preview.effective_after.is_empty());
    }
}

#[test]
fn ac_010_active_host_operation_without_a_crosswalk_hard_stops() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let mut inventory = registry.inventory().clone();
    let operation = inventory
        .operations
        .iter_mut()
        .find(|operation| operation.operation_id == "workspace.update")
        .expect("the compiled Core inventory must contain workspace.update");
    operation.owner.kind = SettingsFeatureOwnerKind::HostExtension;
    operation.owner.owner_id = "fixture-host".to_string();
    operation.owner.version = "1.0.0".to_string();

    let error = compile_core_console_policy_migration_plan(&inventory)
        .expect_err("a linked HostExtension needs explicit migration metadata");

    assert!(error
        .to_string()
        .contains("active HostExtension fixture-host@1.0.0 contributes workspace.update"));
}

#[test]
fn ac_010_cli_commands_and_static_evidence_are_deterministic() {
    assert_eq!(
        parse_command("preview").unwrap(),
        ConsolePolicyMigrationCommand::Preview
    );
    assert_eq!(
        parse_command("apply").unwrap(),
        ConsolePolicyMigrationCommand::Apply
    );
    assert_eq!(
        parse_command("finalize").unwrap(),
        ConsolePolicyMigrationCommand::Finalize
    );
    assert_eq!(
        parse_command("rollback").unwrap(),
        ConsolePolicyMigrationCommand::Rollback
    );
    assert!(parse_command("delete").is_err());

    let settings = compile_core_settings_feature_registry().unwrap();
    let registry = compile_core_console_operation_registry(&settings).unwrap();
    let migration = compile_core_console_policy_migration_plan(registry.inventory()).unwrap();
    let first = ConsolePolicyMigrationEvidenceReport::for_compiled(
        "preview",
        "00000000-0000-0000-0000-000000000001",
        &migration,
    );
    let second = ConsolePolicyMigrationEvidenceReport::for_compiled(
        "preview",
        "00000000-0000-0000-0000-000000000001",
        &migration,
    );
    let serialized = serde_json::to_string_pretty(&first).unwrap();

    assert_eq!(serialized, serde_json::to_string_pretty(&second).unwrap());
    assert!(serialized.contains(&first.catalog_fingerprint));
    assert!(serialized.contains(&first.mapping_fingerprint));
    assert!(serialized.contains("data_sources.secret.rotate"));
    assert!(!serialized.contains("system_all"));
    assert!(first
        .markdown()
        .contains("The API runtime consumes the finalized cutover marker"));
    assert!(migration
        .source()
        .permission_resources
        .iter()
        .all(|resource| resource != "settings_route"));
    assert!(migration
        .legacy_mappings()
        .iter()
        .all(|mapping| !mapping.legacy_grant.starts_with("settings_route.visible.")));

    let paths = write_evidence_report(&first).unwrap();
    assert_eq!(std::fs::read_to_string(paths.json).unwrap(), serialized);
    assert!(std::fs::read_to_string(paths.markdown)
        .unwrap()
        .contains("Actor operation/row matrices"));
}
