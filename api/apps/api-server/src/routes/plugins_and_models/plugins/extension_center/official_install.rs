use super::*;

pub(super) async fn install_or_update_official_extension(
    dependencies: &ExtensionCenterDependencies,
    actor: &domain::ActorContext,
    body: InstallOfficialExtensionBody,
    operation_id: &'static str,
) -> Result<ExtensionInstallOutcome, ApiError> {
    let category = ExtensionCatalogCategory::parse(&body.category)?;
    let identity = requested_installation_identity(
        category,
        &body.catalog_id,
        &body.version,
        &dependencies.api_node_id,
    )?;
    let install_service = extension_installation_service(dependencies);
    if let Some(installation) = install_service
        .find_local_installation(&dependencies.api_node_id, &identity)
        .await?
    {
        if domain::is_client_collector_receipt(&installation.receipt) {
            install_service
                .client_collector_manifest(&installation)
                .await
                .map_err(|_| {
                    control_plane::errors::ControlPlaneError::Conflict(
                        "client_collector_artifact_unavailable",
                    )
                })?;
        }
        let artifact_missing = is_node_plugin_category(category)
            && control_plane::ports::PluginRepository::get_artifact_instance(
                &dependencies.store,
                &dependencies.api_node_id,
                installation.id,
            )
            .await?
            .is_some_and(|artifact| {
                artifact.artifact_status == domain::PluginArtifactInstanceStatus::Missing
            });
        if !artifact_missing {
            let node_plugin_installation_id = if is_node_plugin_category(category)
                && !domain::is_client_collector_receipt(&installation.receipt)
            {
                Some(installation.id.to_string())
            } else {
                None
            };
            return Ok(ExtensionInstallOutcome::Existing(
                ExtensionInstallResponse {
                    application_action: installation.application_action.as_str().to_string(),
                    application_status: default_application_status(installation.application_action)
                        .to_string(),
                    installation: to_local_inventory_entry(installation)?,
                    local_artifact_was_present: true,
                    node_plugin_installation_id,
                    managed_schema_preview: None,
                    managed_schema_receipt: None,
                },
            ));
        }
    }

    let located = dependencies
        .official_extension_catalog_source
        .find_entry_for_workspace(
            actor.current_workspace_id,
            category.as_str(),
            &body.catalog_id,
        )
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_catalog_entry",
        ))?;
    if located.entry.category != category.as_str()
        || located.entry.id != body.catalog_id
        || located.entry.version != body.version
        || located.entry.organization != identity.organization
        || located.entry.artifact != identity.artifact_id
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_catalog_identity",
        )
        .into());
    }
    let collector_descriptor = located.entry.source.client_collector()?;
    if collector_descriptor.is_some() && category != ExtensionCatalogCategory::RuntimeExtensions {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "client_collector_category",
        )
        .into());
    }
    let source_kind = if located.source_kind == "official_repository" {
        "official_registry"
    } else {
        "mirror_registry"
    };

    let descriptor = dependencies
        .official_extension_catalog_source
        .resolve_artifact(&located.entry)?;
    let trusted_public_keys = dependencies.official_plugin_source.trusted_public_keys();
    let trusted_key_ids = trusted_public_keys
        .iter()
        .map(|key| key.key_id.clone())
        .collect::<Vec<_>>();
    let preflight = artifact_preflight_challenge(&located.entry, &descriptor, &trusted_key_ids);
    let declared_warnings = preflight
        .warnings
        .iter()
        .map(|warning| domain::ExtensionIntegrityWarning {
            code: warning.code.clone(),
            message: warning.message.clone(),
            overridable: warning.overridable,
        })
        .collect();
    let confirmation_receipt = match validate_preflight_overrides(
        &preflight,
        body.risk_override.as_ref(),
        body.compatibility_override.as_ref(),
    )? {
        PreflightDecision::Challenge => return Ok(ExtensionInstallOutcome::Challenge(preflight)),
        PreflightDecision::Accepted(receipt) => receipt,
    };
    let downloaded = dependencies
        .official_extension_catalog_source
        .download_artifact_for_workspace(actor.current_workspace_id, &located.entry)
        .await
        .map_err(|error| {
            if error
                .downcast_ref::<OfficialExtensionArtifactError>()
                .is_some()
            {
                ApiError(error)
            } else {
                OfficialExtensionArtifactError::DownloadUnavailable { source: error }.into()
            }
        })?;
    verify_trusted_artifact_signature(&downloaded, &trusted_public_keys)?;
    let mut signature_status =
        signature_status_from_descriptor(&downloaded.descriptor, &trusted_key_ids);
    let mut signature_algorithm = downloaded
        .descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("algorithm"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut signing_key_id = downloaded
        .descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut application_action = catalog_application_action(category);
    let mut managed_schema_declaration = None;
    if let Some(descriptor) = &collector_descriptor {
        let manifest = control_plane::plugin_management::inspect_client_collector_archive(
            downloaded.artifact_bytes.as_slice(),
        )?;
        control_plane::plugin_management::ensure_client_collector_identity(&manifest, &identity)?;
        if &manifest.descriptor() != descriptor {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "client_collector_descriptor",
            )
            .into());
        }
    }
    if is_node_plugin_category(category) && collector_descriptor.is_none() {
        let inspection = inspect_node_plugin(
            dependencies,
            &downloaded.file_name,
            &downloaded.artifact_bytes,
            source_kind,
        )
        .await?;
        let catalog_plugin_id = located
            .entry
            .source
            .metadata
            .get("plugin_id")
            .and_then(serde_json::Value::as_str)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_plugin_id",
            ))?;
        let catalog_plugin_type = located
            .entry
            .source
            .metadata
            .get("plugin_type")
            .and_then(serde_json::Value::as_str)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_plugin_type",
            ))?;
        let manifest_plugin_id = format!("{}.{}", inspection.organization, inspection.plugin_id);
        if inspection.category != category
            || inspection.organization != located.entry.organization
            || inspection.version != located.entry.version
            || manifest_plugin_id != catalog_plugin_id
            || inspection.plugin_type != catalog_plugin_type
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "extension_artifact_identity",
            )
            .into());
        }
        signature_status = inspection.signature_status;
        signature_algorithm = inspection.signature_algorithm;
        signing_key_id = inspection.signing_key_id;
        application_action = inspection.application_action;
        managed_schema_declaration = inspection.managed_schema;
    }
    let prepared_schema = prepare_managed_schema(
        dependencies,
        actor.current_workspace_id,
        managed_schema_declaration.as_ref(),
    )
    .await?;
    let managed_schema_preview = prepared_schema.as_ref().map(|prepared| prepared.preview());
    let trust = match (source_kind, signature_status) {
        ("official_registry", domain::ExtensionSignatureStatus::Verified) => "official",
        (_, domain::ExtensionSignatureStatus::Verified) => "trusted",
        _ => "unknown",
    };
    let risk_override = body.risk_override.map(|value| ExtensionRiskOverride {
        reason: value.reason,
        acknowledged_warnings: value.acknowledged_warnings,
    });
    if is_node_plugin_category(category) && collector_descriptor.is_none() {
        let installed = service(dependencies, actor, operation_id)
            .install_extension_node_plugin(InstallExtensionNodePluginCommand {
                actor_user_id: actor.user_id,
                category,
                file_name: downloaded.file_name,
                package_bytes: downloaded.artifact_bytes,
                source_kind: source_kind.to_string(),
            })
            .await?;
        let current = control_plane::ports::PluginRepository::list_assignments(
            &dependencies.store,
            actor.current_workspace_id,
        )
        .await?
        .into_iter()
        .find(|assignment| assignment.provider_code == installed.installation.provider_code);
        let is_model_provider = installed
            .installation
            .metadata_json
            .get("plugin_type")
            .and_then(serde_json::Value::as_str)
            == Some("model_provider");
        let supports_assignment = matches!(
            installed.installation.contract_version.as_str(),
            plugin_framework::provider_contract::CURRENT_PROVIDER_CONTRACT
                | "1flowbase.data_source/v1"
                | "1flowbase.capability/v1"
        );
        let requires_model_provider_switch = operation_id == "extension_center.update"
            && is_model_provider
            && current
                .as_ref()
                .is_some_and(|assignment| assignment.installation_id != installed.installation.id);
        if requires_model_provider_switch {
            service(dependencies, actor, operation_id)
                .switch_version(SwitchPluginVersionCommand {
                    actor_user_id: actor.user_id,
                    provider_code: installed.installation.provider_code.clone(),
                    target_installation_id: installed.installation.id,
                })
                .await?;
        } else if supports_assignment
            && current
                .as_ref()
                .is_none_or(|assignment| assignment.installation_id != installed.installation.id)
        {
            service(dependencies, actor, operation_id)
                .assign_plugin(AssignPluginCommand {
                    actor_user_id: actor.user_id,
                    installation_id: installed.installation.id,
                })
                .await?;
        }
        let installation = control_plane::ports::ExtensionInstallationRepository::find_extension_installation_by_id(
            &dependencies.store,
            &dependencies.api_node_id,
            installed.installation.id,
        )
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "extension_installation",
        ))?;
        let managed_schema_receipt = match prepared_schema {
            Some(prepared) => Some(prepared.apply(dependencies).await?),
            None => None,
        };
        return Ok(ExtensionInstallOutcome::Created(ExtensionInstallResponse {
            application_action: installation.application_action.as_str().to_string(),
            application_status: default_application_status(installation.application_action)
                .to_string(),
            installation: to_local_inventory_entry(installation.clone())?,
            local_artifact_was_present: false,
            node_plugin_installation_id: Some(installation.id.to_string()),
            managed_schema_preview,
            managed_schema_receipt,
        }));
    }
    let outcome = install_service
        .install_from_bytes(InstallExtensionArtifactCommand {
            actor_user_id: actor.user_id,
            category,
            organization: located.entry.organization,
            artifact_id: located.entry.artifact,
            version: located.entry.version,
            node_id: dependencies.api_node_id.clone(),
            artifact_bytes: downloaded.artifact_bytes,
            source: if source_kind == "official_registry" {
                "official".to_string()
            } else {
                "mirror".to_string()
            },
            trust: trust.to_string(),
            expected_checksum: downloaded.descriptor.expected_checksum,
            signature_status,
            signature_algorithm,
            signing_key_id,
            declared_warnings,
            risk_override,
            confirmation_receipt,
            application_action,
        })
        .await?;
    let (installation, local_artifact_was_present) = match outcome {
        ExtensionArtifactInstallOutcome::RiskConfirmationRequired { risk_challenge } => {
            return Ok(ExtensionInstallOutcome::Challenge(domain_challenge(
                risk_challenge,
                preflight.compatibility,
            )));
        }
        ExtensionArtifactInstallOutcome::Installed {
            installation,
            local_artifact_was_present,
        } => (installation, local_artifact_was_present),
    };
    let node_plugin_installation_id = None;
    Ok(ExtensionInstallOutcome::Created(ExtensionInstallResponse {
        application_action: installation.application_action.as_str().to_string(),
        application_status: default_application_status(installation.application_action).to_string(),
        installation: to_local_inventory_entry(installation)?,
        local_artifact_was_present,
        node_plugin_installation_id,
        managed_schema_preview: None,
        managed_schema_receipt: None,
    }))
}
