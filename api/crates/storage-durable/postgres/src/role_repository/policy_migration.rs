use super::*;

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

async fn role_console_policy_migration_actor_bindings(
    tx: &mut Transaction<'_, Postgres>,
    role_ids: &[Uuid],
) -> Result<Vec<ConsolePolicyMigrationActorRoleBinding>> {
    let rows = sqlx::query(
        r#"
        select binding.user_id as actor_user_id,
               array_agg(binding.role_id order by binding.role_id) as role_ids
        from user_role_bindings binding
        where binding.role_id = any($1::uuid[])
        group by binding.user_id
        order by binding.user_id
        "#,
    )
    .bind(role_ids)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ConsolePolicyMigrationActorRoleBinding {
            actor_user_id: row.get("actor_user_id"),
            role_ids: row.get("role_ids"),
        })
        .collect())
}

fn actor_bindings_from_previews(
    actor_previews: &[ConsolePolicyMigrationActorPreview],
) -> Result<Vec<ConsolePolicyMigrationActorRoleBinding>> {
    let bindings = actor_previews
        .iter()
        .map(|preview| (preview.binding.actor_user_id, preview.binding.clone()))
        .collect::<BTreeMap<_, _>>();
    if bindings.len() != actor_previews.len() {
        return Err(
            ControlPlaneError::InvalidInput("console_policy_migration_actor_binding").into(),
        );
    }
    Ok(bindings.into_values().collect())
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
            input.plan.catalog_fingerprint(),
            input.plan.mapping_fingerprint(),
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
        validate_console_policy_migration_actor_previews(
            &input.plan,
            &input.previews,
            &input.actor_previews,
        )
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_migration_actor_preview"))?;

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
        for inventory in &inventories {
            let expected = input
                .plan
                .project_legacy_role(inventory.role_id, &inventory.source_grants)
                .map_err(|_| ControlPlaneError::InvalidInput("console_policy_migration_mapping"))?;
            if preview_by_role
                .get(&inventory.role_id)
                .is_none_or(|preview| *preview != &expected)
            {
                return Err(
                    ControlPlaneError::Conflict("console_policy_migration_preview_drift").into(),
                );
            }
        }

        let role_ids = preview_by_role.keys().copied().collect::<Vec<_>>();
        let source_actor_bindings =
            role_console_policy_migration_actor_bindings(&mut tx, &role_ids).await?;
        if actor_bindings_from_previews(&input.actor_previews)? != source_actor_bindings {
            return Err(ControlPlaneError::Conflict(
                "console_policy_migration_actor_binding_drift",
            )
            .into());
        }
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
        .bind(input.plan.catalog_fingerprint())
        .bind(input.plan.mapping_fingerprint())
        .bind(serde_json::to_value(&source)?)
        .bind(source_snapshot)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into role_console_policy_migration_run_artifacts (
              run_id, catalog_fingerprint, mapping_fingerprint,
              compiled_catalog, legacy_mappings, actor_role_bindings
            )
            values ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(input.run_id)
        .bind(input.plan.catalog_fingerprint())
        .bind(input.plan.mapping_fingerprint())
        .bind(serde_json::to_value(input.plan.catalog())?)
        .bind(serde_json::to_value(input.plan.mappings())?)
        .bind(serde_json::to_value(&source_actor_bindings)?)
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
        for preview in &input.actor_previews {
            sqlx::query(
                r#"
                insert into role_console_policy_migration_actor_previews (
                  run_id, actor_user_id, role_ids, probes,
                  effective_before, effective_after, effective_delta, status
                )
                values ($1, $2, $3, $4, $5, $6, $7, 'previewed')
                "#,
            )
            .bind(input.run_id)
            .bind(preview.binding.actor_user_id)
            .bind(serde_json::to_value(&preview.binding.role_ids)?)
            .bind(serde_json::to_value(&preview.probes)?)
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
            || run.get::<String, _>("catalog_fingerprint") != input.plan.catalog_fingerprint()
            || run.get::<String, _>("mapping_fingerprint") != input.plan.mapping_fingerprint()
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
        validate_console_policy_migration_actor_previews(
            &input.plan,
            &input.previews,
            &input.actor_previews,
        )
        .map_err(|_| ControlPlaneError::InvalidInput("console_policy_migration_actor_preview"))?;
        let role_ids = preview_by_role.keys().copied().collect::<Vec<_>>();
        let source_actor_bindings =
            role_console_policy_migration_actor_bindings(&mut tx, &role_ids).await?;
        if actor_bindings_from_previews(&input.actor_previews)? != source_actor_bindings {
            return Err(ControlPlaneError::Conflict(
                "console_policy_migration_actor_binding_drift",
            )
            .into());
        }
        let current_source_snapshot =
            role_console_policy_migration_source_snapshot(&mut tx, &stored_source, &role_ids)
                .await?;
        if current_source_snapshot != run.get::<Value, _>("source_snapshot") {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_source_drift").into(),
            );
        }

        let artifacts = sqlx::query(
            r#"
            select catalog_fingerprint, mapping_fingerprint, compiled_catalog,
                   legacy_mappings, actor_role_bindings
            from role_console_policy_migration_run_artifacts
            where run_id = $1
            for update
            "#,
        )
        .bind(input.run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::Conflict(
            "console_policy_migration_artifact",
        ))?;
        if artifacts.get::<String, _>("catalog_fingerprint") != input.plan.catalog_fingerprint()
            || artifacts.get::<String, _>("mapping_fingerprint") != input.plan.mapping_fingerprint()
            || artifacts.get::<Value, _>("compiled_catalog")
                != serde_json::to_value(input.plan.catalog()).unwrap_or(Value::Null)
            || artifacts.get::<Value, _>("legacy_mappings")
                != serde_json::to_value(input.plan.mappings()).unwrap_or(Value::Null)
            || artifacts.get::<Value, _>("actor_role_bindings")
                != serde_json::to_value(&source_actor_bindings).unwrap_or(Value::Null)
        {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_artifact_drift").into(),
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

        let actor_preview_by_actor = input
            .actor_previews
            .iter()
            .map(|preview| (preview.binding.actor_user_id, preview))
            .collect::<BTreeMap<_, _>>();
        let actor_ledger_rows = sqlx::query(
            r#"
            select actor_user_id, role_ids, probes, effective_before, effective_after, effective_delta
            from role_console_policy_migration_actor_previews
            where run_id = $1
            order by actor_user_id
            "#,
        )
        .bind(input.run_id)
        .fetch_all(&mut *tx)
        .await?;
        if actor_ledger_rows.len() != actor_preview_by_actor.len()
            || actor_ledger_rows.iter().any(|row| {
                let actor_user_id: Uuid = row.get("actor_user_id");
                actor_preview_by_actor
                    .get(&actor_user_id)
                    .is_none_or(|preview| {
                        row.get::<Value, _>("role_ids")
                            != serde_json::to_value(&preview.binding.role_ids)
                                .unwrap_or(Value::Null)
                            || row.get::<Value, _>("probes")
                                != serde_json::to_value(&preview.probes).unwrap_or(Value::Null)
                            || row.get::<Value, _>("effective_before")
                                != serde_json::to_value(&preview.effective_before)
                                    .unwrap_or(Value::Null)
                            || row.get::<Value, _>("effective_after")
                                != serde_json::to_value(&preview.effective_after)
                                    .unwrap_or(Value::Null)
                            || row.get::<Value, _>("effective_delta")
                                != serde_json::to_value(&preview.effective_delta)
                                    .unwrap_or(Value::Null)
                    })
            })
        {
            return Err(ControlPlaneError::Conflict(
                "console_policy_migration_actor_preview_drift",
            )
            .into());
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
        let cutover_marker: String = sqlx::query_scalar(
            "select marker from role_console_policy_migration_cutover_state where singleton for update",
        )
        .fetch_one(&mut *tx)
        .await?;
        if cutover_marker != "legacy" {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_cutover_state").into(),
            );
        }
        let cutover_update = sqlx::query(
            r#"
            update role_console_policy_migration_cutover_state
            set marker = 'fenced', run_id = $1,
                catalog_fingerprint = $2, mapping_fingerprint = $3,
                updated_at = now()
            where singleton and marker = 'legacy'
            "#,
        )
        .bind(input.run_id)
        .bind(input.plan.catalog_fingerprint())
        .bind(input.plan.mapping_fingerprint())
        .execute(&mut *tx)
        .await?;
        if cutover_update.rows_affected() != 1 {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_cutover_state").into(),
            );
        }
        sqlx::query(
            r#"
            insert into role_console_group_policy_snapshots (
              run_id, group_policy_id, role_id, group_kind, group_id, mode,
              created_by, created_at, updated_by, updated_at, enabled, strategy
            )
            select $1, id, role_id, group_kind, group_id,
                   case when not enabled then 'disabled' else strategy end,
                   created_by, created_at, updated_by, updated_at, enabled, strategy
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
            select $1, id, role_id, group_policy_id, 'custom',
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
        sqlx::query(
            r#"
            update role_console_policy_migration_actor_previews
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
            select source_filter, source_snapshot, status, cutover_marker, write_fenced,
                   catalog_fingerprint, mapping_fingerprint
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
        let cutover = sqlx::query(
            r#"
            select marker, run_id, catalog_fingerprint, mapping_fingerprint
            from role_console_policy_migration_cutover_state
            where singleton
            for update
            "#,
        )
        .fetch_one(&mut *tx)
        .await?;
        if cutover.get::<String, _>("marker") != "fenced"
            || cutover.get::<Option<Uuid>, _>("run_id") != Some(run_id)
            || cutover.get::<Option<String>, _>("catalog_fingerprint")
                != Some(run.get::<String, _>("catalog_fingerprint"))
            || cutover.get::<Option<String>, _>("mapping_fingerprint")
                != Some(run.get::<String, _>("mapping_fingerprint"))
        {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_cutover_state").into(),
            );
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
        let actor_preview_count: i64 = sqlx::query_scalar(
            "select count(*) from role_console_policy_migration_actor_previews where run_id = $1",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        let applied_actor_preview_count: i64 = sqlx::query_scalar(
            r#"
            select count(*)
            from role_console_policy_migration_actor_previews
            where run_id = $1 and status = 'applied'
            "#,
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        if actor_preview_count != applied_actor_preview_count {
            return Err(ControlPlaneError::Conflict(
                "console_policy_migration_actor_preview_state",
            )
            .into());
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
        let cutover_result = sqlx::query(
            r#"
            update role_console_policy_migration_cutover_state
            set marker = 'console_policy', updated_at = now()
            where singleton and marker = 'fenced' and run_id = $1
              and catalog_fingerprint = $2 and mapping_fingerprint = $3
            "#,
        )
        .bind(run_id)
        .bind(run.get::<String, _>("catalog_fingerprint"))
        .bind(run.get::<String, _>("mapping_fingerprint"))
        .execute(&mut *tx)
        .await?;
        if cutover_result.rows_affected() != 1 {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_cutover_state").into(),
            );
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
            select source_filter, source_snapshot, status, catalog_fingerprint, mapping_fingerprint
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
        let cutover = sqlx::query(
            r#"
            select marker, run_id, catalog_fingerprint, mapping_fingerprint
            from role_console_policy_migration_cutover_state
            where singleton
            for update
            "#,
        )
        .fetch_one(&mut *tx)
        .await?;
        if cutover.get::<String, _>("marker") != "fenced"
            || cutover.get::<Option<Uuid>, _>("run_id") != Some(run_id)
            || cutover.get::<Option<String>, _>("catalog_fingerprint")
                != Some(run.get::<String, _>("catalog_fingerprint"))
            || cutover.get::<Option<String>, _>("mapping_fingerprint")
                != Some(run.get::<String, _>("mapping_fingerprint"))
        {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_cutover_state").into(),
            );
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
              id, role_id, group_kind, group_id, enabled, strategy,
              created_by, created_at, updated_by, updated_at
            )
            select group_policy_id, role_id, group_kind, group_id, enabled, strategy,
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
              id, role_id, group_policy_id, operation_id, policy_kind,
              simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
            )
            select operation_policy_id, role_id, group_policy_id,
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
                (select id, role_id, group_kind, group_id, enabled, strategy, created_by, created_at, updated_by, updated_at
                 from role_console_group_policies where role_id = any($2::uuid[])
                 except
                 select group_policy_id, role_id, group_kind, group_id, enabled, strategy, created_by, created_at, updated_by, updated_at
                 from role_console_group_policy_snapshots where run_id = $1)
                union all
                (select group_policy_id, role_id, group_kind, group_id, enabled, strategy, created_by, created_at, updated_by, updated_at
                 from role_console_group_policy_snapshots where run_id = $1
                 except
                 select id, role_id, group_kind, group_id, enabled, strategy, created_by, created_at, updated_by, updated_at
                 from role_console_group_policies where role_id = any($2::uuid[]))
              )
              and not exists (
                (select id, role_id, group_policy_id, operation_id, policy_kind,
                        simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policies where role_id = any($2::uuid[])
                 except
                 select operation_policy_id, role_id, group_policy_id, operation_id,
                        policy_kind, simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policy_snapshots where run_id = $1)
                union all
                (select operation_policy_id, role_id, group_policy_id, operation_id,
                        policy_kind, simple_enabled, row_scope, created_by, created_at, updated_by, updated_at
                 from role_console_operation_policy_snapshots where run_id = $1
                 except
                 select id, role_id, group_policy_id, operation_id, policy_kind,
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
            "update role_console_policy_migration_actor_previews set status = 'rolled_back' where run_id = $1",
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
        let cutover_result = sqlx::query(
            r#"
            update role_console_policy_migration_cutover_state
            set marker = 'legacy', run_id = null,
                catalog_fingerprint = null, mapping_fingerprint = null,
                updated_at = now()
            where singleton and marker = 'fenced' and run_id = $1
              and catalog_fingerprint = $2 and mapping_fingerprint = $3
            "#,
        )
        .bind(run_id)
        .bind(run.get::<String, _>("catalog_fingerprint"))
        .bind(run.get::<String, _>("mapping_fingerprint"))
        .execute(&mut *tx)
        .await?;
        if cutover_result.rows_affected() != 1 {
            return Err(
                ControlPlaneError::Conflict("console_policy_migration_cutover_state").into(),
            );
        }
        tx.commit().await?;
        Ok(())
    }

    async fn role_console_policy_migration_cutover_state(
        &self,
    ) -> Result<RoleConsolePolicyMigrationCutoverState> {
        let row = sqlx::query(
            r#"
            select marker, run_id, catalog_fingerprint, mapping_fingerprint
            from role_console_policy_migration_cutover_state
            where singleton
            "#,
        )
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound(
            "console_policy_migration_cutover_state",
        ))?;
        let marker = match row.get::<String, _>("marker").as_str() {
            "legacy" => RoleConsolePolicyMigrationCutoverMarker::Legacy,
            "fenced" => RoleConsolePolicyMigrationCutoverMarker::Fenced,
            "console_policy" => RoleConsolePolicyMigrationCutoverMarker::ConsolePolicy,
            _ => {
                return Err(ControlPlaneError::InvalidInput(
                    "console_policy_migration_cutover_state",
                )
                .into())
            }
        };
        Ok(RoleConsolePolicyMigrationCutoverState {
            marker,
            run_id: row.get("run_id"),
            catalog_fingerprint: row.get("catalog_fingerprint"),
            mapping_fingerprint: row.get("mapping_fingerprint"),
        })
    }
}
