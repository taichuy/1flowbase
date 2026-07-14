use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        AuthRepository, CreateWorkspaceRoleInput, ReplaceRoleConsolePolicyInput,
        ReplaceRoleDataPolicyInput, RoleDataPolicyDefaultsInput, RoleRepository,
        UpdateWorkspaceRoleInput,
    },
};
use domain::{ActorContext, AuditLogRecord, RoleScopeKind};
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
            order by
              group_policy.group_kind,
              group_policy.group_id,
              operation_policy.operation_id
            "#,
        )
        .bind(role.id)
        .fetch_all(self.pool())
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

            let operation_id: Option<String> = row.get("operation_id");
            let Some(operation_id) = operation_id else {
                continue;
            };
            let operation_id = domain::ConsoleOperationId::try_from(operation_id)?;
            let policy_kind: String = row.get("policy_kind");
            let operation = match policy_kind.as_str() {
                "simple" => domain::ConsoleOperationPolicy::simple(
                    operation_id,
                    row.get::<Option<bool>, _>("simple_enabled")
                        .ok_or_else(|| {
                            anyhow!("stored simple console policy is missing enabled")
                        })?,
                ),
                "row" => {
                    let row_scope: Option<String> = row.get("row_scope");
                    let row_scope = row_scope
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
                domain::ConsolePolicyMode::Disabled => {
                    domain::RoleConsoleGroupPolicy::disabled(group)
                }
                domain::ConsolePolicyMode::Full => domain::RoleConsoleGroupPolicy::full(group),
                domain::ConsolePolicyMode::Custom => {
                    domain::RoleConsoleGroupPolicy::custom(group, operations)
                }
            })
            .collect();
        Ok(domain::RoleConsolePolicy::new(role.id, groups))
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
        sqlx::query("delete from role_console_group_policies where role_id = $1")
            .bind(role.id)
            .execute(&mut *tx)
            .await?;
        for group_policy in &input.groups {
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
            .bind(role.id)
            .bind(group_policy.group().kind().as_str())
            .bind(group_policy.group().group_id().as_str())
            .bind(group_policy.mode().as_str())
            .bind(input.actor_user_id)
            .execute(&mut *tx)
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
                .bind(role.id)
                .bind(group_policy_id)
                .bind(operation.operation_id().as_str())
                .bind(operation.policy_kind())
                .bind(operation.simple_enabled())
                .bind(operation.row_scope().map(|scope| scope.as_str()))
                .bind(input.actor_user_id)
                .execute(&mut *tx)
                .await?;
            }
        }
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
