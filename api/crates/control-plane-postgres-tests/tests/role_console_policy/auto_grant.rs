use access_control::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory, ConsoleOperationInventoryEntry,
    ConsoleOperationOwner, ConsolePolicyGroup as RegisteredGroup, ConsoleRouteBinding,
    SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
};
use control_plane::{
    ports::{CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput, RoleRepository},
    role::sync_console_permission_catalog,
};
use domain::{
    effective_console_simple_operation, ConsoleOperationId, ConsoleOperationPolicy,
    ConsolePolicyGroup, RoleConsoleGroupPolicy, RoleConsolePolicy,
};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn group(id: &str) -> ConsolePolicyGroup {
    ConsolePolicyGroup::other(id).unwrap()
}
fn operation(id: &str, enabled: bool) -> ConsoleOperationPolicy {
    ConsoleOperationPolicy::simple(ConsoleOperationId::try_from(id).unwrap(), enabled)
}
fn inventory(entries: &[(&str, &str)]) -> ConsoleOperationCompiledInventory {
    ConsoleOperationCompiledInventory {
        schema_version: "test",
        interfaces: vec![],
        resources: vec![],
        locale_catalog: None,
        operations: entries
            .iter()
            .enumerate()
            .map(|(index, (group, id))| ConsoleOperationInventoryEntry {
                operation_id: id.to_string(),
                authorization_profile_id: id.to_string(),
                owner: ConsoleOperationOwner {
                    kind: SettingsFeatureOwnerKind::Core,
                    owner_id: "test".into(),
                    version: "1".into(),
                },
                lifecycle: SettingsFeatureLifecycle::Active,
                policy_group: RegisteredGroup::Other(group.to_string()),
                order: index as i32,
                routes: vec![ConsoleRouteBinding {
                    method: "GET".into(),
                    path: format!("/api/console/test/{id}"),
                }],
                authorization: ConsoleAuthorization::Simple,
            })
            .collect(),
    }
}
async fn fixture() -> (PgControlPlaneStore, Uuid) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let pool = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap()
        .connect()
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "auto-grant")
        .await
        .unwrap();
    for (code, enabled) in [("receiver", true), ("manual", false)] {
        RoleRepository::create_team_role(
            &store,
            &CreateWorkspaceRoleInput {
                actor_user_id: Uuid::now_v7(),
                workspace_id: workspace.id,
                code: code.into(),
                name: code.into(),
                introduction: String::new(),
                auto_grant_new_permissions: enabled,
                is_default_member_role: false,
            },
        )
        .await
        .unwrap();
        save(
            &store,
            workspace.id,
            code,
            vec![
                RoleConsoleGroupPolicy::custom(
                    group("other.existing"),
                    vec![operation("existing.denied", false)],
                ),
                RoleConsoleGroupPolicy::disabled(group("other.closed")),
                RoleConsoleGroupPolicy::full(group("other.full")),
            ],
        )
        .await;
    }
    (store, workspace.id)
}
async fn save(
    store: &PgControlPlaneStore,
    workspace: Uuid,
    code: &str,
    groups: Vec<RoleConsoleGroupPolicy>,
) {
    RoleRepository::replace_role_console_policy(
        store,
        &ReplaceRoleConsolePolicyInput {
            actor_user_id: Uuid::now_v7(),
            workspace_id: workspace,
            role_code: code.into(),
            groups,
        },
    )
    .await
    .unwrap();
}
async fn policy(store: &PgControlPlaneStore, workspace: Uuid, code: &str) -> RoleConsolePolicy {
    RoleRepository::get_role_console_policy(store, workspace, code)
        .await
        .unwrap()
}
fn allowed(policy: &RoleConsolePolicy, group_id: &str, id: &str) -> bool {
    effective_console_simple_operation(
        &[policy.clone()],
        &group(group_id),
        &ConsoleOperationId::try_from(id).unwrap(),
    )
}
const BASE: &[(&str, &str)] = &[
    ("other.existing", "existing.old"),
    ("other.closed", "closed.old"),
    ("other.absent", "absent.old"),
    ("other.full", "full.old"),
];

#[tokio::test]
async fn ac_001_002_003_only_new_permissions_reach_eligible_custom_and_new_groups() {
    let (store, workspace) = fixture().await;
    let before = policy(&store, workspace, "receiver").await;
    sync_console_permission_catalog(&store, &inventory(BASE))
        .await
        .unwrap();
    assert_eq!(
        policy(&store, workspace, "receiver").await,
        before,
        "initial baseline must not backfill"
    );
    let mut next = BASE.to_vec();
    next.extend_from_slice(&[
        ("other.existing", "existing.new"),
        ("other.existing", "existing.denied"),
        ("other.closed", "closed.new"),
        ("other.absent", "absent.new"),
        ("other.new", "new.read"),
        ("other.full", "full.new"),
    ]);
    sync_console_permission_catalog(&store, &inventory(&next))
        .await
        .unwrap();
    let receiver = policy(&store, workspace, "receiver").await;
    assert!(allowed(&receiver, "other.existing", "existing.new"));
    assert!(allowed(&receiver, "other.new", "new.read"));
    assert!(allowed(&receiver, "other.full", "full.new"));
    for (g, id) in [
        ("other.existing", "existing.old"),
        ("other.existing", "existing.denied"),
        ("other.closed", "closed.new"),
        ("other.absent", "absent.new"),
    ] {
        assert!(!allowed(&receiver, g, id), "must preserve {id}");
    }
    assert_eq!(
        receiver
            .groups()
            .iter()
            .find(|g| g.group() == &group("other.new"))
            .unwrap()
            .strategy(),
        domain::ConsolePolicyStrategy::Custom
    );
    assert!(!allowed(
        &policy(&store, workspace, "manual").await,
        "other.existing",
        "existing.new"
    ));
    save(&store, workspace, "receiver", before.groups().to_vec()).await;
    sync_console_permission_catalog(&store, &inventory(BASE))
        .await
        .unwrap();
    sync_console_permission_catalog(&store, &inventory(&next))
        .await
        .unwrap();
    assert_eq!(
        policy(&store, workspace, "receiver").await,
        before,
        "restart and reappearance must preserve revocation"
    );
}

#[tokio::test]
async fn ac_004_disabled_interval_is_not_backfilled_after_reenable() {
    let (store, workspace) = fixture().await;
    sync_console_permission_catalog(&store, &inventory(BASE))
        .await
        .unwrap();
    let mut next = BASE.to_vec();
    next.push(("other.existing", "existing.missed"));
    sync_console_permission_catalog(&store, &inventory(&next))
        .await
        .unwrap();
    sqlx::query("update roles set auto_grant_new_permissions = true where workspace_id = $1 and code = 'manual'").bind(workspace).execute(store.pool()).await.unwrap();
    next.push(("other.existing", "existing.future"));
    sync_console_permission_catalog(&store, &inventory(&next))
        .await
        .unwrap();
    let manual = policy(&store, workspace, "manual").await;
    assert!(!allowed(&manual, "other.existing", "existing.missed"));
    assert!(allowed(&manual, "other.existing", "existing.future"));
}

#[tokio::test]
async fn ac_005_concurrent_catalog_sync_is_idempotent() {
    let (store, workspace) = fixture().await;
    sync_console_permission_catalog(&store, &inventory(BASE))
        .await
        .unwrap();
    let mut next = BASE.to_vec();
    next.push(("other.existing", "existing.concurrent"));
    let catalog = inventory(&next);
    let (a, b) = tokio::join!(
        sync_console_permission_catalog(&store, &catalog),
        sync_console_permission_catalog(&store, &catalog)
    );
    a.unwrap();
    b.unwrap();
    let receiver = policy(&store, workspace, "receiver").await;
    assert!(allowed(&receiver, "other.existing", "existing.concurrent"));
    let count: i64 = sqlx::query_scalar("select count(*) from role_console_operation_policies where role_id = $1 and operation_id = 'existing.concurrent'").bind(receiver.role_id()).fetch_one(store.pool()).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn ac_005_failed_grant_rolls_back_first_seen_ledger_and_can_retry() {
    let (store, workspace) = fixture().await;
    sync_console_permission_catalog(&store, &inventory(BASE))
        .await
        .unwrap();
    let before = policy(&store, workspace, "receiver").await;
    sqlx::raw_sql("create function reject_auto_grant() returns trigger language plpgsql as $$ begin if new.operation_id = 'existing.retry' then raise exception 'controlled grant failure'; end if; return new; end $$; create trigger reject_auto_grant before insert on role_console_operation_policies for each row execute function reject_auto_grant();").execute(store.pool()).await.unwrap();
    let mut next = BASE.to_vec();
    next.push(("other.existing", "existing.retry"));
    assert!(sync_console_permission_catalog(&store, &inventory(&next))
        .await
        .is_err());
    assert_eq!(policy(&store, workspace, "receiver").await, before);
    let seen: i64 = sqlx::query_scalar("select count(*) from console_permission_catalog_operations where operation_id = 'existing.retry'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(seen, 0, "failed grants cannot consume the first-seen event");
    sqlx::raw_sql("drop trigger reject_auto_grant on role_console_operation_policies; drop function reject_auto_grant();").execute(store.pool()).await.unwrap();
    sync_console_permission_catalog(&store, &inventory(&next))
        .await
        .unwrap();
    assert!(allowed(
        &policy(&store, workspace, "receiver").await,
        "other.existing",
        "existing.retry"
    ));
}

#[tokio::test]
async fn ac_002_row_grants_use_compiled_scope_and_preserve_explicit_own_scope() {
    use access_control::{
        ResourceAccessAction, ResourceAccessRegistration, ResourceAccessScopeKind,
    };
    use domain::{effective_console_row_scope, ConsoleOperationRowScope};
    let (store, workspace) = fixture().await;
    sync_console_permission_catalog(&store, &inventory(BASE))
        .await
        .unwrap();
    sqlx::query("update roles set auto_grant_new_permissions = true where workspace_id = $1")
        .bind(workspace)
        .execute(store.pool())
        .await
        .unwrap();
    save(
        &store,
        workspace,
        "receiver",
        vec![RoleConsoleGroupPolicy::custom(
            group("other.existing"),
            vec![ConsoleOperationPolicy::row(
                ConsoleOperationId::try_from("records.read").unwrap(),
                ConsoleOperationRowScope::Own,
            )],
        )],
    )
    .await;
    let mut entries = BASE.to_vec();
    entries.push(("other.existing", "records.read"));
    let mut catalog = inventory(&entries);
    let row = catalog.operations.last_mut().unwrap();
    row.authorization = ConsoleAuthorization::ResourceAction {
        resource_code: "record".into(),
        action_code: "read".into(),
    };
    catalog.resources.push(ResourceAccessRegistration {
        resource_code: "record".into(),
        owner: row.owner.clone(),
        lifecycle: SettingsFeatureLifecycle::Active,
        scope_kind: ResourceAccessScopeKind::Workspace,
        identity_field: "id".into(),
        scope_field: Some("scope_id".into()),
        owner_field: Some("created_by".into()),
        label_ref: "record".into(),
        description_ref: None,
        actions: vec![ResourceAccessAction {
            action_code: "read".into(),
            label_ref: "read".into(),
            description_ref: None,
        }],
    });
    sync_console_permission_catalog(&store, &catalog)
        .await
        .unwrap();
    for (code, expected) in [
        ("receiver", ConsoleOperationRowScope::Own),
        ("manual", ConsoleOperationRowScope::ScopeAll),
    ] {
        assert_eq!(
            effective_console_row_scope(
                &[policy(&store, workspace, code).await],
                &group("other.existing"),
                &ConsoleOperationId::try_from("records.read").unwrap()
            ),
            expected
        );
    }
}
