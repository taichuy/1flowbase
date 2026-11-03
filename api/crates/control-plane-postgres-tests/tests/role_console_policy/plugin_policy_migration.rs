use super::*;
use control_plane::ports::ConsolePermissionCatalogRepository;
use domain::{effective_console_simple_operation, ConsolePolicyStrategy};
use sqlx::migrate::Migrator;
use std::borrow::Cow;

const PLUGIN_GROUP_MIGRATION: i64 = 20261103100000;

async fn old_store() -> (PgControlPlaneStore, Uuid, Uuid) {
    let pool = isolated_database().await.connect().await.unwrap();
    let migrations = sqlx::migrate!("../storage/durable/postgres/migrations")
        .iter()
        .filter(|migration| migration.version < PLUGIN_GROUP_MIGRATION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "plugin-policy-migration")
        .await
        .unwrap();
    let owner = Uuid::now_v7();
    sqlx::query("insert into users (id,account,email,password_hash,name,nickname,status) values ($1,$2,$3,'!fixture','','','active')")
        .bind(owner).bind(format!("plugin-{owner}")).bind(format!("{owner}@example.com"))
        .execute(store.pool()).await.unwrap();
    (store, workspace.id, owner)
}

async fn installation(store: &PgControlPlaneStore, owner: Uuid, artifact: &str, feature: &str) {
    sqlx::query("insert into extension_installations (id,category,organization,artifact_id,artifact_version,plugin_id,contract_version,protocol,display_name,source_kind,trust_level,verification_status,desired_state,signature_status,metadata_json,created_by) values ($1,'runtime-extensions','fixture',$2,'1.0.0',$2,'1flowbase.extension-bus/v1','stdio_json_multiplex_v1','Fixture','uploaded','unverified','valid','disabled','missing',$3,$4)")
        .bind(Uuid::now_v7()).bind(artifact)
        .bind(serde_json::json!({"managed_service":{"feature":{"feature_id":feature}}}))
        .bind(owner).execute(store.pool()).await.unwrap();
}

async fn role(
    store: &PgControlPlaneStore,
    workspace: Uuid,
    owner: Uuid,
    code: &str,
    policy: RoleConsoleGroupPolicy,
) -> Uuid {
    RoleRepository::create_team_role(
        store,
        &CreateWorkspaceRoleInput {
            actor_user_id: owner,
            workspace_id: workspace,
            code: code.into(),
            name: code.into(),
            introduction: String::new(),
            auto_grant_new_permissions: true,
            is_default_member_role: false,
        },
    )
    .await
    .unwrap();
    let saved = RoleRepository::replace_role_console_policy(
        store,
        &ReplaceRoleConsolePolicyInput {
            actor_user_id: owner,
            workspace_id: workspace,
            role_code: code.into(),
            groups: vec![policy],
        },
    )
    .await
    .unwrap();
    saved.role_id()
}

#[tokio::test]
async fn inactive_plugin_migration_preserves_policy_ids_denials_and_first_seen_state() {
    let (store, workspace, owner) = old_store().await;
    installation(&store, owner, "acme.calendar", "acme.calendar.settings").await;
    let legacy = ConsolePolicyGroup::settings_feature("acme.calendar.settings").unwrap();
    let plugin = ConsolePolicyGroup::plugin("acme.calendar").unwrap();
    let allowed = ConsoleOperationId::try_from("acme.calendar.list").unwrap();
    let denied = ConsoleOperationId::try_from("acme.calendar.delete").unwrap();
    let custom = RoleConsoleGroupPolicy::custom(
        legacy.clone(),
        vec![
            ConsoleOperationPolicy::simple(allowed.clone(), true),
            ConsoleOperationPolicy::simple(denied.clone(), false),
        ],
    );
    let full_id = role(
        &store,
        workspace,
        owner,
        "full",
        RoleConsoleGroupPolicy::full(legacy),
    )
    .await;
    let custom_id = role(&store, workspace, owner, "custom", custom.clone()).await;
    let disabled_id = role(
        &store,
        workspace,
        owner,
        "disabled",
        custom.with_enabled(false),
    )
    .await;
    let before: Vec<(Uuid, Uuid, bool, String)> = sqlx::query_as(
        "select id,role_id,enabled,strategy from role_console_group_policies order by id",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    let operation_ids: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "select id,group_policy_id from role_console_operation_policies order by id",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    sqlx::query("insert into console_permission_catalog_groups (group_kind,group_id,first_seen_at) values ('settings_feature','acme.calendar.settings','2026-01-01T00:00:00Z')")
        .execute(store.pool()).await.unwrap();
    sqlx::query("insert into console_permission_catalog_operations (operation_id) values ('acme.calendar.list'),('acme.calendar.delete')")
        .execute(store.pool()).await.unwrap();
    sqlx::query("update console_permission_catalog_sync set initialized=true")
        .execute(store.pool())
        .await
        .unwrap();
    run_migrations(store.pool()).await.unwrap();
    let after: Vec<(Uuid, Uuid, bool, String)> = sqlx::query_as(
        "select id,role_id,enabled,strategy from role_console_group_policies order by id",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(before, after);
    let after_operations: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "select id,group_policy_id from role_console_operation_policies order by id",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(operation_ids, after_operations);
    let markers: Vec<(String, String, String)> = sqlx::query_as(
        "select group_kind,group_id,first_seen_at::text from console_permission_catalog_groups",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(markers.len(), 1);
    assert_eq!(
        (&markers[0].0, &markers[0].1),
        (&"plugin".to_string(), &"acme.calendar".to_string())
    );
    assert!(markers[0].2.starts_with("2026-01-01"));
    // Publishing the renamed group cannot refill custom denials or enable disabled policy.
    let catalog = CompiledConsolePolicyCatalog {
        complete: true,
        groups: vec![CompiledConsolePolicyGroup {
            group: plugin.clone(),
            full_operations: vec![
                ConsoleOperationPolicy::simple(allowed.clone(), true),
                ConsoleOperationPolicy::simple(denied.clone(), true),
            ],
        }],
    };
    ConsolePermissionCatalogRepository::sync_console_permission_catalog(&store, &catalog)
        .await
        .unwrap();
    for (code, id, grants) in [
        ("full", full_id, (true, true)),
        ("custom", custom_id, (true, false)),
        ("disabled", disabled_id, (false, false)),
    ] {
        let policy = RoleRepository::get_role_console_policy(&store, workspace, code)
            .await
            .unwrap();
        assert_eq!(policy.role_id(), id);
        assert_eq!(policy.groups().len(), 1);
        assert_eq!(
            effective_console_simple_operation(&[policy.clone()], &plugin, &allowed),
            grants.0
        );
        assert_eq!(
            effective_console_simple_operation(&[policy.clone()], &plugin, &denied),
            grants.1
        );
        if code == "disabled" {
            assert_eq!(policy.groups()[0].strategy(), ConsolePolicyStrategy::Custom);
            assert_eq!(policy.groups()[0].operations().len(), 2);
        }
    }
}

#[tokio::test]
async fn ambiguous_plugin_identity_aborts_migration_without_moving_policy() {
    let (store, workspace, owner) = old_store().await;
    installation(&store, owner, "acme.calendar", "shared.settings").await;
    installation(&store, owner, "other.calendar", "shared.settings").await;
    role(
        &store,
        workspace,
        owner,
        "operator",
        RoleConsoleGroupPolicy::disabled(
            ConsolePolicyGroup::settings_feature("shared.settings").unwrap(),
        ),
    )
    .await;
    let error = run_migrations(store.pool()).await.unwrap_err();
    assert!(format!("{error:#}").contains("ambiguous or invalid historical plugin policy identity"));
    let policy = RoleRepository::get_role_console_policy(&store, workspace, "operator")
        .await
        .unwrap();
    assert_eq!(
        policy.groups()[0].group().kind().as_str(),
        "settings_feature"
    );
    assert!(!policy.groups()[0].enabled());
}

#[tokio::test]
async fn existing_canonical_ledger_conflict_aborts_without_losing_legacy_grants() {
    let (store, workspace, owner) = old_store().await;
    installation(&store, owner, "acme.calendar", "acme.calendar.settings").await;
    role(
        &store,
        workspace,
        owner,
        "operator",
        RoleConsoleGroupPolicy::full(
            ConsolePolicyGroup::settings_feature("acme.calendar.settings").unwrap(),
        ),
    )
    .await;
    // Represents an already present canonical record during an inconsistent deployment.
    sqlx::query("alter table console_permission_catalog_groups drop constraint console_permission_catalog_groups_group_kind_check")
        .execute(store.pool()).await.unwrap();
    sqlx::query("alter table console_permission_catalog_groups add constraint console_permission_catalog_groups_group_kind_check check (group_kind in ('settings_feature','other','plugin'))")
        .execute(store.pool()).await.unwrap();
    sqlx::query("insert into console_permission_catalog_groups (group_kind,group_id) values ('settings_feature','acme.calendar.settings'),('plugin','acme.calendar')")
        .execute(store.pool()).await.unwrap();
    let error = run_migrations(store.pool()).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("conflicting historical and canonical plugin policy groups")
    );
    let count: i64 = sqlx::query_scalar("select count(*) from console_permission_catalog_groups")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 2);
    let policy = RoleRepository::get_role_console_policy(&store, workspace, "operator")
        .await
        .unwrap();
    assert!(policy.groups()[0].enabled());
    assert_eq!(
        policy.groups()[0].group().kind().as_str(),
        "settings_feature"
    );
}

#[tokio::test]
async fn canonical_role_conflict_aborts_without_merging_enabled_and_disabled_groups() {
    let (store, workspace, owner) = old_store().await;
    installation(&store, owner, "acme.calendar", "acme.calendar.settings").await;
    let role_id = role(
        &store,
        workspace,
        owner,
        "operator",
        RoleConsoleGroupPolicy::full(
            ConsolePolicyGroup::settings_feature("acme.calendar.settings").unwrap(),
        ),
    )
    .await;
    sqlx::query("alter table role_console_group_policies drop constraint role_console_group_policies_group_kind_check")
        .execute(store.pool()).await.unwrap();
    sqlx::query("alter table role_console_group_policies add constraint role_console_group_policies_group_kind_check check (group_kind in ('settings_feature','other','plugin'))")
        .execute(store.pool()).await.unwrap();
    sqlx::query("insert into role_console_group_policies (id,role_id,group_kind,group_id,enabled,strategy) values ($1,$2,'plugin','acme.calendar',false,'custom')")
        .bind(Uuid::now_v7()).bind(role_id).execute(store.pool()).await.unwrap();
    let error = run_migrations(store.pool()).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("conflicting historical and canonical plugin policy groups")
    );
    let rows: Vec<(String,bool)> = sqlx::query_as(
        "select group_kind,enabled from role_console_group_policies where role_id=$1 order by group_kind")
        .bind(role_id).fetch_all(store.pool()).await.unwrap();
    assert_eq!(
        rows,
        vec![("plugin".into(), false), ("settings_feature".into(), true)]
    );
}

#[tokio::test]
async fn fully_uninstalled_plugin_without_metadata_is_retained_without_guessing_identity() {
    let (store, workspace, owner) = old_store().await;
    role(
        &store,
        workspace,
        owner,
        "operator",
        RoleConsoleGroupPolicy::disabled(
            ConsolePolicyGroup::settings_feature("deleted.plugin.settings").unwrap(),
        ),
    )
    .await;
    run_migrations(store.pool()).await.unwrap();
    let policy = RoleRepository::get_role_console_policy(&store, workspace, "operator")
        .await
        .unwrap();
    assert_eq!(policy.groups().len(), 1);
    assert_eq!(
        policy.groups()[0].group().kind().as_str(),
        "settings_feature"
    );
    assert_eq!(
        policy.groups()[0].group().group_id().as_str(),
        "deleted.plugin.settings"
    );
    assert!(!policy.groups()[0].enabled());
}
