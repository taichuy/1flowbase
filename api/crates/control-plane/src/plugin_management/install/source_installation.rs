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
    pub async fn install_uploaded_plugin(
        &self,
        command: InstallUploadedPluginCommand,
    ) -> Result<InstallPluginResult> {
        let file_name = command.file_name.clone();
        let intake = intake_package_bytes(
            &command.package_bytes,
            &PackageIntakePolicy {
                source_kind: "uploaded".to_string(),
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: None,
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(file_name.clone()),
            },
        )
        .await?;
        self.ensure_console_package_kind(route_plugin_package(&intake.manifest)?)?;
        self.install_intake_result(
            command.actor_user_id,
            intake,
            Some(command.package_bytes),
            json!({
                "install_kind": "upload",
                "file_name": file_name,
            }),
        )
        .await
    }

    pub async fn install_uploaded_model_provider(
        &self,
        command: InstallUploadedPluginCommand,
    ) -> Result<InstallPluginResult> {
        let actor_user_id = command.actor_user_id;
        let installation_id = self.install_uploaded_plugin(command).await?.installation.id;
        self.enable_and_assign_model_provider_installation(actor_user_id, installation_id)
            .await
    }

    pub async fn install_uploaded_network_egress_provider(
        &self,
        command: InstallUploadedPluginCommand,
    ) -> Result<InstallPluginResult> {
        self.install_uploaded_plugin(command).await
    }

    pub async fn install_extension_node_plugin(
        &self,
        command: InstallExtensionNodePluginCommand,
    ) -> Result<InstallPluginResult> {
        let intake = intake_package_bytes(
            &command.package_bytes,
            &PackageIntakePolicy {
                source_kind: command.source_kind.clone(),
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: None,
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(command.file_name.clone()),
            },
        )
        .await?;
        let expected_category = match intake.manifest.consumption_kind {
            PluginConsumptionKind::HostExtension => ExtensionCatalogCategory::HostExtensions,
            PluginConsumptionKind::RuntimeExtension => ExtensionCatalogCategory::RuntimeExtensions,
            PluginConsumptionKind::CapabilityPlugin => ExtensionCatalogCategory::CapabilityPlugins,
        };
        if expected_category != command.category {
            return Err(ControlPlaneError::InvalidInput("extension_catalog_category").into());
        }
        let package_kind = route_plugin_package(&intake.manifest)?;
        let install = self
            .install_intake_result(
                command.actor_user_id,
                intake,
                Some(command.package_bytes),
                json!({
                    "install_kind": "extension_inventory_node_registration",
                    "file_name": command.file_name,
                    "category": command.category.as_str(),
                }),
            )
            .await?;
        // Managed services need schema application and explicit permission grants before enable.
        // Re-upload retains the repository's deliberate desired state; installation never activates it.
        if is_host_extension_installation(&install.installation)
            || is_system_managed_service(&install.installation)
        {
            return Ok(install);
        }
        self.ensure_initial_managed_assignment(command.actor_user_id, &install.installation)
            .await?;
        self.enable_plugin(EnablePluginCommand {
            actor_user_id: command.actor_user_id,
            installation_id: install.installation.id,
        })
        .await?;
        if package_kind == RoutedPluginPackageKind::ProviderDistributionRuleRuntime {
            let previous = self
                .repository
                .list_installations()
                .await?
                .into_iter()
                .filter(|candidate| {
                    candidate.id != install.installation.id
                        && candidate.provider_code == install.installation.provider_code
                        && candidate.metadata_json["plugin_type"] == "provider_distribution_rule"
                        && candidate.desired_state == domain::PluginDesiredState::ActiveRequested
                })
                .collect::<Vec<_>>();
            for candidate in previous {
                self.disable_plugin(DisablePluginCommand {
                    actor_user_id: command.actor_user_id,
                    installation_id: candidate.id,
                })
                .await?;
            }
        }
        let installation = self
            .repository
            .get_installation(install.installation.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let local_artifact = self
            .repository
            .get_artifact_instance(&self.node_id, installation.id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_artifact_instance"))?;
        Ok(InstallPluginResult {
            installation,
            local_artifact,
            task: install.task,
        })
    }

    pub async fn install_official_plugin(
        &self,
        command: InstallOfficialPluginCommand,
    ) -> Result<InstallPluginResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;

        let snapshot = self
            .official_source
            .list_official_catalog_for_workspace(actor.current_workspace_id)
            .await?;
        let entry = snapshot
            .entries
            .into_iter()
            .find(|item| item.plugin_id == command.plugin_id)
            .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
        if self.is_model_provider_console_operation() && entry.plugin_type != "model_provider" {
            return Err(
                ControlPlaneError::PermissionDenied("model_provider_plugin_required").into(),
            );
        }
        if self.is_network_egress_provider_console_operation()
            && entry.plugin_type != "network_egress_provider"
        {
            return Err(ControlPlaneError::PermissionDenied(
                "network_egress_provider_plugin_required",
            )
            .into());
        }
        let compatibility_override = validate_official_plugin_compatibility_override(
            &entry,
            &self.host_version,
            command.compatibility_override.as_ref(),
        )?;
        let downloaded = self
            .official_source
            .download_plugin_for_workspace(actor.current_workspace_id, &entry)
            .await?;
        let intake = intake_package_bytes(
            &downloaded.package_bytes,
            &PackageIntakePolicy {
                source_kind: snapshot.source.source_kind.clone(),
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: None,
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(downloaded.file_name.clone()),
            },
        )
        .await?;
        let risk_warnings = official_package_risk_warnings(
            &downloaded.package_bytes,
            &entry.selected_artifact.checksum,
            &intake,
        );
        let risk_override =
            validate_plugin_risk_override(&risk_warnings, command.risk_override.as_ref())?;
        let package_kind = route_plugin_package(&intake.manifest)?;
        if catalog_plugin_type(&intake.manifest)? != entry.plugin_type {
            return Err(ControlPlaneError::Conflict("extension_catalog_package_mismatch").into());
        }
        self.ensure_console_package_kind(package_kind)?;
        let result = async {
            let mut detail_json = json!({
                "install_kind": "official_source",
                "plugin_id": command.plugin_id,
                "file_name": downloaded.file_name,
            });
            if let Some(compatibility_override) = compatibility_override.clone() {
                detail_json["compatibility_override"] = compatibility_override;
            }
            detail_json["risk_warnings"] = json!(risk_warnings);
            if let Some(risk_override) = risk_override.clone() {
                detail_json["risk_override"] = risk_override;
            }
            let install = self
                .install_intake_result(
                    command.actor_user_id,
                    intake,
                    Some(downloaded.package_bytes.clone()),
                    detail_json,
                )
                .await?;
            if is_host_extension_installation(&install.installation)
                || is_system_managed_service(&install.installation)
            {
                return Ok::<InstallPluginResult, anyhow::Error>(install);
            }
            if self.is_network_egress_provider_console_operation() {
                Ok(install)
            } else {
                self.enable_and_assign_model_provider_installation(
                    command.actor_user_id,
                    install.installation.id,
                )
                .await
            }
        }
        .await;
        result
    }

    pub async fn install_resolved_official_plugin(
        &self,
        command: InstallResolvedOfficialPluginCommand,
    ) -> Result<InstallPluginResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        if self.is_model_provider_console_operation() && command.plugin_type != "model_provider" {
            return Err(
                ControlPlaneError::PermissionDenied("model_provider_plugin_required").into(),
            );
        }
        if self.is_network_egress_provider_console_operation()
            && command.plugin_type != "network_egress_provider"
        {
            return Err(ControlPlaneError::PermissionDenied(
                "network_egress_provider_plugin_required",
            )
            .into());
        }
        let compatibility_override = validate_plugin_compatibility_requirement(
            &command.minimum_host_version,
            &self.host_version,
            command.compatibility_override.as_ref(),
        )?;
        let intake = intake_package_bytes(
            &command.package_bytes,
            &PackageIntakePolicy {
                source_kind: command.source_kind.clone(),
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: None,
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(command.file_name.clone()),
            },
        )
        .await?;
        let risk_warnings = official_package_risk_warnings(
            &command.package_bytes,
            &command.expected_checksum,
            &intake,
        );
        let risk_override =
            validate_plugin_risk_override(&risk_warnings, command.risk_override.as_ref())?;
        let package_kind = route_plugin_package(&intake.manifest)?;
        if catalog_plugin_type(&intake.manifest)? != command.plugin_type {
            return Err(ControlPlaneError::Conflict("extension_catalog_package_mismatch").into());
        }
        self.ensure_console_package_kind(package_kind)?;
        let mut detail_json = json!({
            "install_kind": "official_extension_catalog",
            "plugin_id": command.plugin_id,
            "file_name": command.file_name,
            "risk_warnings": risk_warnings,
        });
        if let Some(value) = compatibility_override {
            detail_json["compatibility_override"] = value;
        }
        if let Some(value) = risk_override {
            detail_json["risk_override"] = value;
        }
        let install = self
            .install_intake_result(
                command.actor_user_id,
                intake,
                Some(command.package_bytes),
                detail_json,
            )
            .await?;
        if is_host_extension_installation(&install.installation)
            || is_system_managed_service(&install.installation)
        {
            return Ok(install);
        }
        if self.is_network_egress_provider_console_operation() {
            Ok(install)
        } else {
            self.enable_and_assign_model_provider_installation(
                command.actor_user_id,
                install.installation.id,
            )
            .await
        }
    }
}
