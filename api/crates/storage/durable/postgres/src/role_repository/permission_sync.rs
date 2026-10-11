use std::collections::BTreeSet;

use anyhow::{ensure, Result};
use async_trait::async_trait;
use control_plane_contracts::{
    console_permission_sync::project_console_permission_additions,
    ports::ConsolePermissionCatalogRepository, CompiledConsolePolicyCatalog,
    CompiledConsolePolicyGroup,
};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::role_console_policy_by_id;
use crate::repositories::PgControlPlaneStore;

/// Seed a newly created Admin from the published catalog. The publication lock is held by
/// the caller; existing grants are never replaced and full groups need no operation rows.
pub(crate) async fn seed_admin_console_policy(
    tx: &mut Transaction<'_, Postgres>,
    role_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"insert into role_console_group_policies
        (id, role_id, group_kind, group_id, enabled, strategy)
        select gen_random_uuid(), $1, group_kind, group_id, true, 'full'
        from console_permission_catalog_groups
        on conflict (role_id, group_kind, group_id) do nothing"#,
    )
    .bind(role_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[async_trait]
impl ConsolePermissionCatalogRepository for PgControlPlaneStore {
    async fn sync_console_permission_catalog(
        &self,
        catalog: &CompiledConsolePolicyCatalog,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sync_console_permission_catalog_in_transaction(&mut tx, catalog).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Prepare catalog additions and opt-in grants in the caller's publication transaction.
pub(crate) async fn sync_console_permission_catalog_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    catalog: &CompiledConsolePolicyCatalog,
) -> Result<()> {
    ensure!(
        catalog.complete && !catalog.groups.is_empty(),
        "console permission synchronization requires a complete catalog"
    );
    let mut group_ids = BTreeSet::new();
    let mut operation_ids = BTreeSet::new();
    for group in &catalog.groups {
        ensure!(
            group_ids.insert(&group.group),
            "duplicate console permission group"
        );
        for operation in &group.full_operations {
            ensure!(
                operation_ids.insert(operation.operation_id()),
                "duplicate console permission operation"
            );
        }
    }
    // One publication at a time across all nodes. The ledger and grants commit together.
    let initialized: bool = sqlx::query_scalar(
        "select initialized from console_permission_catalog_sync where singleton = true for update",
    )
    .fetch_one(&mut **tx)
    .await?;
    let inserted_groups: BTreeSet<(String, String)> = sqlx::query_as(
        "insert into console_permission_catalog_groups (group_kind, group_id) select * from unnest($1::text[], $2::text[]) on conflict do nothing returning group_kind, group_id"
    ).bind(catalog.groups.iter().map(|g| g.group.kind().as_str()).collect::<Vec<_>>())
        .bind(catalog.groups.iter().map(|g| g.group.group_id().as_str()).collect::<Vec<_>>())
        .fetch_all(&mut **tx).await?.into_iter().collect();
    let inserted_operations: BTreeSet<String> = sqlx::query_scalar(
        "insert into console_permission_catalog_operations (operation_id) select unnest($1::text[]) on conflict do nothing returning operation_id"
    ).bind(operation_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>())
        .fetch_all(&mut **tx).await?.into_iter().collect();
    let mut new_groups = BTreeSet::new();
    let mut additions = Vec::new();
    for group in &catalog.groups {
        if inserted_groups.contains(&(
            group.group.kind().as_str().to_owned(),
            group.group.group_id().as_str().to_owned(),
        )) {
            new_groups.insert(group.group.clone());
        }
        let full_operations = group
            .full_operations
            .iter()
            .filter(|operation| inserted_operations.contains(operation.operation_id().as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !full_operations.is_empty() {
            additions.push(CompiledConsolePolicyGroup {
                group: group.group.clone(),
                full_operations,
            });
        }
    }
    let mut granted_operations = 0;
    if initialized && !additions.is_empty() {
        // Role edits use these same tables. Serialize the short publication transaction
        // with opt-in changes and policy replacement so a manual revoke cannot be lost.
        sqlx::query("lock table roles, role_console_group_policies, role_console_operation_policies in share row exclusive mode")
            .execute(&mut **tx).await?;
        let role_ids: Vec<Uuid> = sqlx::query_scalar(
            "select id from roles where scope_kind = 'workspace' and auto_grant_new_permissions = true order by id"
        ).fetch_all(&mut **tx).await?;
        for role_id in role_ids {
            let policy = role_console_policy_by_id(&mut **tx, role_id).await?;
            for group in project_console_permission_additions(&policy, &additions, &new_groups) {
                let group_policy_id: Uuid = sqlx::query_scalar(
                    r#"insert into role_console_group_policies
                    (id, role_id, group_kind, group_id, enabled, strategy)
                    values ($1, $2, $3, $4, true, 'custom')
                    on conflict (role_id, group_kind, group_id) do update
                    set group_id = excluded.group_id returning id"#,
                )
                .bind(Uuid::now_v7())
                .bind(role_id)
                .bind(group.group().kind().as_str())
                .bind(group.group().group_id().as_str())
                .fetch_one(&mut **tx)
                .await?;
                for operation in group.operations() {
                    granted_operations += sqlx::query(
                        r#"insert into role_console_operation_policies
                        (id, role_id, group_policy_id, operation_id, policy_kind, simple_enabled, row_scope)
                        values ($1, $2, $3, $4, $5, $6, $7)
                        on conflict (role_id, operation_id) do nothing"#
                    ).bind(Uuid::now_v7()).bind(role_id).bind(group_policy_id)
                        .bind(operation.operation_id().as_str()).bind(operation.policy_kind())
                        .bind(operation.simple_enabled()).bind(operation.row_scope().map(|scope| scope.as_str()))
                        .execute(&mut **tx).await?.rows_affected();
                }
            }
        }
    }
    if !initialized {
        // Bootstrap runs before the first catalog publication. Only initialize pristine
        // default Admin roles here; subsequent publications never backfill the baseline.
        sqlx::query("lock table roles, role_console_group_policies, role_console_operation_policies in share row exclusive mode")
            .execute(&mut **tx).await?;
        let admin_ids: Vec<Uuid> = sqlx::query_scalar(
            "select id from roles where scope_kind = 'workspace' and code = 'admin' and auto_grant_new_permissions = true and not exists (select 1 from role_console_group_policies where role_id = roles.id) order by id"
        ).fetch_all(&mut **tx).await?;
        for role_id in admin_ids {
            seed_admin_console_policy(tx, role_id).await?;
        }
        sqlx::query(
            "update console_permission_catalog_sync set initialized = true where singleton = true",
        )
        .execute(&mut **tx)
        .await?;
    }
    if !additions.is_empty() || !initialized {
        tracing::debug!(
            baseline = !initialized,
            new_operations = additions
                .iter()
                .map(|g| g.full_operations.len())
                .sum::<usize>(),
            granted_operations,
            "console permission catalog synchronization prepared"
        );
    }
    Ok(())
}
