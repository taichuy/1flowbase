use super::*;
use super::{
    catalog_projection::refresh_provider_package_catalog_projection,
    filesystem::remove_path_if_exists,
    install::{load_actor_context_for_user, map_catalog_source, map_model_discovery_mode},
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

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository,
    H: ProviderRuntimePort,
{
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
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&installation)?;
        let requires_restart = is_host_extension_installation(&installation);
        let local_installation = self
            .ready_current_node_installation(command.installation_id)
            .await?;

        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: Some(command.installation_id),
                workspace_id: None,
                provider_code: installation.provider_code.clone(),
                task_kind: domain::PluginTaskKind::Enable,
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

        let enable_result = async {
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
                    Ok(()) => {
                        self.mark_current_node_runtime_status(
                            &runtime_installation,
                            domain::PluginRuntimeStatus::Active,
                            None,
                        )
                        .await?;
                        runtime_installation.installation.clone()
                    }
                    Err(error) => {
                        self.mark_current_node_runtime_status(
                            &runtime_installation,
                            domain::PluginRuntimeStatus::LoadFailed,
                            Some(error.to_string()),
                        )
                        .await?;
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
                    Some("enabled".to_string()),
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
        let disabled = async {
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
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) {
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

    pub async fn delete_family(
        &self,
        command: DeletePluginFamilyCommand,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let installations = self
            .repository
            .list_installations()
            .await?
            .into_iter()
            .filter(|installation| installation.provider_code == command.provider_code)
            .collect::<Vec<_>>();
        if installations.is_empty() {
            return Err(ControlPlaneError::NotFound("plugin_family").into());
        }
        for installation in &installations {
            self.ensure_model_provider_target(installation)?;
        }
        let current_installation_id = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?
            .into_iter()
            .find(|assignment| assignment.provider_code == command.provider_code)
            .map(|assignment| assignment.installation_id)
            .or_else(|| installations.first().map(|installation| installation.id));

        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: current_installation_id,
                workspace_id: Some(actor.current_workspace_id),
                provider_code: command.provider_code.clone(),
                task_kind: domain::PluginTaskKind::Uninstall,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".into()),
                detail_json: json!({}),
                actor_user_id: Some(command.actor_user_id),
            })
            .await?;
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".into()),
                json!({
                    "provider_code": command.provider_code,
                    "installation_ids": installations
                        .iter()
                        .map(|installation| installation.id)
                        .collect::<Vec<_>>(),
                }),
            )
            .await?;

        let uninstall_result = async {
            let instances = self
                .repository
                .list_instances_by_provider_code(&command.provider_code)
                .await?;
            // Runtime scopes own their resources. Dispose all of them before removing any
            // artifact, so an unloaded family cannot retain a callable contribution.
            for installation in &installations {
                self.runtime.deactivate_plugin(installation).await?;
            }

            let mut artifacts = Vec::with_capacity(installations.len());
            let mut removed_paths = HashSet::<PathBuf>::new();
            for installation in &installations {
                let artifact = self
                    .repository
                    .get_artifact_instance(&self.node_id, installation.id)
                    .await?;
                if let Some(artifact) = &artifact {
                    if let Some(local_path) = &artifact.local_path {
                        removed_paths.insert(PathBuf::from(local_path));
                    }
                    if let Some(package_path) = &artifact.package_path {
                        removed_paths.insert(PathBuf::from(package_path));
                    }
                }
                artifacts.push((installation, artifact));
            }
            for path in &removed_paths {
                remove_path_if_exists(path)?;
            }

            for (installation, artifact) in artifacts {
                let artifact = artifact.as_ref();
                self.repository
                    .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
                        node_id: self.node_id.clone(),
                        installation_id: installation.id,
                        local_version: artifact
                            .and_then(|item| item.local_version.clone())
                            .or_else(|| Some(installation.plugin_version.clone())),
                        local_checksum: artifact
                            .and_then(|item| item.local_checksum.clone())
                            .or_else(|| installation.expected_checksum.clone()),
                        local_path: None,
                        package_path: None,
                        manifest_fingerprint: artifact
                            .and_then(|item| item.manifest_fingerprint.clone()),
                        artifact_status: domain::PluginArtifactInstanceStatus::Missing,
                        runtime_status: domain::PluginRuntimeStatus::Inactive,
                        availability_status: domain::PluginAvailabilityStatus::ArtifactMissing,
                        checked_at: OffsetDateTime::now_utc(),
                        last_error: Some("artifact_missing".to_string()),
                        // A family is unavailable as a unit; a sibling version must not win.
                        is_current: false,
                    })
                    .await?;
            }

            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "plugin_family",
                    None,
                    "plugin.family_uninstalled",
                    json!({
                        "provider_code": command.provider_code,
                        "retained_instance_count": instances.len(),
                        "retained_installation_count": installations.len(),
                    }),
                ))
                .await?;

            Ok::<(usize, usize), anyhow::Error>((instances.len(), installations.len()))
        }
        .await;

        match uninstall_result {
            Ok((retained_instance_count, retained_installation_count)) => {
                self.invalidate_model_routing_catalog(actor.current_workspace_id)
                    .await;
                self.transition_task(
                    &running_task,
                    domain::PluginTaskStatus::Succeeded,
                    Some("uninstalled".into()),
                    json!({
                        "provider_code": command.provider_code,
                        "retained_instance_count": retained_instance_count,
                        "retained_installation_count": retained_installation_count,
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
                            "provider_code": command.provider_code,
                        }),
                    )
                    .await;
                Err(error)
            }
        }
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

    pub(super) async fn load_current_family_installation(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
    ) -> Result<domain::PluginInstallationRecord> {
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

    pub(super) async fn switch_family_installation(
        &self,
        actor: &domain::ActorContext,
        provider_code: &str,
        current: &domain::PluginInstallationRecord,
        target: &domain::PluginInstallationRecord,
        actor_user_id: Uuid,
        compatibility_override: Option<&serde_json::Value>,
    ) -> Result<domain::PluginTaskRecord> {
        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: Some(target.id),
                workspace_id: Some(actor.current_workspace_id),
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

        let switch_result = async {
            let mut local_artifact = self.refresh_current_node_artifact_snapshot(target).await?;
            if !local_artifact.artifact_status.is_ready() {
                local_artifact = self
                    .install_current_node_artifact(InstallCurrentNodePluginArtifactCommand {
                        actor_user_id,
                        installation_id: target.id,
                    })
                    .await?;
            }
            if matches!(target.desired_state, domain::PluginDesiredState::Disabled)
                || local_artifact.runtime_status != domain::PluginRuntimeStatus::Active
            {
                self.enable_plugin(EnablePluginCommand {
                    actor_user_id,
                    installation_id: target.id,
                })
                .await?;
            }
            let local_target = self.ready_current_node_installation(target.id).await?;
            let package =
                crate::installed_provider_package::load_installed_provider_package(&local_target)?;
            refresh_provider_package_catalog_projection(&self.repository, &local_target, &package)
                .await?;
            let migrated_instances = self
                .repository
                .reassign_instances_to_installation(&ReassignModelProviderInstancesInput {
                    workspace_id: actor.current_workspace_id,
                    provider_code: provider_code.to_string(),
                    target_installation_id: local_target.id,
                    target_protocol: local_target.protocol.clone(),
                    updated_by: actor_user_id,
                })
                .await?;

            for instance in &migrated_instances {
                self.repository
                    .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                        provider_instance_id: instance.id,
                        model_discovery_mode: map_model_discovery_mode(
                            package.provider.model_discovery_mode,
                        ),
                        refresh_status: domain::ModelProviderCatalogRefreshStatus::Idle,
                        source: map_catalog_source(package.provider.model_discovery_mode),
                        models_json: json!([]),
                        last_error_message: None,
                        refreshed_at: None,
                    })
                    .await?;
            }
            self.repository
                .create_assignment(&CreatePluginAssignmentInput {
                    installation_id: local_target.id,
                    workspace_id: actor.current_workspace_id,
                    provider_code: provider_code.to_string(),
                    actor_user_id,
                })
                .await?;
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
                        "migrated_instance_count": migrated_instances.len(),
                    }),
                ))
                .await?;
            self.invalidate_model_routing_catalog(actor.current_workspace_id)
                .await;
            Ok::<usize, anyhow::Error>(migrated_instances.len())
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
    matches!(
        installation.contract_version.as_str(),
        CURRENT_PROVIDER_CONTRACT | "1flowbase.data_source/v1" | "1flowbase.capability/v1"
    )
}
