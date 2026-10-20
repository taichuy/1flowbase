use super::*;

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository
        + FrontendBlockCatalogRepository,
    H: ProviderRuntimePort,
{
    pub(super) async fn install_plugin_with_metadata(
        &self,
        command: InstallPluginCommand,
        source_metadata: InstallSourceMetadata,
        detail_json: serde_json::Value,
    ) -> Result<InstallPluginResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let task_id = Uuid::now_v7();
        let task = self
            .repository
            .create_task(&CreatePluginTaskInput {
                task_id,
                installation_id: None,
                workspace_id: None,
                provider_code: "pending_provider".to_string(),
                task_kind: domain::PluginTaskKind::Install,
                status: domain::PluginTaskStatus::Queued,
                status_message: Some("pending".to_string()),
                detail_json: detail_json.clone(),
                actor_user_id: Some(command.actor_user_id),
            })
            .await?;
        let running_task = self
            .transition_task(
                &task,
                domain::PluginTaskStatus::Running,
                Some("running".to_string()),
                detail_json.clone(),
            )
            .await?;

        let installation_result: anyhow::Result<(domain::PluginInstallationRecord, bool)> = async {
            let manifest = load_plugin_manifest(&command.package_root)?;
            let package_kind = route_plugin_package(&manifest)?;
            let plugin_code = plugin_code_from_plugin_id(&manifest.plugin_id)?;
            let package_id = manifest.versioned_plugin_id().map_err(map_framework_error)?;
            let install_path = self
                .install_root
                .join("installed")
                .join(&plugin_code)
                .join(&manifest.version);
            let package_archive_path = self
                .install_root
                .join("packages")
                .join(&plugin_code)
                .join(format!("{package_id}.1flowbasepkg"));
            let admission = crate::ports::PluginInstallationAdmission {
                plugin_id: package_id.clone(),
                content: if manifest.managed.is_some() {
                    crate::ports::PluginInstallationContentIdentity::ManagedArchive {
                        checksum: source_metadata.checksum.clone().ok_or(
                            ControlPlaneError::Conflict("managed_archive_identity_missing"),
                        )?,
                    }
                } else {
                    crate::ports::PluginInstallationContentIdentity::Legacy
                },
            };
            // Declare the lease first: staging guards restore synchronously before it drops.
            let lease = self.repository.begin_plugin_installation(&admission).await?;
            let staged_package = source_metadata
                .package_bytes
                .as_deref()
                .map(|bytes| filesystem::StagedArtifactPath::prepare_file(bytes, &package_archive_path))
                .transpose()?;
            let staged_installation = filesystem::StagedArtifactPath::prepare_directory(
                Path::new(&command.package_root),
                &install_path,
            )?;
            if let Some(managed) = &manifest.managed {
                let package_root = tokio::fs::canonicalize(staged_installation.staged_path()).await?;
                for binding in &managed.execution_bindings {
                    let entry = staged_installation.staged_path().join(&binding.runtime.entry);
                    let entry = tokio::fs::canonicalize(entry).await
                        .map_err(|_| ControlPlaneError::InvalidInput("managed_execution_entry"))?;
                    if !entry.starts_with(&package_root) {
                        return Err(ControlPlaneError::InvalidInput("managed_execution_entry").into());
                    }
                    let metadata = tokio::fs::metadata(&entry).await
                        .map_err(|_| ControlPlaneError::InvalidInput("managed_execution_entry"))?;
                    if !metadata.is_file() {
                        return Err(ControlPlaneError::InvalidInput("managed_execution_entry").into());
                    }
                }
            }
            let manifest_fingerprint =
                compute_manifest_fingerprint(&staged_installation.staged_path().join("manifest.yaml"))
                    .await
                    .map_err(map_framework_error)?;
            write_artifact_marker(
                staged_installation.staged_path(),
                &package_id,
                &manifest.version,
                source_metadata.checksum.as_deref(),
                Some(&manifest_fingerprint),
                Some(&source_metadata.source_kind),
                Some(&source_metadata.trust_level),
                source_metadata.signature_status.as_deref(),
                source_metadata.signature_algorithm.as_deref(),
                source_metadata.signing_key_id.as_deref(),
            )?;
            let installation_input = (async {
                match package_kind {
                RoutedPluginPackageKind::HostExtension => {
                    ensure_root_actor(&actor)?;
                    ensure_uploaded_host_extensions_enabled(self.allow_uploaded_host_extensions)?;
                    let mut metadata_json = json!({"plugin_type": "host_extension"});
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = prepare_installation_commit(
                        staged_installation.staged_path(),
                        &self.node_id,
                        PreparedPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: plugin_code.clone(),
                            plugin_id: package_id.clone(),
                            plugin_version: manifest.version.clone(),
                            contract_version: manifest.contract_version.clone(),
                            protocol: manifest.runtime.protocol.clone(),
                            display_name: manifest.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::PendingRestart,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::PendingRestart,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        },
                        &manifest,
                        None,
                    ).await
                    ?;
                    Ok::<CommitPluginInstallationInput, anyhow::Error>(installation)
                }
                RoutedPluginPackageKind::ModelProviderRuntime => {
                    let installed_package = load_provider_package(staged_installation.staged_path())?;
                    let mut metadata_json = json!({
                        "plugin_type": "model_provider",
                        "help_url": installed_package.provider.help_url,
                        "default_base_url": installed_package.provider.default_base_url,
                        "model_discovery_mode": format!("{:?}", installed_package.provider.model_discovery_mode).to_ascii_lowercase(),
                        "icon": installed_package.manifest.icon,
                        "supported_model_types": ["llm"],
                        "runtime_capabilities": installed_package.manifest.runtime.capabilities,
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation_input = PreparedPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: installed_package.provider.provider_code.clone(),
                            plugin_id: installed_package.identifier(),
                            plugin_version: installed_package.manifest.version.clone(),
                            contract_version: installed_package.manifest.contract_version.clone(),
                            protocol: installed_package.provider.protocol.clone(),
                            display_name: installed_package.provider.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        };
                    let package_catalog = UpsertPluginPackageCatalogProjectionInput {
                        installation_id: installation_input.installation_id,
                        package_code: installation_input.provider_code.clone(),
                        package_version: installation_input.plugin_version.clone(),
                        catalog_snapshot_json: provider_catalog_snapshot(&installed_package)?,
                        projection_status: domain::PluginPackageCatalogProjectionStatus::Ok,
                        last_error_message: None,
                        refreshed_at: Some(OffsetDateTime::now_utc()),
                    };
                    let installation = prepare_installation_commit(
                        staged_installation.staged_path(),
                        &self.node_id,
                        installation_input,
                        &manifest,
                        Some(package_catalog),
                    ).await
                    ?;
                    Ok::<CommitPluginInstallationInput, anyhow::Error>(installation)
                }
                RoutedPluginPackageKind::DataSourceRuntime => {
                    let installed_package =
                        plugin_framework::DataSourcePackage::load_from_dir(staged_installation.staged_path())
                            .map_err(map_framework_error)?;
                    let mut metadata_json = json!({
                        "plugin_type": "data_source",
                        "supported_resource_kinds": installed_package.definition.resource_kinds,
                        "auth_modes": installed_package.definition.auth_modes,
                        "capabilities": installed_package.definition.capabilities,
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = prepare_installation_commit(
                        staged_installation.staged_path(),
                        &self.node_id,
                        PreparedPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: installed_package.definition.source_code.clone(),
                            plugin_id: installed_package.identifier(),
                            plugin_version: installed_package.manifest.version.clone(),
                            contract_version: installed_package.manifest.contract_version.clone(),
                            protocol: "data_source".to_string(),
                            display_name: installed_package.definition.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        },
                        &manifest,
                        None,
                    ).await
                    ?;
                    Ok::<CommitPluginInstallationInput, anyhow::Error>(installation)
                }
                RoutedPluginPackageKind::NetworkEgressProviderRuntime => {
                    let installed_package = plugin_framework::NetworkEgressProviderPackage::load_from_dir(
                        staged_installation.staged_path(),
                    )
                    .map_err(map_framework_error)?;
                    let mut metadata_json = json!({
                        "plugin_type": "network_egress_provider",
                        "slot_code": "network_egress_provider",
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = prepare_installation_commit(
                        staged_installation.staged_path(),
                        &self.node_id,
                        PreparedPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: installed_package.provider.provider_code.clone(),
                            plugin_id: installed_package.identifier(),
                            plugin_version: installed_package.manifest.version.clone(),
                            contract_version: installed_package.manifest.contract_version.clone(),
                            protocol: installed_package.manifest.runtime.protocol.clone(),
                            display_name: installed_package.provider.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        },
                        &manifest,
                        None,
                    ).await
                    ?;
                    Ok::<CommitPluginInstallationInput, anyhow::Error>(installation)
                }
                RoutedPluginPackageKind::ProviderDistributionRuleRuntime => {
                    let manifest = load_plugin_manifest(staged_installation.staged_path())?;
                    let mut metadata_json = json!({
                        "plugin_type": "provider_distribution_rule",
                        "slot_code": "provider_distribution_rule",
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation = prepare_installation_commit(
                        staged_installation.staged_path(),
                        &self.node_id,
                        PreparedPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: stable_plugin_unique_identifier(&manifest.plugin_id),
                            plugin_id: manifest.versioned_plugin_id().map_err(map_framework_error)?,
                            plugin_version: manifest.version.clone(),
                            contract_version: manifest.contract_version.clone(),
                            protocol: manifest.runtime.protocol.clone(),
                            display_name: manifest.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        },
                        &manifest,
                        None,
                    ).await
                    ?;
                    Ok::<CommitPluginInstallationInput, anyhow::Error>(installation)
                }
                RoutedPluginPackageKind::CapabilityPlugin | RoutedPluginPackageKind::ManagedContributions => {
                    let manifest = load_plugin_manifest(staged_installation.staged_path())?;
                    let mut metadata_json = json!({
                        "plugin_type": package_kind.as_plugin_type(),
                        "node_contributions": manifest
                            .node_contributions
                            .iter()
                            .map(|entry| entry.contribution_code.clone())
                            .collect::<Vec<_>>(),
                        "block_contributions": manifest
                            .block_contributions
                            .iter()
                            .map(|entry| entry.contribution_code.clone())
                            .collect::<Vec<_>>(),
                    });
                    merge_install_detail_metadata(&mut metadata_json, &detail_json);
                    let installation_input = PreparedPluginInstallationInput {
                            installation_id: Uuid::now_v7(),
                            provider_code: stable_plugin_unique_identifier(&manifest.plugin_id),
                            plugin_id: manifest.versioned_plugin_id().map_err(map_framework_error)?,
                            plugin_version: manifest.version.clone(),
                            contract_version: manifest.contract_version.clone(),
                            protocol: manifest.runtime.protocol.clone(),
                            display_name: manifest.display_name.clone(),
                            source_kind: source_metadata.source_kind.clone(),
                            trust_level: source_metadata.trust_level.clone(),
                            verification_status: domain::PluginVerificationStatus::Valid,
                            desired_state: domain::PluginDesiredState::Disabled,
                            runtime_status: domain::PluginRuntimeStatus::Inactive,
                            availability_status: derive_availability_status(
                                domain::PluginDesiredState::Disabled,
                                domain::PluginArtifactStatus::Ready,
                                domain::PluginRuntimeStatus::Inactive,
                            ),
                            package_path: source_metadata
                                .package_bytes
                                .as_ref()
                                .map(|_| package_archive_path.display().to_string()),
                            installed_path: install_path.display().to_string(),
                            checksum: source_metadata.checksum.clone(),
                            manifest_fingerprint: Some(manifest_fingerprint),
                            signature_status: source_metadata.signature_status.clone(),
                            signature_algorithm: source_metadata.signature_algorithm.clone(),
                            signing_key_id: source_metadata.signing_key_id.clone(),
                            last_load_error: None,
                            metadata_json,
                            actor_user_id: command.actor_user_id,
                        };
                    let installation = prepare_installation_commit(
                        staged_installation.staged_path(),
                        &self.node_id,
                        installation_input,
                        &manifest,
                        None,
                    ).await
                    ?;
                    Ok::<CommitPluginInstallationInput, anyhow::Error>(installation)
                }
                }
            }).await?;
            let installation = super::super::managed_installation::publish_installation(
                lease, staged_installation, staged_package, installation_input,
            ).await?;
            let audit = self.repository.append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "plugin_installation",
                Some(installation.id),
                "plugin.installed",
                plugin_install_audit_detail(&installation, &detail_json,
                    matches!(package_kind, RoutedPluginPackageKind::HostExtension)),
            )).await;
            if !matches!(package_kind,
                RoutedPluginPackageKind::NetworkEgressProviderRuntime
                | RoutedPluginPackageKind::CapabilityPlugin
                | RoutedPluginPackageKind::ManagedContributions) {
                audit?;
            }
            Ok((installation, true))
        }
        .await;

        match installation_result {
            Ok((installation, artifact_committed)) => {
                if !artifact_committed {
                    if let Err(error) = self.record_ready_current_node_artifact(&installation).await
                    {
                        let _ = self
                            .transition_task(
                                &running_task,
                                domain::PluginTaskStatus::Failed,
                                Some(error.to_string()),
                                json!({
                                    "installation_id": installation.id,
                                    "provider_code": installation.provider_code,
                                }),
                            )
                            .await;
                        return Err(error);
                    }
                }
                if matches!(
                    installation.desired_state,
                    domain::PluginDesiredState::ActiveRequested
                ) && !is_host_extension_installation(&installation)
                    && !is_system_managed_service(&installation)
                {
                    let reactivation = async {
                        let local_installation = self
                            .ready_current_node_installation(installation.id)
                            .await?;
                        match self.runtime.activate_plugin(&local_installation).await {
                            Ok(()) => {
                                self.mark_current_node_runtime_status(
                                    &installation,
                                    domain::PluginRuntimeStatus::Active,
                                    None,
                                )
                                .await?;
                                Ok(())
                            }
                            Err(error) => {
                                self.mark_current_node_runtime_status(
                                    &installation,
                                    domain::PluginRuntimeStatus::LoadFailed,
                                    Some(error.to_string()),
                                )
                                .await?;
                                Err(error)
                            }
                        }
                    }
                    .await;
                    if let Err(error) = reactivation {
                        let _ = self
                            .transition_task(
                                &running_task,
                                domain::PluginTaskStatus::Failed,
                                Some(error.to_string()),
                                json!({
                                    "installation_id": installation.id,
                                    "provider_code": installation.provider_code,
                                }),
                            )
                            .await;
                        return Err(error);
                    }
                }
                let local_path = self
                    .repository
                    .get_artifact_instance(&self.node_id, installation.id)
                    .await?
                    .and_then(|artifact| artifact.local_path);
                let installed_message = if is_host_extension_installation(&installation) {
                    "installed; restart required"
                } else {
                    "installed"
                };
                let mut task_detail_json = json!({
                    "installation_id": installation.id,
                    "provider_code": installation.provider_code,
                    "plugin_id": installation.plugin_id,
                    "local_path": local_path,
                });
                if let Some(compatibility_override) =
                    detail_json.get("compatibility_override").cloned()
                {
                    task_detail_json["compatibility_override"] = compatibility_override;
                }
                let task = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Succeeded,
                        Some(installed_message.to_string()),
                        task_detail_json,
                    )
                    .await?;
                let local_artifact = self
                    .repository
                    .get_artifact_instance(&self.node_id, installation.id)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("plugin_artifact_instance"))?;
                Ok(InstallPluginResult {
                    installation,
                    local_artifact,
                    task,
                })
            }
            Err(error) => {
                let _ = self
                    .transition_task(
                        &running_task,
                        domain::PluginTaskStatus::Failed,
                        Some(error.to_string()),
                        detail_json,
                    )
                    .await;
                Err(error)
            }
        }
    }
}
