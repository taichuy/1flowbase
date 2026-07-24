use control_plane::application::console_policy_migration::{
    applications_console_policy_catalog, applications_legacy_console_grant_mappings,
    applications_legacy_console_policy_source,
};
use control_plane::ports::{
    CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput,
    RoleConsolePolicyMigrationCutoverMarker, RoleConsolePolicyMigrationRehearsalInput,
    RoleConsolePolicyMigrationRepository, RoleConsolePolicyMigrationSource, RoleRepository,
};
use control_plane::role::console_policy_migration::{
    compile_console_policy_migration_plan_from_catalog,
    preview_console_policy_migration_actor_authorizations, project_legacy_role_console_policy,
    CompiledConsolePolicyCatalog, CompiledConsolePolicyGroup, ConsolePolicyMigrationActorProbeSet,
    ConsolePolicyMigrationActorRoleBinding, ConsolePolicyMigrationLegacyGrantMapping,
    ConsolePolicyMigrationLegacyGrantProjection, ConsolePolicyMigrationProbe,
    ConsolePolicyMigrationProbeKind,
};
use domain::{
    ConsoleOperationId, ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup,
    RoleConsoleGroupPolicy,
};
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

async fn role_store() -> (PgControlPlaneStore, Uuid, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
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

async fn simulate_legacy_cutover_state(store: &PgControlPlaneStore) {
    sqlx::query(
        r#"
        update role_console_policy_migration_cutover_state
        set marker = 'legacy',
            run_id = null,
            catalog_fingerprint = null,
            mapping_fingerprint = null,
            updated_at = now()
        where singleton
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
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
    let plan = compile_console_policy_migration_plan_from_catalog(
        applications_console_policy_catalog(),
        &applications_legacy_console_grant_mappings()
            .into_iter()
            .map(|mapping| ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: mapping.legacy_grant,
                projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(
                    mapping.operations,
                ),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let inventories = store
        .list_role_console_policy_migration_grants(&source)
        .await
        .unwrap();
    let previews = inventories
        .iter()
        .map(|inventory| {
            plan.project_legacy_role(inventory.role_id, &inventory.source_grants)
                .unwrap()
        })
        .collect();
    RoleConsolePolicyMigrationRehearsalInput {
        run_id: Uuid::now_v7(),
        source_contract: "applications-legacy/v1".into(),
        source,
        plan,
        previews,
        actor_previews: vec![],
    }
}

fn synthetic_migration_plan(
) -> control_plane::role::console_policy_migration::CompiledConsolePolicyMigrationPlan {
    compile_console_policy_migration_plan_from_catalog(
        CompiledConsolePolicyCatalog {
            complete: true,
            groups: vec![CompiledConsolePolicyGroup {
                group: ConsolePolicyGroup::other("migration").unwrap(),
                full_operations: vec![
                    ConsoleOperationPolicy::simple(
                        ConsoleOperationId::try_from("console.simple").unwrap(),
                        true,
                    ),
                    ConsoleOperationPolicy::simple(
                        ConsoleOperationId::try_from("console.create").unwrap(),
                        true,
                    ),
                    ConsoleOperationPolicy::row(
                        ConsoleOperationId::try_from("console.records.view").unwrap(),
                        ConsoleOperationRowScope::ScopeAll,
                    ),
                ],
            }],
        },
        &[
            ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: "legacy.simple".into(),
                projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![
                    ConsoleOperationPolicy::simple(
                        ConsoleOperationId::try_from("console.simple").unwrap(),
                        true,
                    ),
                ]),
            },
            ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: "legacy.create".into(),
                projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![
                    ConsoleOperationPolicy::simple(
                        ConsoleOperationId::try_from("console.create").unwrap(),
                        true,
                    ),
                ]),
            },
            ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: "legacy.records.view.own".into(),
                projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![
                    ConsoleOperationPolicy::row(
                        ConsoleOperationId::try_from("console.records.view").unwrap(),
                        ConsoleOperationRowScope::Own,
                    ),
                ]),
            },
            ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: "legacy.records.view.all".into(),
                projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![
                    ConsoleOperationPolicy::row(
                        ConsoleOperationId::try_from("console.records.view").unwrap(),
                        ConsoleOperationRowScope::ScopeAll,
                    ),
                ]),
            },
            ConsolePolicyMigrationLegacyGrantMapping {
                legacy_grant: "legacy.stale.non_console".into(),
                projection: ConsolePolicyMigrationLegacyGrantProjection::NoProjection {
                    evidence: "legacy permission is outside the console contract".into(),
                },
            },
        ],
    )
    .expect("synthetic complete catalog must compile")
}

async fn grant_legacy_migration_permissions(
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
        sqlx::query(
            r#"
            insert into permission_definitions (
              id, scope_id, resource, action, scope, code, name, introduction
            )
            values ($1, $2, 'migration', 'legacy', 'workspace', $3, $3, '')
            on conflict (code) do nothing
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
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

async fn bind_synthetic_migration_actor(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    role_ids: &[Uuid],
) {
    let account = format!("migration-{}", actor_user_id.simple());
    sqlx::query(
        r#"
        insert into users (id, account, email, password_hash, name, nickname, status)
        values ($1, $2, $3, 'hash', 'Migration actor', 'Migration actor', 'active')
        "#,
    )
    .bind(actor_user_id)
    .bind(&account)
    .bind(format!("{account}@example.test"))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into workspace_memberships (id, workspace_id, user_id, introduction) values ($1, $2, $3, '')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(actor_user_id)
    .execute(store.pool())
    .await
    .unwrap();
    for role_id in role_ids {
        sqlx::query(
            "insert into user_role_bindings (id, user_id, role_id, scope_id) values ($1, $2, $3, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(actor_user_id)
        .bind(role_id)
        .bind(workspace_id)
        .execute(store.pool())
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn ac_010_ac_011_migration_artifacts_bind_multi_role_probe_union_to_singleton_cutover_state()
{
    let (store, workspace_id, actor_user_id) = role_store().await;
    simulate_legacy_cutover_state(&store).await;
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
    grant_legacy_migration_permissions(
        &store,
        workspace_id,
        "operator",
        &[
            "legacy.simple",
            "legacy.records.view.own",
            "legacy.stale.non_console",
        ],
    )
    .await;
    grant_legacy_migration_permissions(
        &store,
        workspace_id,
        "viewer",
        &["legacy.create", "legacy.records.view.all"],
    )
    .await;

    let source = RoleConsolePolicyMigrationSource {
        permission_resources: vec!["migration".into()],
        exact_permission_codes: vec![],
    };
    let inventories = store
        .list_role_console_policy_migration_grants(&source)
        .await
        .unwrap();
    let plan = synthetic_migration_plan();
    let previews = inventories
        .iter()
        .map(|inventory| {
            plan.project_legacy_role(inventory.role_id, &inventory.source_grants)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let operator_role_id = inventories
        .iter()
        .find(|inventory| inventory.role_code == "operator")
        .unwrap()
        .role_id;
    let viewer_role_id = inventories
        .iter()
        .find(|inventory| inventory.role_code == "viewer")
        .unwrap()
        .role_id;
    let matrix_actor_id = Uuid::now_v7();
    bind_synthetic_migration_actor(
        &store,
        workspace_id,
        matrix_actor_id,
        &[operator_role_id, viewer_role_id],
    )
    .await;
    let actor_previews = preview_console_policy_migration_actor_authorizations(
        &plan,
        &[ConsolePolicyMigrationActorProbeSet {
            binding: ConsolePolicyMigrationActorRoleBinding {
                actor_user_id: matrix_actor_id,
                role_ids: vec![viewer_role_id, operator_role_id],
            },
            probes: vec![
                ConsolePolicyMigrationProbe {
                    operation_id: ConsoleOperationId::try_from("console.simple").unwrap(),
                    kind: ConsolePolicyMigrationProbeKind::Simple,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: ConsoleOperationId::try_from("console.create").unwrap(),
                    kind: ConsolePolicyMigrationProbeKind::Create,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: ConsoleOperationId::try_from("console.records.view").unwrap(),
                    kind: ConsolePolicyMigrationProbeKind::OwnRow,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: ConsoleOperationId::try_from("console.records.view").unwrap(),
                    kind: ConsolePolicyMigrationProbeKind::SameScopeOther,
                },
                ConsolePolicyMigrationProbe {
                    operation_id: ConsoleOperationId::try_from("console.records.view").unwrap(),
                    kind: ConsolePolicyMigrationProbeKind::CrossScope,
                },
            ],
        }],
        &previews,
    )
    .unwrap();
    let input = RoleConsolePolicyMigrationRehearsalInput {
        run_id: Uuid::now_v7(),
        source_contract: "synthetic-legacy/v1".into(),
        source,
        plan,
        previews,
        actor_previews,
    };

    store
        .rehearse_role_console_policy_migration(&input)
        .await
        .unwrap();
    let persisted_artifacts: (String, String, String) = sqlx::query_as(
        r#"
        select catalog_fingerprint, mapping_fingerprint, actor_role_bindings::text
        from role_console_policy_migration_run_artifacts
        where run_id = $1
        "#,
    )
    .bind(input.run_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(persisted_artifacts.0, input.plan.catalog_fingerprint());
    assert_eq!(persisted_artifacts.1, input.plan.mapping_fingerprint());
    assert!(persisted_artifacts.2.contains(&matrix_actor_id.to_string()));

    let mut mismatched = input.clone();
    mismatched.plan = compile_console_policy_migration_plan_from_catalog(
        input.plan.catalog().clone(),
        &[ConsolePolicyMigrationLegacyGrantMapping {
            legacy_grant: "legacy.simple".into(),
            projection: ConsolePolicyMigrationLegacyGrantProjection::Operations(vec![
                ConsoleOperationPolicy::simple(
                    ConsoleOperationId::try_from("console.simple").unwrap(),
                    true,
                ),
            ]),
        }],
    )
    .unwrap();
    assert!(store
        .apply_role_console_policy_migration(&mismatched, actor_user_id)
        .await
        .unwrap_err()
        .to_string()
        .contains("console_policy_migration_revision"));

    store
        .apply_role_console_policy_migration(&input, actor_user_id)
        .await
        .unwrap();
    let fenced = store
        .role_console_policy_migration_cutover_state()
        .await
        .unwrap();
    assert_eq!(
        fenced.marker,
        RoleConsolePolicyMigrationCutoverMarker::Fenced
    );
    assert_eq!(fenced.run_id, Some(input.run_id));
    assert_eq!(
        fenced.catalog_fingerprint.as_deref(),
        Some(input.plan.catalog_fingerprint())
    );
    assert_eq!(
        fenced.mapping_fingerprint.as_deref(),
        Some(input.plan.mapping_fingerprint())
    );

    store
        .rollback_role_console_policy_migration(input.run_id, actor_user_id)
        .await
        .unwrap();
    let legacy = store
        .role_console_policy_migration_cutover_state()
        .await
        .unwrap();
    assert_eq!(
        legacy.marker,
        RoleConsolePolicyMigrationCutoverMarker::Legacy
    );
    assert_eq!(legacy.run_id, None);
    assert!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from role_console_policy_migration_run_artifacts where run_id = $1",
        )
        .bind(input.run_id)
        .fetch_one(store.pool())
        .await
        .unwrap()
            == 1
    );

    let mut finalized_input = input.clone();
    finalized_input.run_id = Uuid::now_v7();
    store
        .rehearse_role_console_policy_migration(&finalized_input)
        .await
        .unwrap();
    store
        .apply_role_console_policy_migration(&finalized_input, actor_user_id)
        .await
        .unwrap();
    store
        .finalize_role_console_policy_migration(finalized_input.run_id, actor_user_id)
        .await
        .unwrap();
    let finalized = store
        .role_console_policy_migration_cutover_state()
        .await
        .unwrap();
    assert_eq!(
        finalized.marker,
        RoleConsolePolicyMigrationCutoverMarker::ConsolePolicy
    );
    assert_eq!(finalized.run_id, Some(finalized_input.run_id));
    assert_eq!(
        finalized.catalog_fingerprint.as_deref(),
        Some(finalized_input.plan.catalog_fingerprint())
    );
    assert_eq!(
        finalized.mapping_fingerprint.as_deref(),
        Some(finalized_input.plan.mapping_fingerprint())
    );
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
    simulate_legacy_cutover_state(&store).await;
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
    simulate_legacy_cutover_state(&store).await;
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
