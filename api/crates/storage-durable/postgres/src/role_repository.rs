use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput,
        ReplaceRoleDataPolicyInput, RoleConsolePolicyMigrationGrantInventory,
        RoleConsolePolicyMigrationRehearsalInput, RoleConsolePolicyMigrationRepository,
        RoleConsolePolicyMigrationSource, RoleDataPolicyDefaultsInput, RoleRepository,
        UpdateWorkspaceRoleInput,
    },
};
use domain::{ActorContext, AuditLogRecord, RoleScopeKind};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    mappers::role_mapper::PgRoleMapper,
    repositories::{
        find_role_by_code, permission_codes_for_role, stored_role_from_row,
        tenant_id_for_workspace, workspace_id_for_user, PgControlPlaneStore,
    },
};

fn data_policy_scope_from_db(value: String) -> domain::RoleDataPolicyScope {
    domain::RoleDataPolicyScope::from_db(&value)
}

fn optional_data_policy_scope_from_db(
    value: Option<String>,
) -> Option<domain::RoleDataPolicyScope> {
    value.map(|scope| domain::RoleDataPolicyScope::from_db(&scope))
}

fn default_role_data_policy() -> RoleDataPolicyDefaultsInput {
    RoleDataPolicyDefaultsInput {
        can_view: false,
        can_create: false,
        can_update: false,
        can_delete: false,
        default_view_scope: domain::RoleDataPolicyScope::Own,
        default_update_scope: domain::RoleDataPolicyScope::Own,
        default_delete_scope: domain::RoleDataPolicyScope::Own,
    }
}

async fn insert_default_role_data_policy(
    tx: &mut Transaction<'_, Postgres>,
    role_id: Uuid,
) -> Result<()> {
    let policy = default_role_data_policy();
    sqlx::query(
        r#"
        insert into role_data_policies (
            id, role_id, can_view, can_create, can_update, can_delete,
            default_view_scope, default_update_scope, default_delete_scope
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        on conflict (role_id) do nothing
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(role_id)
    .bind(policy.can_view)
    .bind(policy.can_create)
    .bind(policy.can_update)
    .bind(policy.can_delete)
    .bind(policy.default_view_scope.as_str())
    .bind(policy.default_update_scope.as_str())
    .bind(policy.default_delete_scope.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn role_console_policy_by_id(
    pool: &sqlx::PgPool,
    role_id: Uuid,
) -> Result<domain::RoleConsolePolicy> {
    let rows = sqlx::query(
        r#"
        select
          group_policy.id as group_policy_id,
          group_policy.group_kind,
          group_policy.group_id,
          group_policy.mode,
          operation_policy.operation_id,
          operation_policy.policy_kind,
          operation_policy.simple_enabled,
          operation_policy.row_scope
        from role_console_group_policies group_policy
        left join role_console_operation_policies operation_policy
          on operation_policy.group_policy_id = group_policy.id
         and operation_policy.role_id = group_policy.role_id
        where group_policy.role_id = $1
        order by group_policy.group_kind, group_policy.group_id, operation_policy.operation_id
        "#,
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    let mut stored_groups = BTreeMap::<
        Uuid,
        (
            domain::ConsolePolicyGroup,
            domain::ConsolePolicyMode,
            Vec<domain::ConsoleOperationPolicy>,
        ),
    >::new();
    for row in rows {
        let group_policy_id: Uuid = row.get("group_policy_id");
        let group_kind_value: String = row.get("group_kind");
        let group_kind = domain::ConsolePolicyGroupKind::parse(&group_kind_value)
            .ok_or_else(|| anyhow!("stored console policy group kind is invalid"))?;
        let group_id: String = row.get("group_id");
        let group = domain::ConsolePolicyGroup::new(group_kind, &group_id)?;
        let mode_value: String = row.get("mode");
        let mode = domain::ConsolePolicyMode::parse(&mode_value)
            .ok_or_else(|| anyhow!("stored console policy mode is invalid"))?;
        let stored_group = stored_groups
            .entry(group_policy_id)
            .or_insert_with(|| (group, mode, Vec::new()));
        let Some(operation_id) = row.get::<Option<String>, _>("operation_id") else {
            continue;
        };
        let operation_id = domain::ConsoleOperationId::try_from(operation_id)?;
        let policy_kind: String = row.get("policy_kind");
        let operation = match policy_kind.as_str() {
            "simple" => domain::ConsoleOperationPolicy::simple(
                operation_id,
                row.get::<Option<bool>, _>("simple_enabled")
                    .ok_or_else(|| anyhow!("stored simple console policy is missing enabled"))?,
            ),
            "row" => {
                let row_scope = row
                    .get::<Option<String>, _>("row_scope")
                    .as_deref()
                    .and_then(domain::ConsoleOperationRowScope::parse)
                    .ok_or_else(|| anyhow!("stored console row scope is invalid"))?;
                domain::ConsoleOperationPolicy::row(operation_id, row_scope)
            }
            _ => return Err(anyhow!("stored console operation policy kind is invalid")),
        };
        stored_group.2.push(operation);
    }
    let groups = stored_groups
        .into_values()
        .map(|(group, mode, operations)| match mode {
            domain::ConsolePolicyMode::Disabled => domain::RoleConsoleGroupPolicy::disabled(group),
            domain::ConsolePolicyMode::Full => domain::RoleConsoleGroupPolicy::full(group),
            domain::ConsolePolicyMode::Custom => {
                domain::RoleConsoleGroupPolicy::custom(group, operations)
            }
        })
        .collect();
    Ok(domain::RoleConsolePolicy::new(role_id, groups))
}

async fn replace_role_console_policy_rows(
    tx: &mut Transaction<'_, Postgres>,
    role_id: Uuid,
    actor_user_id: Uuid,
    groups: &[domain::RoleConsoleGroupPolicy],
) -> Result<()> {
    sqlx::query("delete from role_console_group_policies where role_id = $1")
        .bind(role_id)
        .execute(&mut **tx)
        .await?;
    for group_policy in groups {
        let group_policy_id = Uuid::now_v7();
        sqlx::query(
            r#"
            insert into role_console_group_policies (
              id, role_id, group_kind, group_id, mode, created_by, updated_by
            )
            values ($1, $2, $3, $4, $5, $6, $6)
            "#,
        )
        .bind(group_policy_id)
        .bind(role_id)
        .bind(group_policy.group().kind().as_str())
        .bind(group_policy.group().group_id().as_str())
        .bind(group_policy.mode().as_str())
        .bind(actor_user_id)
        .execute(&mut **tx)
        .await?;

        for operation in group_policy.operations() {
            sqlx::query(
                r#"
                insert into role_console_operation_policies (
                  id, role_id, group_policy_id, group_mode, operation_id, policy_kind,
                  simple_enabled, row_scope, created_by, updated_by
                )
                values ($1, $2, $3, 'custom', $4, $5, $6, $7, $8, $8)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(role_id)
            .bind(group_policy_id)
            .bind(operation.operation_id().as_str())
            .bind(operation.policy_kind())
            .bind(operation.simple_enabled())
            .bind(operation.row_scope().map(|scope| scope.as_str()))
            .bind(actor_user_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

fn normalized_migration_source(
    source: &RoleConsolePolicyMigrationSource,
) -> Result<RoleConsolePolicyMigrationSource> {
    let permission_resources = source
        .permission_resources
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let exact_permission_codes = source
        .exact_permission_codes
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if permission_resources.is_empty() && exact_permission_codes.is_empty() {
        return Err(ControlPlaneError::InvalidInput("console_policy_migration_source").into());
    }
    Ok(RoleConsolePolicyMigrationSource {
        permission_resources,
        exact_permission_codes,
    })
}

async fn migration_grant_inventories(
    pool: &sqlx::PgPool,
    source: &RoleConsolePolicyMigrationSource,
) -> Result<Vec<RoleConsolePolicyMigrationGrantInventory>> {
    let rows = sqlx::query(
        r#"
        select
          role.id as role_id,
          role.workspace_id,
          role.code as role_code,
          coalesce((
            select array_agg(distinct definition.code order by definition.code)
            from role_permissions grant_row
            join permission_definitions definition on definition.id = grant_row.permission_id
            where grant_row.role_id = role.id
              and (
                definition.resource = any($1::text[])
                or definition.code = any($2::text[])
              )
          ), array[]::text[]) as source_grants
        from roles role
        where role.scope_kind = 'workspace'
        order by role.workspace_id, role.code, role.id
        "#,
    )
    .bind(&source.permission_resources)
    .bind(&source.exact_permission_codes)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| RoleConsolePolicyMigrationGrantInventory {
            role_id: row.get("role_id"),
            workspace_id: row.get("workspace_id"),
            role_code: row.get("role_code"),
            source_grants: row.get("source_grants"),
        })
        .collect())
}

async fn role_console_policy_migration_source_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    source: &RoleConsolePolicyMigrationSource,
    role_ids: &[Uuid],
) -> Result<Value> {
    Ok(sqlx::query_scalar(
        r#"
        with selected_definitions as (
          select definition.*
          from permission_definitions definition
          where definition.resource = any($1::text[])
             or definition.code = any($2::text[])
        ), selected_grants as (
          select grant_row.*
          from role_permissions grant_row
          join selected_definitions definition on definition.id = grant_row.permission_id
          where grant_row.role_id = any($3::uuid[])
        ), selected_bindings as (
          select binding.*
          from user_role_bindings binding
          where binding.role_id = any($3::uuid[])
        )
        select jsonb_build_object(
          'definitions', coalesce((
            select jsonb_agg(to_jsonb(definition) order by definition.code, definition.id)
            from selected_definitions definition
          ), '[]'::jsonb),
          'grants', coalesce((
            select jsonb_agg(to_jsonb(grant_row) order by grant_row.role_id, grant_row.permission_id, grant_row.id)
            from selected_grants grant_row
          ), '[]'::jsonb),
          'role_bindings', coalesce((
            select jsonb_agg(to_jsonb(binding) order by binding.role_id, binding.user_id, binding.id)
            from selected_bindings binding
          ), '[]'::jsonb)
        )
        "#,
    )
    .bind(&source.permission_resources)
    .bind(&source.exact_permission_codes)
    .bind(role_ids)
    .fetch_one(&mut **tx)
    .await?)
}

#[async_trait]
impl RoleRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        let workspace_id = workspace_id_for_user(self.pool(), actor_user_id).await?;
        let tenant_id = tenant_id_for_workspace(self.pool(), workspace_id).await?;
        AuthRepository::load_actor_context(self, actor_user_id, tenant_id, workspace_id, None).await
    }

    async fn list_roles(&self, workspace_id: Uuid) -> Result<Vec<domain::RoleTemplate>> {
        let rows = sqlx::query(
            r#"
            select
              id,
              code,
              name,
              introduction,
              scope_kind,
              is_builtin,
              is_editable,
              auto_grant_new_permissions,
              is_default_member_role
            from roles
            where scope_kind = 'workspace' and workspace_id = $1
            order by scope_kind asc, code asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        let mut roles = Vec::with_capacity(rows.len());
        for row in rows {
            let role = stored_role_from_row(row);
            let permissions = permission_codes_for_role(self.pool(), role.id).await?;
            roles.push(PgRoleMapper::to_role_template(role, permissions));
        }

        Ok(roles)
    }

    async fn create_team_role(&self, input: &CreateWorkspaceRoleInput) -> Result<()> {
        if find_role_by_code(self.pool(), input.workspace_id, &input.code)
            .await?
            .is_some()
        {
            return Err(ControlPlaneError::Conflict("role_code").into());
        }

        let mut tx = self.pool().begin().await?;
        if input.is_default_member_role {
            sqlx::query(
                "update roles set is_default_member_role = false where scope_kind = 'workspace' and workspace_id = $1",
            )
            .bind(input.workspace_id)
            .execute(&mut *tx)
            .await?;
        }

        let role_id = Uuid::now_v7();
        sqlx::query(
            r#"
            insert into roles (
                id, scope_id, scope_kind, workspace_id, code, name, introduction, is_builtin, is_editable,
                auto_grant_new_permissions, is_default_member_role, created_by, updated_by
            )
            values ($1, $2, 'workspace', $2, $3, $4, $5, false, true, $6, $7, $8, $8)
            "#,
        )
        .bind(role_id)
        .bind(input.workspace_id)
        .bind(&input.code)
        .bind(&input.name)
        .bind(&input.introduction)
        .bind(input.auto_grant_new_permissions)
        .bind(input.is_default_member_role)
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;
        insert_default_role_data_policy(&mut tx, role_id).await?;

        tx.commit().await?;
        Ok(())
    }

    async fn update_team_role(&self, input: &UpdateWorkspaceRoleInput) -> Result<()> {
        let role = find_role_by_code(self.pool(), input.workspace_id, &input.role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;
        if role.code == "root"
            || !role.is_editable
            || matches!(role.scope_kind, RoleScopeKind::System)
        {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }
        if matches!(input.is_default_member_role, Some(false)) && role.is_default_member_role {
            return Err(ControlPlaneError::InvalidInput("default_member_role_required").into());
        }

        let mut tx = self.pool().begin().await?;
        if matches!(input.is_default_member_role, Some(true)) {
            sqlx::query(
                "update roles set is_default_member_role = false where scope_kind = 'workspace' and workspace_id = $1 and id <> $2",
            )
            .bind(input.workspace_id)
            .bind(role.id)
            .execute(&mut *tx)
            .await?;
        }

        let result = sqlx::query(
            r#"
            update roles
            set name = $2,
                introduction = $3,
                auto_grant_new_permissions = coalesce($4, auto_grant_new_permissions),
                is_default_member_role = coalesce($5, is_default_member_role),
                updated_by = $6,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(role.id)
        .bind(&input.name)
        .bind(&input.introduction)
        .bind(input.auto_grant_new_permissions)
        .bind(input.is_default_member_role)
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("role").into());
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete_team_role(
        &self,
        _actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<()> {
        let role = find_role_by_code(self.pool(), workspace_id, role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;
        if role.code == "root"
            || role.is_builtin
            || matches!(role.scope_kind, RoleScopeKind::System)
        {
            return Err(ControlPlaneError::PermissionDenied("builtin_role_immutable").into());
        }
        if role.is_default_member_role {
            return Err(ControlPlaneError::InvalidInput("default_member_role_required").into());
        }

        let binding_count: i64 =
            sqlx::query_scalar("select count(*) from user_role_bindings where role_id = $1")
                .bind(role.id)
                .fetch_one(self.pool())
                .await?;
        if binding_count > 0 {
            return Err(ControlPlaneError::Conflict("role_in_use").into());
        }

        sqlx::query("delete from roles where id = $1")
            .bind(role.id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn replace_role_permissions(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
        permission_codes: &[String],
    ) -> Result<()> {
        let role = find_role_by_code(self.pool(), workspace_id, role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;
        if role.code == "root" || !role.is_editable {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }

        let normalized_codes = permission_codes
            .iter()
            .map(|code| code.trim())
            .filter(|code| !code.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut permission_ids = Vec::with_capacity(normalized_codes.len());
        for permission_code in &normalized_codes {
            let permission_id: Uuid =
                sqlx::query_scalar("select id from permission_definitions where code = $1")
                    .bind(permission_code)
                    .fetch_optional(self.pool())
                    .await?
                    .ok_or(ControlPlaneError::InvalidInput("permission_code"))?;
            permission_ids.push(permission_id);
        }

        let mut tx = self.pool().begin().await?;
        sqlx::query("delete from role_permissions where role_id = $1")
            .bind(role.id)
            .execute(&mut *tx)
            .await?;

        for permission_id in permission_ids {
            sqlx::query(
                r#"
                insert into role_permissions (id, role_id, permission_id, scope_id, created_by, updated_by)
                select $1, roles.id, $3, roles.scope_id, $4, $4
                from roles
                where roles.id = $2
                on conflict (role_id, permission_id) do nothing
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(role.id)
            .bind(permission_id)
            .bind(actor_user_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_role_permissions(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<Vec<String>> {
        let role = find_role_by_code(self.pool(), workspace_id, role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;

        permission_codes_for_role(self.pool(), role.id).await
    }

    async fn get_role_console_policy(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<domain::RoleConsolePolicy> {
        let role = find_role_by_code(self.pool(), workspace_id, role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;
        role_console_policy_by_id(self.pool(), role.id).await
    }

    async fn replace_role_console_policy(
        &self,
        input: &ReplaceRoleConsolePolicyInput,
    ) -> Result<domain::RoleConsolePolicy> {
        let role = find_role_by_code(self.pool(), input.workspace_id, &input.role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;
        if role.code == "root" || !role.is_editable {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }

        let mut tx = self.pool().begin().await?;
        replace_role_console_policy_rows(&mut tx, role.id, input.actor_user_id, &input.groups)
            .await?;
        tx.commit().await?;

        self.get_role_console_policy(input.workspace_id, &input.role_code)
            .await
    }

    async fn get_role_data_policy(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<control_plane::ports::RoleDataPolicyView> {
        let role = find_role_by_code(self.pool(), workspace_id, role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;

        let row = sqlx::query(
            r#"
            select
              id, role_id, can_view, can_create, can_update, can_delete,
              default_view_scope, default_update_scope, default_delete_scope,
              created_at, updated_at
            from role_data_policies
            where role_id = $1
            "#,
        )
        .bind(role.id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("role_data_policy"))?;

        let default_policy = domain::RoleDataPolicyRecord {
            id: row.get("id"),
            role_id: row.get("role_id"),
            role_code: role.code.clone(),
            can_view: row.get("can_view"),
            can_create: row.get("can_create"),
            can_update: row.get("can_update"),
            can_delete: row.get("can_delete"),
            default_view_scope: data_policy_scope_from_db(row.get("default_view_scope")),
            default_update_scope: data_policy_scope_from_db(row.get("default_update_scope")),
            default_delete_scope: data_policy_scope_from_db(row.get("default_delete_scope")),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        let rows = sqlx::query(
            r#"
            select
              id, role_id, data_model_id, can_create_override, view_scope_override, update_scope_override,
              delete_scope_override, created_at, updated_at
            from role_data_model_policies
            where role_id = $1
            order by data_model_id asc
            "#,
        )
        .bind(role.id)
        .fetch_all(self.pool())
        .await?;
        let model_policies = rows
            .into_iter()
            .map(|row| domain::RoleDataModelPolicyRecord {
                id: row.get("id"),
                role_id: row.get("role_id"),
                data_model_id: row.get("data_model_id"),
                can_create_override: row.get("can_create_override"),
                view_scope_override: optional_data_policy_scope_from_db(
                    row.get("view_scope_override"),
                ),
                update_scope_override: optional_data_policy_scope_from_db(
                    row.get("update_scope_override"),
                ),
                delete_scope_override: optional_data_policy_scope_from_db(
                    row.get("delete_scope_override"),
                ),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect();

        Ok(control_plane::ports::RoleDataPolicyView {
            role_code: role.code,
            default_policy,
            model_policies,
        })
    }

    async fn replace_role_data_policy(
        &self,
        input: &ReplaceRoleDataPolicyInput,
    ) -> Result<control_plane::ports::RoleDataPolicyView> {
        let role = find_role_by_code(self.pool(), input.workspace_id, &input.role_code)
            .await?
            .ok_or(ControlPlaneError::NotFound("role"))?;
        if role.code == "root" || !role.is_editable {
            return Err(ControlPlaneError::PermissionDenied("root_role_immutable").into());
        }

        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"
            insert into role_data_policies (
                id, role_id, can_view, can_create, can_update, can_delete,
                default_view_scope, default_update_scope, default_delete_scope, created_by, updated_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            on conflict (role_id) do update set
                can_view = excluded.can_view,
                can_create = excluded.can_create,
                can_update = excluded.can_update,
                can_delete = excluded.can_delete,
                default_view_scope = excluded.default_view_scope,
                default_update_scope = excluded.default_update_scope,
                default_delete_scope = excluded.default_delete_scope,
                updated_by = excluded.updated_by,
                updated_at = now()
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(role.id)
        .bind(input.default_policy.can_view)
        .bind(input.default_policy.can_create)
        .bind(input.default_policy.can_update)
        .bind(input.default_policy.can_delete)
        .bind(input.default_policy.default_view_scope.as_str())
        .bind(input.default_policy.default_update_scope.as_str())
        .bind(input.default_policy.default_delete_scope.as_str())
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("delete from role_data_model_policies where role_id = $1")
            .bind(role.id)
            .execute(&mut *tx)
            .await?;
        for model_policy in &input.model_policies {
            sqlx::query(
                r#"
                insert into role_data_model_policies (
                    id, role_id, data_model_id, can_create_override, view_scope_override,
                    update_scope_override, delete_scope_override, created_by, updated_by
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(role.id)
            .bind(model_policy.data_model_id)
            .bind(model_policy.can_create_override)
            .bind(model_policy.view_scope_override.map(|scope| scope.as_str()))
            .bind(
                model_policy
                    .update_scope_override
                    .map(|scope| scope.as_str()),
            )
            .bind(
                model_policy
                    .delete_scope_override
                    .map(|scope| scope.as_str()),
            )
            .bind(input.actor_user_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        self.get_role_data_policy(input.workspace_id, &input.role_code)
            .await
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}

#[async_trait]
impl RoleConsolePolicyMigrationRepository for PgControlPlaneStore {
    async fn list_role_console_policy_migration_grants(
        &self,
        source: &RoleConsolePolicyMigrationSource,
    ) -> Result<Vec<RoleConsolePolicyMigrationGrantInventory>> {
        let source = normalized_migration_source(source)?;
        migration_grant_inventories(self.pool(), &source).await
    }

    async fn rehearse_role_console_policy_migration(
        &self,
        input: &RoleConsolePolicyMigrationRehearsalInput,
    ) -> Result<()> {
        let source = normalized_migration_source(&input.source)?;
        for value in [
            &input.source_contract,
            &input.catalog_fingerprint,
            &input.mapping_fingerprint,
        ] {
            if value.is_empty() || value.trim() != value {
                return Err(
                    ControlPlaneError::InvalidInput("console_policy_migration_revision").into(),
                );
            }
        }

        let preview_by_role = input
            .previews
            .iter()
            .map(|preview| (preview.policy.role_id(), preview))
            .collect::<BTreeMap<_, _>>();
        if preview_by_role.is_empty()
            || preview_by_role.len() != input.previews.len()
            || input.previews.iter().any(|preview| {
                !preview.authorization_delta.added.is_empty()
                    || !preview.authorization_delta.removed.is_empty()
                    || preview.effective_before != preview.effective_after
                    || !preview.effective_delta.is_empty()
            })
        {
            return Err(ControlPlaneError::InvalidInput(
                "console_policy_migration_authorization_delta",
            )
            .into());
        }

        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtext('role_console_policy_migration'))")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "lock table roles, permission_definitions, role_permissions, user_role_bindings in share mode",
        )
        .execute(&mut *tx)
        .await?;
        let inventories = migration_grant_inventories(self.pool(), &source).await?;
        let inventory_by_role = inventories
            .iter()
            .map(|inventory| (inventory.role_id, inventory))
            .collect::<BTreeMap<_, _>>();
        if inventory_by_role.len() != preview_by_role.len()
            || inventory_by_role.keys().ne(preview_by_role.keys())
            || preview_by_role.iter().any(|(role_id, preview)| {
                inventory_by_role.get(role_id).is_none_or(|inventory| {
                    preview.source_grants.iter().cloned().collect::<Vec<_>>()
                        != inventory.source_grants
                })
            })
        {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_source_drift").into(),
            );
        }

        let role_ids = preview_by_role.keys().copied().collect::<Vec<_>>();
        let source_snapshot =
            role_console_policy_migration_source_snapshot(&mut tx, &source, &role_ids).await?;
        sqlx::query(
            r#"
            insert into role_console_policy_migration_runs (
              id, source_contract, catalog_fingerprint, mapping_fingerprint,
              source_filter, source_snapshot, status, write_fenced
            )
            values ($1, $2, $3, $4, $5, $6, 'previewed', false)
            "#,
        )
        .bind(input.run_id)
        .bind(&input.source_contract)
        .bind(&input.catalog_fingerprint)
        .bind(&input.mapping_fingerprint)
        .bind(serde_json::to_value(&source)?)
        .bind(source_snapshot)
        .execute(&mut *tx)
        .await?;

        for preview in preview_by_role.values() {
            sqlx::query(
                r#"
                insert into role_console_policy_migration_role_previews (
                  run_id, role_id, source_grants,
                  projected_policy, authorization_delta, effective_before,
                  effective_after, effective_delta, status
                )
                values (
                  $1, $2, $3, $4, $5, $6, $7, $8,
                  'previewed'
                )
                "#,
            )
            .bind(input.run_id)
            .bind(preview.policy.role_id())
            .bind(serde_json::to_value(&preview.source_grants)?)
            .bind(serde_json::to_value(&preview.policy)?)
            .bind(serde_json::to_value(&preview.authorization_delta)?)
            .bind(serde_json::to_value(&preview.effective_before)?)
            .bind(serde_json::to_value(&preview.effective_after)?)
            .bind(serde_json::to_value(&preview.effective_delta)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn apply_role_console_policy_migration(
        &self,
        input: &RoleConsolePolicyMigrationRehearsalInput,
        actor_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtext('role_console_policy_migration'))")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "lock table roles, permission_definitions, role_permissions, user_role_bindings in share mode",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "lock table role_console_group_policies, role_console_operation_policies in share row exclusive mode",
        )
        .execute(&mut *tx)
        .await?;
        let run = sqlx::query(
            r#"
            select source_contract, catalog_fingerprint, mapping_fingerprint,
                   source_filter, source_snapshot, status
            from role_console_policy_migration_runs
            where id = $1
            for update
            "#,
        )
        .bind(input.run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("console_policy_migration_run"))?;
        if run.get::<String, _>("status") != "previewed"
            || run.get::<String, _>("source_contract") != input.source_contract
            || run.get::<String, _>("catalog_fingerprint") != input.catalog_fingerprint
            || run.get::<String, _>("mapping_fingerprint") != input.mapping_fingerprint
        {
            return Err(ControlPlaneError::Conflict("console_policy_migration_revision").into());
        }
        let stored_source: RoleConsolePolicyMigrationSource =
            serde_json::from_value(run.get("source_filter"))?;
        if stored_source != normalized_migration_source(&input.source)? {
            return Err(ControlPlaneError::Conflict("console_policy_migration_source").into());
        }

        let preview_by_role = input
            .previews
            .iter()
            .map(|preview| (preview.policy.role_id(), preview))
            .collect::<BTreeMap<_, _>>();
        if preview_by_role.len() != input.previews.len() {
            return Err(ControlPlaneError::InvalidInput("console_policy_migration_preview").into());
        }
        let role_ids = preview_by_role.keys().copied().collect::<Vec<_>>();
        let current_source_snapshot =
            role_console_policy_migration_source_snapshot(&mut tx, &stored_source, &role_ids)
                .await?;
        if current_source_snapshot != run.get::<Value, _>("source_snapshot") {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_source_drift").into(),
            );
        }

        let ledger_rows = sqlx::query(
            r#"
            select role_id, source_grants, projected_policy, authorization_delta,
                   effective_before, effective_after, effective_delta
            from role_console_policy_migration_role_previews
            where run_id = $1
            order by role_id
            "#,
        )
        .bind(input.run_id)
        .fetch_all(&mut *tx)
        .await?;
        if ledger_rows.len() != preview_by_role.len()
            || ledger_rows.iter().any(|row| {
                let role_id: Uuid = row.get("role_id");
                preview_by_role.get(&role_id).is_none_or(|preview| {
                    row.get::<Value, _>("source_grants")
                        != serde_json::to_value(&preview.source_grants).unwrap_or(Value::Null)
                        || row.get::<Value, _>("projected_policy")
                            != serde_json::to_value(&preview.policy).unwrap_or(Value::Null)
                        || row.get::<Value, _>("authorization_delta")
                            != serde_json::to_value(&preview.authorization_delta)
                                .unwrap_or(Value::Null)
                        || row.get::<Value, _>("effective_before")
                            != serde_json::to_value(&preview.effective_before)
                                .unwrap_or(Value::Null)
                        || row.get::<Value, _>("effective_after")
                            != serde_json::to_value(&preview.effective_after).unwrap_or(Value::Null)
                        || row.get::<Value, _>("effective_delta")
                            != serde_json::to_value(&preview.effective_delta).unwrap_or(Value::Null)
                })
            })
        {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_preview_drift").into(),
            );
        }

        sqlx::query("select set_config('oneflow.role_console_policy_migration_run_id', $1, true)")
            .bind(input.run_id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            update role_console_policy_migration_runs
            set status = 'applied_fenced', cutover_marker = 'console_policy',
                write_fenced = true, applied_at = now()
            where id = $1
            "#,
        )
        .bind(input.run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            insert into role_console_group_policy_snapshots (
              run_id, group_policy_id, role_id, group_kind, group_id, mode,
              created_by, created_at, updated_by, updated_at
            )
            select $1, id, role_id, group_kind, group_id, mode,
                   created_by, created_at, updated_by, updated_at
            from role_console_group_policies
            where role_id = any($2::uuid[])
            "#,
        )
        .bind(input.run_id)
        .bind(&role_ids)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            insert into role_console_operation_policy_snapshots (
              run_id, operation_policy_id, role_id, group_policy_id, group_mode,
              operation_id, policy_kind, simple_enabled, row_scope,
              created_by, created_at, updated_by, updated_at
            )
            select $1, id, role_id, group_policy_id, group_mode,
                   operation_id, policy_kind, simple_enabled, row_scope,
                   created_by, created_at, updated_by, updated_at
            from role_console_operation_policies
            where role_id = any($2::uuid[])
            "#,
        )
        .bind(input.run_id)
        .bind(&role_ids)
        .execute(&mut *tx)
        .await?;
        for preview in preview_by_role.values() {
            replace_role_console_policy_rows(
                &mut tx,
                preview.policy.role_id(),
                actor_user_id,
                preview.policy.groups(),
            )
            .await?;
        }
        sqlx::query(
            r#"
            update role_console_policy_migration_role_previews
            set status = 'applied', applied_at = now()
            where run_id = $1
            "#,
        )
        .bind(input.run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn finalize_role_console_policy_migration(
        &self,
        run_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtext('role_console_policy_migration'))")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "lock table roles, permission_definitions, role_permissions, user_role_bindings in share mode",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "lock table role_console_group_policies, role_console_operation_policies in share row exclusive mode",
        )
        .execute(&mut *tx)
        .await?;

        let run = sqlx::query(
            r#"
            select source_filter, source_snapshot, status, cutover_marker, write_fenced
            from role_console_policy_migration_runs
            where id = $1
            for update
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("console_policy_migration_run"))?;
        if run.get::<String, _>("status") != "applied_fenced"
            || run.get::<String, _>("cutover_marker") != "console_policy"
            || !run.get::<bool, _>("write_fenced")
        {
            return Err(ControlPlaneError::Conflict("console_policy_migration_state").into());
        }
        let fenced_run_id = sqlx::query_scalar::<_, Uuid>(
            "select id from role_console_policy_migration_runs where write_fenced for update",
        )
        .fetch_optional(&mut *tx)
        .await?;
        if fenced_run_id != Some(run_id) {
            return Err(ControlPlaneError::Conflict("console_policy_migration_fence_owner").into());
        }

        let role_ids = sqlx::query_scalar::<_, Uuid>(
            "select role_id from role_console_policy_migration_role_previews where run_id = $1 order by role_id",
        )
        .bind(run_id)
        .fetch_all(&mut *tx)
        .await?;
        let applied_preview_count: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from role_console_policy_migration_role_previews
            where run_id = $1 and status = 'applied'
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        if role_ids.is_empty() || applied_preview_count != role_ids.len() as i64 {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_preview_state").into(),
            );
        }

        let source: RoleConsolePolicyMigrationSource =
            serde_json::from_value(run.get("source_filter"))?;
        let current_source_snapshot =
            role_console_policy_migration_source_snapshot(&mut tx, &source, &role_ids).await?;
        if current_source_snapshot != run.get::<Value, _>("source_snapshot") {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_source_drift").into(),
            );
        }

        let result = sqlx::query(
            r#"
            update role_console_policy_migration_runs
            set status = 'applied', write_fenced = false,
                finalized_by = $2, finalized_at = now()
            where id = $1
              and status = 'applied_fenced'
              and cutover_marker = 'console_policy'
              and write_fenced
            "#,
        )
        .bind(run_id)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() != 1 {
            return Err(ControlPlaneError::Conflict("console_policy_migration_state").into());
        }
        tx.commit().await?;
        Ok(())
    }

    async fn rollback_role_console_policy_migration(
        &self,
        run_id: Uuid,
        _actor_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("select pg_advisory_xact_lock(hashtext('role_console_policy_migration'))")
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "lock table roles, permission_definitions, role_permissions, user_role_bindings in share mode",
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "lock table role_console_group_policies, role_console_operation_policies in share row exclusive mode",
        )
        .execute(&mut *tx)
        .await?;
        let run = sqlx::query(
            r#"
            select source_filter, source_snapshot, status
            from role_console_policy_migration_runs
            where id = $1
            for update
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("console_policy_migration_run"))?;
        if run.get::<String, _>("status") != "applied_fenced" {
            return Err(ControlPlaneError::Conflict("console_policy_migration_state").into());
        }
        let source: RoleConsolePolicyMigrationSource =
            serde_json::from_value(run.get("source_filter"))?;
        let role_ids = sqlx::query_scalar::<_, Uuid>(
            "select role_id from role_console_policy_migration_role_previews where run_id = $1 order by role_id",
        )
        .bind(run_id)
        .fetch_all(&mut *tx)
        .await?;
        let current_source_snapshot =
            role_console_policy_migration_source_snapshot(&mut tx, &source, &role_ids).await?;
        if current_source_snapshot != run.get::<Value, _>("source_snapshot") {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_source_drift").into(),
            );
        }

        sqlx::query("select set_config('oneflow.role_console_policy_migration_run_id', $1, true)")
            .bind(run_id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("delete from role_console_group_policies where role_id = any($1::uuid[])")
            .bind(&role_ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            insert into role_console_group_policies (
              id, role_id, group_kind, group_id, mode,
              created_by, created_at, updated_by, updated_at
            )
            select group_policy_id, role_id, group_kind, group_id, mode,
                   created_by, created_at, updated_by, updated_at
            from role_console_group_policy_snapshots
            where run_id = $1
            order by group_kind, group_id, group_policy_id
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            insert into role_console_operation_policies (
              id, role_id, group_policy_id, group_mode, operation_id, policy_kind,
              simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
            )
            select operation_policy_id, role_id, group_policy_id, group_mode,
                   operation_id, policy_kind, simple_enabled, row_scope,
                   created_by, created_at, updated_by, updated_at
            from role_console_operation_policy_snapshots
            where run_id = $1
            order by operation_id, operation_policy_id
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;

        let restored_exactly: bool = sqlx::query_scalar(
            r#"
            select
              not exists (
                (select id, role_id, group_kind, group_id, mode, created_by, created_at, updated_by, updated_at
                 from role_console_group_policies where role_id = any($2::uuid[])
                 except
                 select group_policy_id, role_id, group_kind, group_id, mode, created_by, created_at, updated_by, updated_at
                 from role_console_group_policy_snapshots where run_id = $1)
                union all
                (select group_policy_id, role_id, group_kind, group_id, mode, created_by, created_at, updated_by, updated_at
                 from role_console_group_policy_snapshots where run_id = $1
                 except
                 select id, role_id, group_kind, group_id, mode, created_by, created_at, updated_by, updated_at
                 from role_console_group_policies where role_id = any($2::uuid[]))
              )
              and not exists (
                (select id, role_id, group_policy_id, group_mode, operation_id, policy_kind,
                        simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policies where role_id = any($2::uuid[])
                 except
                 select operation_policy_id, role_id, group_policy_id, group_mode, operation_id,
                        policy_kind, simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policy_snapshots where run_id = $1)
                union all
                (select operation_policy_id, role_id, group_policy_id, group_mode, operation_id,
                        policy_kind, simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policy_snapshots where run_id = $1
                 except
                 select id, role_id, group_policy_id, group_mode, operation_id, policy_kind,
                        simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policies where role_id = any($2::uuid[]))
              )
            "#,
        )
        .bind(run_id)
        .bind(&role_ids)
        .fetch_one(&mut *tx)
        .await?;
        if !restored_exactly {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_rollback_delta").into(),
            );
        }

        sqlx::query(
            "update role_console_policy_migration_role_previews set status = 'rolled_back' where run_id = $1",
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            update role_console_policy_migration_runs
            set status = 'rolled_back', cutover_marker = 'legacy',
                write_fenced = false, rollback_verified_at = now()
            where id = $1
            "#,
        )
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
