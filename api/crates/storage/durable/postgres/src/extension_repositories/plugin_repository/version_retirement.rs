use anyhow::Result;
use control_plane_contracts::{
    ports::{
        CommitProviderVersionSwitchInput, CreatePluginAssignmentInput,
        ProviderVersionSwitchOutcome, UpdatePluginDesiredStateInput,
    },
    ControlPlaneContractError as ControlPlaneError,
};
use uuid::Uuid;

use super::{
    guard_managed_desired_state, lock_managed_dependency_scope, lock_managed_desired_state_scopes,
    lock_node_artifact_slot,
    locked_artifact::{lock_artifact_installation, verify_locked_node_artifact_on_disk},
    write_plugin_desired_state,
};
use crate::repositories::PgControlPlaneStore;

// Shared by startup reconciliation and switch transactions. When a single installation is
// provided, PostgreSQL can push the filter into the union arms.
pub(crate) async fn referenced_installation_ids<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    installation_id: Option<Uuid>,
) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        r#"
        select distinct installation_id
        from (
            select installation_id from plugin_assignments
            union all select installation_id from model_provider_instances
            union all select installation_id from model_provider_preview_sessions where expires_at > now()
            union all select installation_id from data_source_instances
            union all select installation_id from network_egress_providers
            union all select installation_id from notification_instances
            union all select installation_id from application_js_dependency_selections
            union all select installation_id from native_plugin_targets
        ) refs
        where installation_id is not null and ($1::uuid is null or installation_id = $1)
        "#,
    )
    .bind(installation_id)
    .fetch_all(executor)
    .await?)
}

pub(crate) async fn lock_plugin_family(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    category: &str,
    organization: &str,
    provider_code: &str,
) -> Result<()> {
    sqlx::query(
        "select pg_advisory_xact_lock(hashtextextended('plugin-family:' || $1 || ':' || $2 || ':' || $3, 0))",
    )
    .bind(category)
    .bind(organization)
    .bind(provider_code)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(crate) async fn lock_plugin_reference_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_id: Uuid,
) -> Result<(String, String)> {
    Ok(sqlx::query_as(
        "select desired_state, contract_version from extension_installations where id=$1 and plugin_id is not null for share",
    )
    .bind(installation_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ControlPlaneError::NotFound("plugin_installation"))?)
}

pub(crate) async fn lock_active_plugin_reference_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_id: Uuid,
) -> Result<()> {
    let (desired_state, _) = lock_plugin_reference_target(transaction, installation_id).await?;
    if desired_state == "disabled" {
        return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
    }
    Ok(())
}

pub(crate) async fn lock_assigned_active_plugin_reference_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    installation_id: Uuid,
) -> Result<()> {
    if !lock_active_plugin_workspace_assignment(transaction, workspace_id, installation_id).await? {
        return Err(ControlPlaneError::Conflict("plugin_assignment_required").into());
    }
    Ok(())
}

/// Lock the installation and any assignment in that order, returning whether the workspace may use it.
pub(crate) async fn lock_active_plugin_workspace_assignment(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    installation_id: Uuid,
) -> Result<bool> {
    let (desired_state, source_kind): (String, String) = sqlx::query_as(
        "select desired_state, source_kind from extension_installations where id=$1 and plugin_id is not null for share",
    )
    .bind(installation_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    if desired_state == "disabled" {
        return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
    }
    if source_kind == "builtin" {
        return Ok(true);
    }
    Ok(sqlx::query_scalar(
        "select exists(select 1 from plugin_assignments where workspace_id=$1 and installation_id=$2 for share)",
    )
    .bind(workspace_id)
    .bind(installation_id)
    .fetch_one(&mut **transaction)
    .await?)
}

async fn has_active_system_sibling_for_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_id: Uuid,
) -> Result<bool> {
    Ok(sqlx::query_scalar(
        r#"
        select exists(
            select 1
            from extension_installations target
            join extension_installations sibling
              on sibling.plugin_id is not null
             and sibling.id <> target.id
             and sibling.category = target.category
             and sibling.organization = target.organization
             and sibling.artifact_id = target.artifact_id
            where target.id = $1
              and sibling.desired_state = 'active_requested'
              and coalesce(
                  target.metadata_json #>> '{managed_service,scope}' = 'system'
                      or target.metadata_json -> 'binding_targets' = '["system"]'::jsonb,
                  false
              )
        )
        "#,
    )
    .bind(installation_id)
    .fetch_one(&mut **transaction)
    .await?)
}

pub(super) async fn ensure_assignment_target_assignable(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &CreatePluginAssignmentInput,
) -> Result<()> {
    let (desired_state, contract_version) =
        lock_plugin_reference_target(transaction, input.installation_id).await?;
    if desired_state != "disabled" {
        return Ok(());
    }
    if contract_version != "1flowbase.extension-bus/v1" {
        return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
    }
    // First-install workspace scope establishment may assign a disabled managed package only
    // before an active selection exists in the workspace, and never for a retired/unselected
    // system service that already has an active serving sibling.
    let has_active_workspace_selection: bool = sqlx::query_scalar(
        r#"
        select exists(
            select 1
            from plugin_assignments pa
            join extension_installations active_inst on active_inst.id = pa.installation_id
            where pa.workspace_id = $1
              and pa.provider_code = $2
              and active_inst.id <> $3
              and active_inst.desired_state = 'active_requested'
        )
        "#,
    )
    .bind(input.workspace_id)
    .bind(&input.provider_code)
    .bind(input.installation_id)
    .fetch_one(&mut **transaction)
    .await?;
    if has_active_workspace_selection
        || has_active_system_sibling_for_target(transaction, input.installation_id).await?
    {
        return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
    }
    Ok(())
}

pub(super) async fn should_skip_initial_assignment(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    input: &CreatePluginAssignmentInput,
) -> Result<bool> {
    let (desired_state, contract_version) =
        lock_plugin_reference_target(transaction, input.installation_id).await?;
    if desired_state != "disabled" {
        return Ok(false);
    }
    if contract_version != "1flowbase.extension-bus/v1" {
        return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
    }
    has_active_system_sibling_for_target(transaction, input.installation_id).await
}

async fn has_serving_sibling_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_id: Uuid,
    category: &str,
    organization: &str,
    provider_code: &str,
) -> Result<bool> {
    let siblings: Vec<(Uuid, bool)> = sqlx::query_as(
        r#"
        select
            id,
            contract_version = '1flowbase.extension-bus/v1'
                and coalesce(
                    metadata_json #>> '{managed_service,scope}' = 'system'
                        or metadata_json -> 'binding_targets' = '["system"]'::jsonb,
                    false
                )
        from extension_installations
        where plugin_id is not null
          and id <> $1
          and category = $2
          and organization = $3
          and artifact_id = $4
          and desired_state = 'active_requested'
        order by id
        "#,
    )
    .bind(installation_id)
    .bind(category)
    .bind(organization)
    .bind(provider_code)
    .fetch_all(&mut **transaction)
    .await?;
    for (sibling_id, system_scoped) in siblings {
        if system_scoped
            || !referenced_installation_ids(&mut **transaction, Some(sibling_id))
                .await?
                .is_empty()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

impl PgControlPlaneStore {
    pub(super) async fn retire_unreferenced_installation_impl(
        &self,
        input: &UpdatePluginDesiredStateInput,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        self.retire_unreferenced_installation_with_policy(input, true)
            .await
    }

    pub(super) async fn rollback_unreferenced_installation_impl(
        &self,
        input: &UpdatePluginDesiredStateInput,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        self.retire_unreferenced_installation_with_policy(input, false)
            .await
    }

    async fn retire_unreferenced_installation_with_policy(
        &self,
        input: &UpdatePluginDesiredStateInput,
        require_serving_sibling: bool,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        use domain::PluginDesiredState::Disabled;
        debug_assert_eq!(input.desired_state, Disabled);
        let Some((family_category, organization, provider_code)): Option<(String, String, String)> =
            sqlx::query_as(
                "select category, organization, artifact_id from extension_installations where id=$1 and plugin_id is not null",
            )
            .bind(input.installation_id)
            .fetch_optional(self.pool())
            .await?
        else {
            return Ok(None);
        };
        let mut transaction = self.pool().begin().await?;
        lock_plugin_family(
            &mut transaction,
            &family_category,
            &organization,
            &provider_code,
        )
        .await?;
        lock_managed_desired_state_scopes(&mut transaction, input.installation_id).await?;
        // FOR UPDATE conflicts with the FK's KEY SHARE even for reference writers that
        // don't take managed workspace locks. Check references in a subsequent statement.
        let locked: Option<(String, String, bool, String)> = sqlx::query_as(
            "select desired_state, category, is_system_reserved, source_kind from extension_installations where id=$1 and plugin_id is not null for update",
        )
        .bind(input.installation_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((desired_state, category, reserved, source_kind)) = locked else {
            return Ok(None);
        };
        if desired_state != "active_requested"
            || reserved
            || source_kind == "builtin"
            || category == domain::ExtensionCategory::HostExtensions.as_str()
            || !referenced_installation_ids(&mut *transaction, Some(input.installation_id))
                .await?
                .is_empty()
            || (require_serving_sibling
                && !has_serving_sibling_in_transaction(
                    &mut transaction,
                    input.installation_id,
                    &category,
                    &organization,
                    &provider_code,
                )
                .await?)
        {
            return Ok(None);
        }
        guard_managed_desired_state(&mut transaction, input.installation_id, Disabled).await?;
        let disabled_input = UpdatePluginDesiredStateInput {
            desired_state: Disabled,
            ..input.clone()
        };
        let disabled = write_plugin_desired_state(&mut transaction, &disabled_input).await?;
        let node_ids: Vec<String> = sqlx::query_scalar(
            "select node_id from extension_artifact_instances where installation_id = $1 order by node_id",
        )
        .bind(input.installation_id)
        .fetch_all(&mut *transaction)
        .await?;
        for node_id in node_ids {
            lock_node_artifact_slot(&mut transaction, &node_id, input.installation_id).await?;
        }
        sqlx::query(
            r#"
            update extension_artifact_instances
            set runtime_status = 'inactive', availability_status = 'disabled', checked_at = greatest(now(), checked_at + interval '1 microsecond')
            where installation_id = $1
              and (runtime_status <> 'inactive' or availability_status <> 'disabled')
            "#,
        )
        .bind(input.installation_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(disabled))
    }

    pub(super) async fn commit_provider_version_switch_impl(
        &self,
        input: &CommitProviderVersionSwitchInput,
    ) -> Result<ProviderVersionSwitchOutcome> {
        use domain::PluginDesiredState::Disabled;
        let (family_category, organization, provider_code): (String, String, String) =
            sqlx::query_as(
                "select category, organization, artifact_id from extension_installations where id=$1 and plugin_id is not null",
            )
            .bind(input.current_installation_id)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let mut transaction = self.pool().begin().await?;
        lock_plugin_family(
            &mut transaction,
            &family_category,
            &organization,
            &provider_code,
        )
        .await?;
        lock_managed_dependency_scope(&mut transaction).await?;
        crate::plugin_contribution_authority_repository::lock_managed_workspace(
            &mut transaction,
            input.workspace_id,
        )
        .await?;
        // Acquire installation locks BEFORE plugin_assignments to match reference-writer lock
        // order (extension_installations -> plugin_assignments) and prevent deadlocks.
        let current_id = input.current_installation_id;
        let target_id = input.target_installation_id;
        let mut locked_current: Option<(String, String, bool, String)> = None;
        let lock_current = || {
            sqlx::query_as(
                "select desired_state, category, is_system_reserved, source_kind from extension_installations where id=$1 and plugin_id is not null for update",
            )
            .bind(current_id)
        };
        if current_id < target_id {
            locked_current = lock_current().fetch_optional(&mut *transaction).await?;
        }
        let target = lock_artifact_installation(&mut transaction, target_id).await?;
        if target.desired_state == Disabled {
            return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
        }
        if current_id > target_id {
            locked_current = lock_current().fetch_optional(&mut *transaction).await?;
        }
        let current_assigned: Option<Uuid> = sqlx::query_scalar(
            "select id from plugin_assignments where workspace_id=$1 and provider_code=$2 and installation_id=$3 for update",
        )
        .bind(input.workspace_id)
        .bind(&input.provider_code)
        .bind(input.current_installation_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if current_assigned.is_none() {
            return Err(ControlPlaneError::Conflict("plugin_assignment_changed").into());
        }
        let first = input
            .current_installation_id
            .min(input.target_installation_id);
        let last = input
            .current_installation_id
            .max(input.target_installation_id);
        lock_node_artifact_slot(&mut transaction, &input.node_id, first).await?;
        if first != last {
            lock_node_artifact_slot(&mut transaction, &input.node_id, last).await?;
        }
        let artifacts = sqlx::query(
            r#"
            select node_id, installation_id, local_version, local_checksum, local_path,
                   package_path, manifest_fingerprint, artifact_status, runtime_status,
                   availability_status, checked_at, last_error, is_current
            from extension_artifact_instances
            where node_id = $1 and installation_id in ($2, $3)
            order by installation_id asc for update
            "#,
        )
        .bind(&input.node_id)
        .bind(input.current_installation_id)
        .bind(input.target_installation_id)
        .fetch_all(&mut *transaction)
        .await?;
        let artifacts = artifacts
            .into_iter()
            .map(super::map_artifact_instance)
            .collect::<Result<Vec<_>>>()?;
        let target_artifact = artifacts
            .iter()
            .find(|artifact| {
                artifact.installation_id == target_id && input.matches_active_artifact(artifact)
            })
            .ok_or(ControlPlaneError::PluginUnavailable)?;
        if !verify_locked_node_artifact_on_disk(&target, target_artifact, input.validate_manifest)
            .await
        {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        let migrated_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            update model_provider_instances
            set installation_id = $3, protocol = $4, updated_by = $5, updated_at = now()
            where workspace_id = $1 and provider_code = $2
            returning id
            "#,
        )
        .bind(input.workspace_id)
        .bind(&input.provider_code)
        .bind(input.target_installation_id)
        .bind(&input.target_protocol)
        .bind(input.actor_user_id)
        .fetch_all(&mut *transaction)
        .await?;
        for instance_id in &migrated_ids {
            sqlx::query(
                r#"
                insert into provider_instance_model_catalog_cache (
                    id, provider_instance_id, scope_id, model_discovery_mode,
                    refresh_status, source, models_json, last_error_message,
                    refreshed_at, created_by, updated_by
                )
                select $1, instances.id, instances.scope_id, $2, 'idle', $3, '[]'::jsonb, null, null, instances.updated_by, instances.updated_by
                from model_provider_instances instances
                where instances.id = $4
                on conflict (provider_instance_id) do update
                set scope_id = excluded.scope_id,
                    model_discovery_mode = excluded.model_discovery_mode,
                    refresh_status = excluded.refresh_status,
                    source = excluded.source,
                    models_json = excluded.models_json,
                    last_error_message = null,
                    refreshed_at = null,
                    updated_by = excluded.updated_by,
                    updated_at = now()
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(input.model_discovery_mode.as_str())
            .bind(input.catalog_source.as_str())
            .bind(instance_id)
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            r#"
            insert into plugin_assignments (id, installation_id, workspace_id, provider_code, assigned_by)
            values ($1, $2, $3, $4, $5)
            on conflict (workspace_id, provider_code) do update
            set installation_id = excluded.installation_id,
                assigned_by = excluded.assigned_by
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.target_installation_id)
        .bind(input.workspace_id)
        .bind(&input.provider_code)
        .bind(input.actor_user_id)
        .execute(&mut *transaction)
        .await?;
        let mut retired_previous = None;
        if input.current_installation_id != input.target_installation_id {
            if let Some((desired_state, category, reserved, source_kind)) = locked_current {
                if desired_state == "active_requested"
                    && !reserved
                    && source_kind != "builtin"
                    && category != domain::ExtensionCategory::HostExtensions.as_str()
                    && referenced_installation_ids(
                        &mut *transaction,
                        Some(input.current_installation_id),
                    )
                    .await?
                    .is_empty()
                    && guard_managed_desired_state(
                        &mut transaction,
                        input.current_installation_id,
                        Disabled,
                    )
                    .await
                    .is_ok()
                {
                    retired_previous = Some(
                        write_plugin_desired_state(
                            &mut transaction,
                            &UpdatePluginDesiredStateInput {
                                installation_id: input.current_installation_id,
                                desired_state: Disabled,
                                actor_user_id: input.actor_user_id,
                            },
                        )
                        .await?,
                    );
                }
            }
        }
        let retire_current = retired_previous.is_some();
        sqlx::query(
            r#"
            update extension_artifact_instances set
                is_current = (installation_id = $3),
                runtime_status = case
                    when $4 and installation_id = $2 then 'inactive'
                    else runtime_status
                end,
                availability_status = case
                    when $4 and installation_id = $2 then 'disabled'
                    else availability_status
                end,
                checked_at = greatest(now(), checked_at + interval '1 microsecond')
            where node_id = $1 and installation_id in ($2, $3)
            "#,
        )
        .bind(&input.node_id)
        .bind(input.current_installation_id)
        .bind(input.target_installation_id)
        .bind(retire_current)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(ProviderVersionSwitchOutcome {
            migrated_instance_count: migrated_ids.len(),
            retired_previous,
        })
    }
}
