use super::*;

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository,
    H: ProviderRuntimePort,
{
    pub async fn delete_family(
        &self,
        command: DeletePluginFamilyCommand,
    ) -> Result<domain::PluginTaskRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        if let Err(error) = self.reconcile_pending_artifact_removals().await {
            tracing::warn!(
                error = %error,
                "plugin artifact cleanup reconciliation was unavailable before family uninstall"
            );
        }

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
            if !matches!(
                installation.category,
                domain::ExtensionCategory::RuntimeExtensions
                    | domain::ExtensionCategory::CapabilityPlugins
            ) {
                return Err(
                    ControlPlaneError::Conflict("plugin_runtime_uninstall_unsupported").into(),
                );
            }
        }
        let managed_ids = installations
            .iter()
            .filter(|installation| installation.contract_version == "1flowbase.extension-bus/v1")
            .map(|installation| installation.id)
            .collect::<Vec<_>>();
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
            // Disable through the existing state/audit owner before taking assembly. A real
            // frozen invocation or durable delivery may still reject retirement; leave the
            // installation disabled so a later retry can complete safely without a restart.
            let assigned_installations = if managed_ids.is_empty() {
                Vec::new()
            } else {
                self.repository.list_assigned_installation_ids().await?
            };
            for installation in &installations {
                if installation.contract_version != "1flowbase.extension-bus/v1" {
                    continue;
                }
                if installation.desired_state != domain::PluginDesiredState::Disabled {
                    self.disable_plugin(DisablePluginCommand {
                        actor_user_id: command.actor_user_id,
                        installation_id: installation.id,
                    }).await?;
                } else if domain::managed_installation_scope(installation, actor.current_workspace_id)
                    == domain::SYSTEM_SCOPE_ID
                    || assigned_installations.contains(&installation.id)
                {
                    // Disabled commits before runtime publication. An interrupted disable can
                    // therefore leave a current graph; reconcile it on every uninstall retry.
                    // Unlike disable_plugin, this does not require an available local artifact.
                    // Never-assigned workspace siblings have no graph scope to rebuild.
                    self.runtime.deactivate_plugin(installation).await?;
                }
            }
            let _managed_removal = if managed_ids.is_empty() {
                None
            } else {
                Some(
                    self.runtime
                        .prepare_managed_artifact_removal(&managed_ids)
                        .await?,
                )
            };
            let instances = self
                .repository
                .list_instances_by_provider_code(&command.provider_code)
                .await?;
            let mut artifacts = Vec::with_capacity(installations.len());
            let mut artifact_paths = Vec::with_capacity(installations.len() * 2);
            for installation in &installations {
                let artifact = self
                    .repository
                    .get_artifact_instance(&self.node_id, installation.id)
                    .await?;
                if let Some(artifact) = &artifact {
                    if let Some(local_path) = &artifact.local_path {
                        artifact_paths.push(PathBuf::from(local_path));
                    }
                    if let Some(package_path) = &artifact.package_path {
                        artifact_paths.push(PathBuf::from(package_path));
                    }
                }
                artifacts.push((installation.clone(), artifact));
            }
            let runtime_restore = artifacts
                .iter()
                .filter_map(|(installation, artifact)| {
                    if installation.contract_version == "1flowbase.extension-bus/v1" { return None; }
                    artifact
                        .as_ref()
                        .filter(|artifact| {
                            artifact.runtime_status == domain::PluginRuntimeStatus::Active
                        })
                        .cloned()
                        .map(|artifact| domain::LocalPluginInstallationRecord {
                            installation: installation.clone(),
                            artifact,
                        })
                })
                .collect::<Vec<_>>();
            // Runtime scopes own their resources. Dispose all of them before staging any
            // artifact, so an unloaded family cannot retain a callable contribution.
            for installation in &installations {
                // Managed artifacts already have no runtime references; the removal guard owns
                // assembly until DB completion. Re-entering rebuild here would deadlock it.
                if installation.contract_version != "1flowbase.extension-bus/v1" { self.runtime.deactivate_plugin(installation).await?; }
            }
            let mut removals = match stage_artifact_removals(artifact_paths) {
                Ok(removals) => removals,
                Err(error) => {
                    self.restore_runtime_scopes_after_uninstall_rejection(&runtime_restore)
                        .await
                        .context(
                            "failed to restore runtime scopes after artifact staging rejection",
                        )?;
                    return Err(error);
                }
            };
            let checked_at = OffsetDateTime::now_utc();
            let artifact_instances = artifacts
                .iter()
                .map(|(installation, artifact)| {
                    let artifact = artifact.as_ref();
                    UpsertPluginArtifactInstanceInput {
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
                        checked_at,
                        last_error: Some("artifact_missing".to_string()),
                        // A family is unavailable as a unit; a sibling version must not win.
                        is_current: false,
                    }
                })
                .collect::<Vec<_>>();
            let artifact_cleanups = removals
                .iter()
                .filter_map(|removal| {
                    removal.tombstone_path().map(|tombstone_path| {
                        CreatePluginArtifactCleanupInput {
                            cleanup_id: Uuid::now_v7(),
                            node_id: self.node_id.clone(),
                            provider_code: command.provider_code.clone(),
                            tombstone_path: tombstone_path.display().to_string(),
                            created_at: checked_at,
                        }
                    })
                })
                .collect::<Vec<_>>();
            let durable_commit = self
                .repository
                .commit_plugin_family_uninstall(&CommitPluginFamilyUninstallInput {
                    artifact_instances,
                    artifact_cleanups: artifact_cleanups.clone(),
                    audit_log: audit_log(
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
                    ),
                })
                .await;
            if let Err(error) = durable_commit {
                restore_artifact_removals(&mut removals)
                    .context("failed to restore plugin artifacts after durable commit rejection")?;
                self.restore_runtime_scopes_after_uninstall_rejection(&runtime_restore)
                    .await
                    .context("failed to restore runtime scopes after durable commit rejection")?;
                return Err(error);
            }
            let mut cleanup_by_tombstone = artifact_cleanups
                .into_iter()
                .map(|cleanup| (cleanup.tombstone_path.clone(), cleanup))
                .collect::<HashMap<_, _>>();
            let mut cleanup_pending_count = 0usize;
            for removal in removals {
                let Some(tombstone_path) = removal.tombstone_path() else {
                    continue;
                };
                let Some(cleanup) = cleanup_by_tombstone.remove(&tombstone_path.display().to_string()) else {
                    cleanup_pending_count = cleanup_pending_count.saturating_add(1);
                    tracing::error!(
                        tombstone_path = %tombstone_path.display(),
                        "committed plugin artifact cleanup record is unavailable in the local completion pass"
                    );
                    continue;
                };
                match removal.remove() {
                    Ok(()) => {
                        if let Err(error) = self
                            .repository
                            .complete_plugin_artifact_cleanup(cleanup.cleanup_id)
                            .await
                        {
                            cleanup_pending_count = cleanup_pending_count.saturating_add(1);
                            tracing::warn!(
                                cleanup_id = %cleanup.cleanup_id,
                                tombstone_path = %cleanup.tombstone_path,
                                error = %error,
                                "plugin artifact was removed but cleanup acknowledgement will be retried"
                            );
                        }
                    }
                    Err(error) => {
                        cleanup_pending_count = cleanup_pending_count.saturating_add(1);
                        tracing::warn!(
                            cleanup_id = %cleanup.cleanup_id,
                            tombstone_path = %cleanup.tombstone_path,
                            error = %error,
                            "plugin artifact cleanup remains pending after committed family uninstall"
                        );
                        if let Err(record_error) = self
                            .repository
                            .record_plugin_artifact_cleanup_failure(
                                &RecordPluginArtifactCleanupFailureInput {
                                    cleanup_id: cleanup.cleanup_id,
                                    last_error: error.to_string(),
                                    attempted_at: OffsetDateTime::now_utc(),
                                },
                            )
                            .await
                        {
                            tracing::warn!(
                                cleanup_id = %cleanup.cleanup_id,
                                error = %record_error,
                                "failed to record pending plugin artifact cleanup failure"
                            );
                        }
                    }
                }
            }

            Ok::<(usize, usize, usize), anyhow::Error>(
                (instances.len(), installations.len(), cleanup_pending_count),
            )
        }
        .await;

        match uninstall_result {
            Ok((retained_instance_count, retained_installation_count, cleanup_pending_count)) => {
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
                        "cleanup_pending_count": cleanup_pending_count,
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

    pub(super) async fn restore_runtime_scopes_after_uninstall_rejection(
        &self,
        installations: &[domain::LocalPluginInstallationRecord],
    ) -> Result<()> {
        for installation in installations {
            self.runtime.activate_plugin(installation).await?;
        }
        Ok(())
    }
}
