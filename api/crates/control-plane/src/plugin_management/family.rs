use control_plane_contracts::ports::ActivatePluginArtifactOutcome;
pub(crate) fn node_installation_runtime_lock(
    node_id: &str,
    installation_id: Uuid,
) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    use std::{
        collections::HashMap,
        sync::{Arc, LazyLock, Mutex},
    };
    type RuntimeLifecycleLocks = HashMap<(String, Uuid), Arc<tokio::sync::Mutex<()>>>;
    static LOCKS: LazyLock<Mutex<RuntimeLifecycleLocks>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry((node_id.to_string(), installation_id))
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

use super::*;
use super::{
    catalog_projection::refresh_provider_package_catalog_projection,
    filesystem::{remove_path_if_exists, restore_artifact_removals, stage_artifact_removals},
    install::{
        is_system_managed_service, load_actor_context_for_user, map_catalog_source,
        map_model_discovery_mode,
    },
};

pub struct EnablePluginCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
}

pub struct DisablePluginCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
}

pub struct AssignPluginCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
}

pub struct SwitchPluginVersionCommand {
    pub actor_user_id: Uuid,
    pub provider_code: String,
    pub target_installation_id: Uuid,
}

pub struct DeletePluginFamilyCommand {
    pub actor_user_id: Uuid,
    pub provider_code: String,
}

// Local Host and this projection share the process lifetime. Allocate before
// querying so a late old snapshot cannot override a newer committed selection.
static WORKER_DEMAND_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository,
    H: ProviderRuntimePort,
{
    /// Completes durable tombstone cleanup without changing the already-committed uninstall fact.
    ///
    /// A failed delete stays as a durable cleanup record and is retried at boot and on a later
    /// uninstall request. It must never retroactively turn the business task into `Failed`.
    pub async fn reconcile_pending_artifact_removals(&self) -> Result<usize> {
        let cleanups = self
            .repository
            .list_plugin_artifact_cleanups(&self.node_id)
            .await?;
        let mut completed = 0usize;
        for cleanup in cleanups {
            match remove_path_if_exists(Path::new(&cleanup.tombstone_path)) {
                Ok(()) => match self
                    .repository
                    .complete_plugin_artifact_cleanup(cleanup.id)
                    .await
                {
                    Ok(()) => completed = completed.saturating_add(1),
                    Err(error) => tracing::warn!(
                        cleanup_id = %cleanup.id,
                        tombstone_path = %cleanup.tombstone_path,
                        error = %error,
                        "plugin artifact was removed but its cleanup acknowledgement will be retried"
                    ),
                },
                Err(error) => {
                    tracing::warn!(
                        cleanup_id = %cleanup.id,
                        tombstone_path = %cleanup.tombstone_path,
                        error = %error,
                        "plugin artifact cleanup remains pending"
                    );
                    if let Err(record_error) = self
                        .repository
                        .record_plugin_artifact_cleanup_failure(
                            &RecordPluginArtifactCleanupFailureInput {
                                cleanup_id: cleanup.id,
                                last_error: error.to_string(),
                                attempted_at: OffsetDateTime::now_utc(),
                            },
                        )
                        .await
                    {
                        tracing::warn!(
                            cleanup_id = %cleanup.id,
                            error = %record_error,
                            "failed to record pending plugin artifact cleanup failure"
                        );
                    }
                }
            }
        }
        Ok(completed)
    }

    pub(super) async fn transition_task(
        &self,
        task: &domain::PluginTaskRecord,
        next_status: domain::PluginTaskStatus,
        status_message: Option<String>,
        detail_json: serde_json::Value,
    ) -> Result<domain::PluginTaskRecord> {
        ensure_plugin_task_transition(task.status, next_status, "plugin_task_progress")?;
        self.repository
            .update_task_status(&UpdatePluginTaskStatusInput {
                task_id: task.id,
                status: next_status,
                status_message,
                detail_json,
            })
            .await
    }

    pub async fn enable_plugin(
        &self,
        command: EnablePluginCommand,
    ) -> Result<domain::PluginTaskRecord> {
        self.request_plugin_enable(command, domain::PluginTaskKind::Enable)
            .await
    }

    async fn request_plugin_enable(
        &self,
        command: EnablePluginCommand,
        task_kind: domain::PluginTaskKind,
    ) -> Result<domain::PluginTaskRecord> {
        self.request_plugin_enable_with_lock_mode(command, task_kind, false)
            .await
    }

    async fn request_plugin_enable_with_lock_mode(
        &self,
        command: EnablePluginCommand,
        task_kind: domain::PluginTaskKind,
        caller_holds_runtime_lock: bool,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&installation)?;
        if let Some(current) = self
            .active_system_managed_family_installation(&installation)
            .await?
        {
            if current.id != installation.id {
                // Enabling another version replaces the family owner through the existing
                // candidate publication transaction; never publish two active service owners.
                return Box::pin(self.switch_family_installation(
                    &actor,
                    &installation.provider_code,
                    &current,
                    &installation,
                    command.actor_user_id,
                    None,
                ))
                .await;
            }
        }
        let requires_restart = is_host_extension_installation(&installation);

        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: Some(command.installation_id),
                workspace_id: None,
                provider_code: installation.provider_code.clone(),
                task_kind,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".to_string()),
                detail_json: json!({}),
                actor_user_id: Some(command.actor_user_id),
            })
            .await?;
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".to_string()),
                json!({}),
            )
            .await?;

        let runtime_lock = node_installation_runtime_lock(&self.node_id, command.installation_id);
        let enable_result = async {
            let _lifecycle_guard = if caller_holds_runtime_lock {
                None
            } else {
                Some(runtime_lock.lock().await)
            };
            let was_active_on_node = !requires_restart
                && self.current_node_runtime_is_active(command.installation_id).await?;
            let local_installation = self
            .ready_current_node_installation(command.installation_id)
            .await?;
            self.prepare_managed_activation(&actor, &installation).await?;
            let updated = self
                .repository
                .update_desired_state(&UpdatePluginDesiredStateInput {
                    installation_id: command.installation_id,
                    desired_state: if requires_restart {
                        domain::PluginDesiredState::PendingRestart
                    } else {
                        domain::PluginDesiredState::ActiveRequested
                    },
                    actor_user_id: command.actor_user_id,
                })
                .await?;
            let runtime_installation = domain::LocalPluginInstallationRecord {
                installation: updated,
                artifact: local_installation.artifact,
            };
            let loaded = if requires_restart {
                runtime_installation.installation.clone()
            } else {
                match self.runtime.activate_plugin(&runtime_installation).await {
                    Ok(loaded) => {
                        let error = match self.try_mark_current_node_runtime_active(&runtime_installation, loaded.as_ref()).await? {
                            ActivatePluginArtifactOutcome::Activated(_) => None,
                            ActivatePluginArtifactOutcome::Disabled => Some(ControlPlaneError::Conflict("plugin_installation_disabled")),
                            ActivatePluginArtifactOutcome::ArtifactInvalidated => Some(ControlPlaneError::PluginUnavailable),
                        };
                        if let Some(error) = error {
                            self.deactivate_rejected_activation(&runtime_installation.installation, was_active_on_node).await?;
                            return Err(error.into());
                        }
                        runtime_installation.installation.clone()
                    }
                    Err(error) => {
                        let managed = installation.category == domain::ExtensionCategory::RuntimeExtensions
                            && installation.contract_version == "1flowbase.extension-bus/v1";
                        let mark_failure = if managed {
                            match self.repository.compare_restore_desired_state(
                                &control_plane_contracts::ports::CompareRestorePluginDesiredStateInput {
                                    installation_id: installation.id,
                                    expected_updated_at: runtime_installation.updated_at,
                                    expected_desired_state: runtime_installation.desired_state,
                                    restore_desired_state: installation.desired_state,
                                    actor_user_id: command.actor_user_id,
                                },
                            ).await {
                                Ok(restored) => restored,
                                Err(restore_error) => {
                                    tracing::error!(installation_id = %installation.id, %restore_error,
                                        "failed to restore rejected managed activation desired state");
                                    false
                                }
                            }
                        } else { true };
                        if mark_failure {
                            if let Err(status_error) = self.mark_current_node_runtime_status(
                                &runtime_installation,
                                domain::PluginRuntimeStatus::LoadFailed,
                                Some(error.to_string()),
                            ).await {
                                if !managed { return Err(status_error); }
                                tracing::warn!(installation_id = %installation.id, %status_error,
                                    "failed to record rejected managed activation status");
                            }
                        }
                        return Err(error);
                    }
                }
            };
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "plugin_installation",
                    Some(loaded.id),
                    "plugin.enabled",
                    json!({
                        "provider_code": loaded.provider_code,
                    }),
                ))
                .await?;
            Ok::<domain::PluginInstallationRecord, anyhow::Error>(loaded)
        }
        .await;

        match enable_result {
            Ok(updated) => {
                self.invalidate_model_routing_catalog(actor.current_workspace_id)
                    .await;
                self.transition_task(
                    &running_task,
                    domain::PluginTaskStatus::Succeeded,
                    Some(
                        if requires_restart {
                            "pending_restart"
                        } else {
                            "enabled"
                        }
                        .to_string(),
                    ),
                    json!({
                        "installation_id": updated.id,
                        "enabled": !matches!(
                            updated.desired_state,
                            domain::PluginDesiredState::Disabled
                        ),
                    }),
                )
                .await
            }
            Err(error) => {
                let _ = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Failed,
                        Some(error.to_string()),
                        json!({
                            "installation_id": command.installation_id,
                        }),
                    )
                    .await;
                Err(error)
            }
        }
    }

    pub async fn disable_plugin(
        &self,
        command: DisablePluginCommand,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&installation)?;
        if installation.contract_version == "1flowbase.extension-bus/v1"
            && installation.desired_state == domain::PluginDesiredState::ActiveRequested
        {
            super::dependencies::provider_change_preflight(
                &installation,
                None,
                &self.repository.list_installations().await?,
            )?;
        }
        let requires_restart = is_host_extension_installation(&installation);
        let _local_installation = self
            .ready_current_node_installation(command.installation_id)
            .await?;
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id: Uuid::now_v7(),
                installation_id: Some(command.installation_id),
                workspace_id: None,
                provider_code: installation.provider_code.clone(),
                task_kind: domain::PluginTaskKind::Disable,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".to_string()),
                detail_json: json!({}),
                actor_user_id: Some(command.actor_user_id),
            })
            .await?;
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".to_string()),
                json!({}),
            )
            .await?;
        let runtime_lock = node_installation_runtime_lock(&self.node_id, command.installation_id);
        let disabled = async {
            let _lifecycle_guard = runtime_lock.lock().await;
            let updated = self
                .repository
                .update_desired_state(&UpdatePluginDesiredStateInput {
                    installation_id: command.installation_id,
                    desired_state: domain::PluginDesiredState::Disabled,
                    actor_user_id: command.actor_user_id,
                })
                .await?;
            if !requires_restart {
                self.runtime.deactivate_plugin(&updated).await?;
                self.mark_current_node_runtime_status(
                    &updated,
                    domain::PluginRuntimeStatus::Inactive,
                    None,
                )
                .await?;
            }
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "plugin_installation",
                    Some(updated.id),
                    "plugin.disabled",
                    json!({ "provider_code": updated.provider_code }),
                ))
                .await?;
            Ok::<_, anyhow::Error>(updated)
        }
        .await;
        match disabled {
            Ok(updated) => {
                self.invalidate_model_routing_catalog(actor.current_workspace_id)
                    .await;
                self.transition_task(
                    &running_task,
                    domain::PluginTaskStatus::Succeeded,
                    Some("disabled".to_string()),
                    json!({ "installation_id": updated.id, "enabled": false }),
                )
                .await
            }
            Err(error) => {
                let _ = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Failed,
                        Some(error.to_string()),
                        json!({ "installation_id": command.installation_id }),
                    )
                    .await;
                Err(error)
            }
        }
    }

    pub async fn assign_plugin(
        &self,
        command: AssignPluginCommand,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&installation)?;
        if !supports_workspace_assignment(&installation) {
            return Err(ControlPlaneError::Conflict("plugin_assignment_not_supported").into());
        }
        // Managed installation scope must exist before its first host grant and activation.
        // Assignment establishes that scope; it does not start a worker or grant permissions.
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) && installation.contract_version != "1flowbase.extension-bus/v1"
        {
            return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
        }

        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: Some(command.installation_id),
                workspace_id: Some(actor.current_workspace_id),
                provider_code: installation.provider_code.clone(),
                task_kind: domain::PluginTaskKind::Assign,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".to_string()),
                detail_json: json!({}),
                actor_user_id: Some(command.actor_user_id),
            })
            .await?;
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".to_string()),
                json!({}),
            )
            .await?;

        let assign_result = async {
            self.repository
                .create_assignment(&CreatePluginAssignmentInput {
                    installation_id: command.installation_id,
                    workspace_id: actor.current_workspace_id,
                    provider_code: installation.provider_code.clone(),
                    actor_user_id: command.actor_user_id,
                })
                .await?;
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "plugin_assignment",
                    Some(command.installation_id),
                    "plugin.assigned",
                    json!({
                        "provider_code": installation.provider_code,
                    }),
                ))
                .await?;
            Ok::<(), anyhow::Error>(())
        }
        .await;

        match assign_result {
            Ok(()) => {
                self.invalidate_model_routing_catalog(actor.current_workspace_id)
                    .await;
                self.transition_task(
                    &running_task,
                    domain::PluginTaskStatus::Succeeded,
                    Some("assigned".to_string()),
                    json!({
                        "installation_id": command.installation_id,
                        "workspace_id": actor.current_workspace_id,
                    }),
                )
                .await
            }
            Err(error) => {
                let _ = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Failed,
                        Some(error.to_string()),
                        json!({
                            "installation_id": command.installation_id,
                            "workspace_id": actor.current_workspace_id,
                        }),
                    )
                    .await;
                Err(error)
            }
        }
    }

    pub async fn switch_version(
        &self,
        command: SwitchPluginVersionCommand,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let native_target = self
            .repository
            .get_installation(command.target_installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&native_target)?;
        if is_host_extension_installation(&native_target) {
            if native_target.provider_code != command.provider_code {
                return Err(
                    ControlPlaneError::InvalidInput("plugin_family_target_mismatch").into(),
                );
            }
            return self
                .request_plugin_enable(
                    EnablePluginCommand {
                        actor_user_id: command.actor_user_id,
                        installation_id: native_target.id,
                    },
                    domain::PluginTaskKind::SwitchVersion,
                )
                .await;
        }
        let current = self
            .load_current_family_installation(actor.current_workspace_id, &command.provider_code)
            .await?;
        self.ensure_model_provider_target(&current)?;
        let target = self
            .repository
            .get_installation(command.target_installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&target)?;
        if target.provider_code != command.provider_code {
            return Err(ControlPlaneError::InvalidInput("plugin_family_target_mismatch").into());
        }
        if current.id == target.id {
            return Err(ControlPlaneError::Conflict("plugin_version_already_current").into());
        }

        self.switch_family_installation(
            &actor,
            &command.provider_code,
            &current,
            &target,
            command.actor_user_id,
            None,
        )
        .await
    }

    pub async fn list_tasks(&self, actor_user_id: Uuid) -> Result<Vec<domain::PluginTaskRecord>> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.view.all")
            .await?;
        self.repository.list_tasks().await
    }

    pub async fn get_task(
        &self,
        actor_user_id: Uuid,
        task_id: Uuid,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.view.all")
            .await?;
        let task = self
            .repository
            .get_task(task_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_task"))?;
        if self.is_model_provider_console_operation() {
            let owns_task_scope = match task.workspace_id {
                Some(workspace_id) => workspace_id == actor.current_workspace_id,
                None => task.created_by == Some(actor.user_id),
            };
            if !actor.is_root && !owns_task_scope {
                return Err(ControlPlaneError::PermissionDenied("plugin_task_scope").into());
            }
            let installation = match task.installation_id {
                Some(installation_id) => self.repository.get_installation(installation_id).await?,
                None => self
                    .repository
                    .list_installations()
                    .await?
                    .into_iter()
                    .find(|installation| installation.provider_code == task.provider_code),
            }
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
            self.ensure_model_provider_target(&installation)?;
        }
        Ok(task)
    }

    pub(super) async fn active_system_managed_family_installation(
        &self,
        target: &domain::PluginInstallationRecord,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        if target.contract_version != "1flowbase.extension-bus/v1"
            || is_host_extension_installation(target)
            || !is_system_managed_service(target)
        {
            return Ok(None);
        }
        let mut active = self
            .repository
            .list_installations()
            .await?
            .into_iter()
            .filter(|candidate| {
                candidate.category == target.category
                    && candidate.organization == target.organization
                    && candidate.provider_code == target.provider_code
                    && candidate.contract_version == target.contract_version
                    && is_system_managed_service(candidate)
                    && candidate.desired_state == domain::PluginDesiredState::ActiveRequested
            });
        let current = active.next();
        if active.next().is_some() {
            return Err(
                ControlPlaneError::Conflict("managed_active_installation_ambiguous").into(),
            );
        }
        Ok(current)
    }

    pub(super) async fn load_current_family_installation(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
    ) -> Result<domain::PluginInstallationRecord> {
        let mut family = self.repository.list_installations().await?;
        family.retain(|installation| installation.provider_code == provider_code);
        if family.iter().any(is_host_extension_installation) {
            let targets = self.repository.list_native_plugin_targets().await?;
            let mut matches = targets
                .iter()
                .filter(|target| target.artifact_id == provider_code);
            let (Some(target), None) = (matches.next(), matches.next()) else {
                return Err(ControlPlaneError::Conflict("native_plugin_selection_required").into());
            };
            return family
                .into_iter()
                .find(|installation| {
                    is_host_extension_installation(installation)
                        && installation.id == target.installation_id
                })
                .ok_or_else(|| ControlPlaneError::NotFound("native_plugin_installation").into());
        }
        {
            // End iterator borrows before the workspace repository awaits.
            let mut system_managed = family
                .iter()
                .filter(|installation| {
                    domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
                        == domain::SYSTEM_SCOPE_ID
                })
                .peekable();
            if system_managed.peek().is_some() {
                let mut active = system_managed.filter(|installation| {
                    installation.desired_state == domain::PluginDesiredState::ActiveRequested
                });
                let current = active
                    .next()
                    .ok_or(ControlPlaneError::NotFound("managed_active_installation"))?;
                if active.next().is_some() {
                    return Err(ControlPlaneError::Conflict(
                        "managed_active_installation_ambiguous",
                    )
                    .into());
                }
                return Ok(current.clone());
            }
        }
        let assignment = self
            .repository
            .list_assignments(workspace_id)
            .await?
            .into_iter()
            .find(|item| item.provider_code == provider_code)
            .ok_or(ControlPlaneError::NotFound("plugin_assignment"))?;

        self.repository
            .get_installation(assignment.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation").into())
    }

    async fn reconcile_family_worker_demand(
        &self,
        previous: &domain::PluginInstallationRecord,
        target: &domain::PluginInstallationRecord,
    ) {
        let Ok(revision) = WORKER_DEMAND_REVISION.try_update(
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
            |value| value.checked_add(1),
        ) else {
            return;
        };
        // These repositories query all workspaces, never just the upgraded one.
        let installations = self.repository.list_installations().await;
        let assigned = self.repository.list_assigned_installation_ids().await;
        let known = installations.is_ok() && assigned.is_ok();
        let mut family = installations.unwrap_or_else(|_| vec![previous.clone(), target.clone()]);
        family.retain(|installation| {
            installation.organization == previous.organization
                && installation.provider_code == previous.provider_code
                && installation.contract_version == CURRENT_PROVIDER_CONTRACT
        });
        let assigned = assigned.unwrap_or_default();
        for installation in family {
            let selectable = known.then(|| assigned.contains(&installation.id));
            if let Err(error) = self
                .runtime
                .reconcile_provider_worker_demand(&installation, revision, selectable)
                .await
            {
                // The durable switch is already committed. The regular idle
                // policy still reclaims safely if this optimization is unavailable.
                tracing::warn!(installation_id = %installation.id, %error, "provider worker demand notification failed");
            }
        }
    }

    /// Unload a retired version only while it has not been concurrently re-enabled by a rollback.
    pub async fn deactivate_retired_installation_if_still_disabled(
        &self,
        installation_id: Uuid,
    ) -> Result<bool> {
        let runtime_lock = node_installation_runtime_lock(&self.node_id, installation_id);
        let _lifecycle_guard = runtime_lock.lock().await;
        let Some(latest) = self.repository.get_installation(installation_id).await? else {
            return Ok(false);
        };
        if latest.desired_state != domain::PluginDesiredState::Disabled {
            return Ok(false);
        }
        self.runtime.deactivate_plugin(&latest).await?;
        self.mark_current_node_runtime_status(&latest, domain::PluginRuntimeStatus::Inactive, None)
            .await?;
        Ok(true)
    }

    pub(super) async fn switch_family_installation(
        &self,
        actor: &domain::ActorContext,
        provider_code: &str,
        current: &domain::PluginInstallationRecord,
        target: &domain::PluginInstallationRecord,
        actor_user_id: Uuid,
        compatibility_override: Option<&serde_json::Value>,
    ) -> Result<domain::PluginTaskRecord> {
        if is_host_extension_installation(current) {
            if target.category != current.category
                || target.scope_id != current.scope_id
                || target.organization != current.organization
                || target.provider_code != current.provider_code
            {
                return Err(
                    ControlPlaneError::InvalidInput("plugin_family_target_mismatch").into(),
                );
            }
            return self
                .request_plugin_enable(
                    EnablePluginCommand {
                        actor_user_id,
                        installation_id: target.id,
                    },
                    domain::PluginTaskKind::SwitchVersion,
                )
                .await;
        }
        let execution_scope =
            domain::managed_installation_scope(current, actor.current_workspace_id);
        if current.contract_version == "1flowbase.extension-bus/v1"
            && (domain::managed_installation_scope(target, actor.current_workspace_id)
                != execution_scope
                || target.contract_version != current.contract_version
                || target.organization != current.organization
                || target.provider_code != current.provider_code)
        {
            return Err(ControlPlaneError::InvalidInput("plugin_family_target_mismatch").into());
        }
        if current.contract_version == "1flowbase.extension-bus/v1" {
            super::dependencies::provider_change_preflight(
                current,
                Some(target),
                &self.repository.list_installations().await?,
            )?;
        }
        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: Some(target.id),
                workspace_id: (execution_scope != domain::SYSTEM_SCOPE_ID)
                    .then_some(execution_scope),
                provider_code: provider_code.to_string(),
                task_kind: domain::PluginTaskKind::SwitchVersion,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".into()),
                detail_json: json!({}),
                actor_user_id: Some(actor_user_id),
            })
            .await?;
        let mut running_detail_json = json!({
            "provider_code": provider_code,
            "previous_installation_id": current.id,
            "previous_version": current.plugin_version,
            "target_installation_id": target.id,
            "target_version": target.plugin_version,
        });
        if let Some(compatibility_override) = compatibility_override.cloned() {
            running_detail_json["compatibility_override"] = compatibility_override;
        }
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".into()),
                running_detail_json,
            )
            .await?;

        if current.contract_version == "1flowbase.extension-bus/v1" {
            if let Err(error) = self.prepare_managed_activation(actor, target).await {
                self.transition_task(
                    &running_task,
                    domain::PluginTaskStatus::Failed,
                    Some(error.to_string()),
                    running_task.detail_json.clone(),
                )
                .await?;
                return Err(error);
            }
            return self.runtime.switch_managed_installation(execution_scope, current.id, target.id,
                audit_log((execution_scope != domain::SYSTEM_SCOPE_ID).then_some(execution_scope), Some(actor_user_id), if execution_scope == domain::SYSTEM_SCOPE_ID { "plugin_installation" } else { "plugin_assignment" }, Some(target.id), "plugin.version_switched",
                    json!({"previous_installation_id":current.id,"target_installation_id":target.id,"provider_code":provider_code})), running_task).await;
        }
        let switch_result = async {
            let (first_id, second_id) = if current.id < target.id {
                (current.id, target.id)
            } else {
                (target.id, current.id)
            };
            let first_lock = node_installation_runtime_lock(&self.node_id, first_id);
            let second_lock = node_installation_runtime_lock(&self.node_id, second_id);
            let _first_lifecycle_guard = first_lock.lock().await;
            let _second_lifecycle_guard = second_lock.lock().await;
            let live_target = self.repository.get_installation(target.id).await?
                .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
            let target_was_active_on_node = self.current_node_runtime_is_active(live_target.id).await?;
            let mut activation_attempted = false;
            let commit = async {
                let local_artifact = self.refresh_current_node_artifact_snapshot(&live_target).await?;
                if !local_artifact.artifact_status.is_ready() {
                    self
                        .install_current_node_artifact_locked(InstallCurrentNodePluginArtifactCommand {
                            actor_user_id,
                            installation_id: live_target.id,
                        })
                        .await?;
                }
                if live_target.desired_state == domain::PluginDesiredState::Disabled {
                    self.request_plugin_enable_with_lock_mode(
                        EnablePluginCommand {
                            actor_user_id,
                            installation_id: live_target.id,
                        },
                        domain::PluginTaskKind::Enable,
                        true,
                    )
                    .await?;
                }
                let local_target = self.ready_current_node_installation(live_target.id).await?;
                let package =
                    crate::installed_provider_package::load_installed_provider_package(&local_target)?;
                refresh_provider_package_catalog_projection(&self.repository, &local_target, &package)
                    .await?;
                activation_attempted = true;
                let loaded = self.runtime.activate_plugin(&local_target).await?;
                let verified_artifact = match self.try_mark_current_node_runtime_active(&local_target, loaded.as_ref()).await? {
                    ActivatePluginArtifactOutcome::Activated(artifact) => artifact,
                    rejected => {
                        return Err(match rejected {
                            ActivatePluginArtifactOutcome::Disabled => ControlPlaneError::Conflict("plugin_installation_disabled"),
                            _ => ControlPlaneError::PluginUnavailable,
                        }.into());
                    }
                };
                let outcome = self
                    .repository
                    .commit_provider_version_switch(
                        &control_plane_contracts::ports::CommitProviderVersionSwitchInput {
                            validate_manifest: super::validate_installed_artifact_manifest,
                            node_id: self.node_id.clone(),
                            workspace_id: actor.current_workspace_id,
                            provider_code: provider_code.to_string(),
                            current_installation_id: current.id,
                            target_installation_id: local_target.id,
                            target_local_version: verified_artifact.local_version,
                            target_local_checksum: verified_artifact.local_checksum,
                            target_local_path: verified_artifact.local_path.ok_or(ControlPlaneError::PluginUnavailable)?,
                            target_manifest_fingerprint: verified_artifact.manifest_fingerprint,
                            target_protocol: local_target.protocol.clone(),
                            model_discovery_mode: map_model_discovery_mode(
                                package.provider.model_discovery_mode,
                            ),
                            catalog_source: map_catalog_source(package.provider.model_discovery_mode),
                            actor_user_id,
                        },
                    )
                    .await?;
                Ok::<_, anyhow::Error>((local_target, outcome))
            }.await;
            let (local_target, switch_outcome) = match commit {
                Ok(committed) => committed,
                Err(error) => {
                    let cleanup = async {
                        if let Some(retired_target) = self.repository.rollback_unreferenced_installation(&UpdatePluginDesiredStateInput {
                            installation_id: live_target.id,
                            desired_state: domain::PluginDesiredState::Disabled,
                            actor_user_id,
                        }).await? {
                            self.runtime.deactivate_plugin(&retired_target).await?;
                            self.mark_current_node_runtime_status(
                                &retired_target,
                                domain::PluginRuntimeStatus::Inactive,
                                None,
                            ).await?;
                        } else if activation_attempted && !self.current_node_runtime_is_active(live_target.id).await? {
                            self.deactivate_rejected_activation(&live_target, target_was_active_on_node).await?;
                        }
                        Ok::<(), anyhow::Error>(())
                    }.await;
                    if let Err(cleanup_error) = cleanup {
                        tracing::error!(installation_id = %live_target.id, %cleanup_error, "failed to roll back unreferenced target after version switch failure");
                    }
                    return Err(error);
                }
            };
            let migrated_instance_count = switch_outcome.migrated_instance_count;
            if let Some(disabled_current) = switch_outcome.retired_previous {
                self.runtime.deactivate_plugin(&disabled_current).await?;
            }
            let mut switch_audit_detail = json!({
                "provider_code": provider_code,
                "previous_installation_id": current.id,
                "previous_version": current.plugin_version,
                "target_installation_id": local_target.id,
                "target_version": local_target.plugin_version,
            });
            if let Some(compatibility_override) = compatibility_override.cloned() {
                switch_audit_detail["compatibility_override"] = compatibility_override;
            }
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor_user_id),
                    "plugin_assignment",
                    Some(target.id),
                    "plugin.version_switched",
                    switch_audit_detail,
                ))
                .await?;
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor_user_id),
                    "model_provider_instance",
                    None,
                    "provider.instances_migrated_after_plugin_switch",
                    json!({
                        "provider_code": provider_code,
                        "migrated_instance_count": migrated_instance_count,
                    }),
                ))
                .await?;
            self.invalidate_model_routing_catalog(actor.current_workspace_id)
                .await;
            self.reconcile_family_worker_demand(current, target).await;
            Ok::<usize, anyhow::Error>(migrated_instance_count)
        }
        .await;

        match switch_result {
            Ok(migrated_instance_count) => {
                let mut success_detail_json = json!({
                    "provider_code": provider_code,
                    "previous_installation_id": current.id,
                    "previous_version": current.plugin_version,
                    "target_installation_id": target.id,
                    "target_version": target.plugin_version,
                    "migrated_instance_count": migrated_instance_count,
                });
                if let Some(compatibility_override) = compatibility_override.cloned() {
                    success_detail_json["compatibility_override"] = compatibility_override;
                }
                self.transition_task(
                    &running_task,
                    domain::PluginTaskStatus::Succeeded,
                    Some("switched".into()),
                    success_detail_json,
                )
                .await
            }
            Err(error) => {
                let _ = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Failed,
                        Some(error.to_string()),
                        json!({
                            "provider_code": provider_code,
                            "previous_installation_id": current.id,
                            "target_installation_id": target.id,
                        }),
                    )
                    .await;
                Err(error)
            }
        }
    }
}

pub(super) fn supports_workspace_assignment(
    installation: &domain::PluginInstallationRecord,
) -> bool {
    // A mixed system service may expose declarative workspace UI, but assignment
    // remains explicit and frontend resolution separately verifies its exact grant.
    let workspace_ui = control_plane_contracts::ports::has_managed_workspace_ui(installation);
    (domain::managed_installation_scope(installation, domain::DEFAULT_SCOPE_ID)
        != domain::SYSTEM_SCOPE_ID
        || workspace_ui)
        && matches!(
            installation.contract_version.as_str(),
            CURRENT_PROVIDER_CONTRACT
                | "1flowbase.data_source/v1"
                | "1flowbase.capability/v1"
                | "1flowbase.extension-bus/v1"
        )
}

#[path = "family/family_removal.rs"]
mod family_removal;
