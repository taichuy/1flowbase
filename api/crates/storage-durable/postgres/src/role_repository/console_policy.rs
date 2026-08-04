use super::*;

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
          group_policy.enabled,
          group_policy.strategy,
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
            bool,
            domain::ConsolePolicyStrategy,
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
        let enabled: bool = row.get("enabled");
        let strategy_value: String = row.get("strategy");
        let strategy = domain::ConsolePolicyStrategy::parse(&strategy_value)
            .ok_or_else(|| anyhow!("stored console policy strategy is invalid"))?;
        let stored_group = stored_groups
            .entry(group_policy_id)
            .or_insert_with(|| (group, enabled, strategy, Vec::new()));
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
        stored_group.3.push(operation);
    }
    let groups = stored_groups
        .into_values()
        .map(|(group, enabled, strategy, operations)| {
            domain::RoleConsoleGroupPolicy::new(group, enabled, strategy, operations)
        })
        .collect();
    Ok(domain::RoleConsolePolicy::new(role_id, groups))
}

pub(super) async fn replace_role_console_policy_rows(
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
              id, role_id, group_kind, group_id, enabled, strategy, created_by, updated_by
            )
            values ($1, $2, $3, $4, $5, $6, $7, $7)
            "#,
        )
        .bind(group_policy_id)
        .bind(role_id)
        .bind(group_policy.group().kind().as_str())
        .bind(group_policy.group().group_id().as_str())
        .bind(group_policy.enabled())
        .bind(group_policy.strategy().as_str())
        .bind(actor_user_id)
        .execute(&mut **tx)
        .await?;

        for operation in group_policy.operations() {
            sqlx::query(
                r#"
                insert into role_console_operation_policies (
                  id, role_id, group_policy_id, operation_id, policy_kind,
                  simple_enabled, row_scope, created_by, updated_by
                )
                values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
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

#[async_trait]
impl RoleConsolePolicyReader for PgControlPlaneStore {
    async fn load_role_console_policies_for_user(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        let role_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            select role.id
            from user_role_bindings binding
            join roles role on role.id = binding.role_id
            where binding.user_id = $1
              and (role.scope_kind = 'system' or role.workspace_id = $2)
            order by role.scope_kind asc, role.code asc, role.id asc
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        let mut policies = Vec::with_capacity(role_ids.len());
        for role_id in role_ids {
            policies.push(role_console_policy_by_id(self.pool(), role_id).await?);
        }
        Ok(policies)
    }
}
