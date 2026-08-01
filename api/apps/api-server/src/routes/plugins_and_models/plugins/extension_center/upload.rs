use super::*;

async fn read_extension_upload(
    multipart: &mut Multipart,
) -> Result<ExtensionUploadFields, ApiError> {
    let mut fields = ExtensionUploadFields::default();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| control_plane::errors::ControlPlaneError::InvalidInput("extension_upload"))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            fields.file_name = Some(
                field
                    .file_name()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("extension-upload.bin")
                    .to_string(),
            );
            fields.artifact_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput("extension_file")
                    })?
                    .to_vec(),
            );
            continue;
        }
        let value = field.text().await.map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("extension_upload_field")
        })?;
        match name.as_str() {
            "category" => fields.category = Some(value),
            "organization" => fields.organization = Some(value),
            "artifact_id" => fields.artifact_id = Some(value),
            "version" => fields.version = Some(value),
            "risk_override" => {
                fields.risk_override = Some(serde_json::from_str(&value).map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput(
                        "extension_risk_override",
                    )
                })?)
            }
            "compatibility_override" => {
                fields.compatibility_override =
                    Some(serde_json::from_str(&value).map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "extension_compatibility_override",
                        )
                    })?)
            }
            _ => {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "extension_upload_field",
                )
                .into())
            }
        }
    }
    if fields
        .artifact_bytes
        .as_ref()
        .is_none_or(|bytes| bytes.is_empty())
    {
        return Err(
            control_plane::errors::ControlPlaneError::InvalidInput("extension_file").into(),
        );
    }
    Ok(fields)
}

fn explicit_value(value: Option<&String>, field: &'static str) -> Result<String, ApiError> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(field).into())
}

fn ensure_explicit_match(
    explicit: Option<&String>,
    actual: &str,
    field: &'static str,
) -> Result<(), ApiError> {
    if explicit.is_some_and(|value| value.trim() != actual) {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(field).into());
    }
    Ok(())
}

fn parse_mcp_upload_manifest(bytes: &[u8]) -> Result<Option<domain::McpBundleManifest>, ApiError> {
    if !bytes.starts_with(b"PK") {
        return Ok(None);
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| {
        control_plane::errors::ControlPlaneError::InvalidInput("extension_upload_archive")
    })?;
    let Ok(mut manifest_file) = archive.by_name("manifest.json") else {
        return Ok(None);
    };
    let mut manifest_bytes = Vec::new();
    manifest_file
        .read_to_end(&mut manifest_bytes)
        .map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("extension_upload_manifest")
        })?;
    let manifest =
        serde_json::from_slice::<domain::McpBundleManifest>(&manifest_bytes).map_err(|_| {
            control_plane::errors::ControlPlaneError::InvalidInput("extension_upload_manifest")
        })?;
    if manifest.schema_version != domain::MCP_BUNDLE_SCHEMA_VERSION {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_upload_manifest",
        )
        .into());
    }
    Ok(Some(manifest))
}

async fn classify_uploaded_extension(
    state: &ApiState,
    fields: &ExtensionUploadFields,
    file_name: &str,
    artifact_bytes: &[u8],
) -> Result<UploadedExtensionArtifact, ApiError> {
    let explicit_category = fields
        .category
        .as_deref()
        .map(ExtensionCatalogCategory::parse)
        .transpose()?;
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(artifact_bytes) {
        if value
            .get("manifest")
            .and_then(|manifest| manifest.get("schema_version"))
            .and_then(serde_json::Value::as_str)
            == Some(domain::I18N_CATALOG_SEED_SCHEMA_VERSION)
        {
            let category = ExtensionCatalogCategory::I18n;
            if explicit_category.is_some_and(|value| value != category) {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "extension_catalog_category",
                )
                .into());
            }
            let version = value
                .get("manifest")
                .and_then(|manifest| manifest.get("catalog_version"))
                .and_then(serde_json::Value::as_str)
                .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                    "extension_version",
                ))?
                .to_string();
            ensure_explicit_match(fields.version.as_ref(), &version, "extension_version")?;
            return Ok(UploadedExtensionArtifact {
                category,
                organization: explicit_value(
                    fields.organization.as_ref(),
                    "extension_organization",
                )?,
                artifact_id: explicit_value(fields.artifact_id.as_ref(), "extension_artifact_id")?,
                version,
                minimum_host_version: None,
                node_plugin: false,
                signature_status: domain::ExtensionSignatureStatus::Missing,
                signature_algorithm: None,
                signing_key_id: None,
                application_action: domain::ExtensionApplicationAction::ActivateI18n,
            });
        }
        if let Ok(template) =
            serde_json::from_value::<control_plane::flow::AgentFlowTemplatePackage>(value)
        {
            if template.schema_version != control_plane::flow::AGENT_FLOW_TEMPLATE_SCHEMA_VERSION {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "extension_upload_manifest",
                )
                .into());
            }
            let category = ExtensionCatalogCategory::AgentFlow;
            if explicit_category.is_some_and(|value| value != category) {
                return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                    "extension_catalog_category",
                )
                .into());
            }
            return Ok(UploadedExtensionArtifact {
                category,
                organization: explicit_value(
                    fields.organization.as_ref(),
                    "extension_organization",
                )?,
                artifact_id: explicit_value(fields.artifact_id.as_ref(), "extension_artifact_id")?,
                version: explicit_value(fields.version.as_ref(), "extension_version")?,
                minimum_host_version: None,
                node_plugin: false,
                signature_status: domain::ExtensionSignatureStatus::Missing,
                signature_algorithm: None,
                signing_key_id: None,
                application_action: domain::ExtensionApplicationAction::ImportAgentFlow,
            });
        }
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_upload_manifest",
        )
        .into());
    }

    let mcp_bytes = artifact_bytes.to_vec();
    if let Some(manifest) =
        tokio::task::spawn_blocking(move || parse_mcp_upload_manifest(&mcp_bytes))
            .await
            .map_err(|_| {
                control_plane::errors::ControlPlaneError::UpstreamUnavailable(
                    "extension_upload_inspection",
                )
            })??
    {
        let category = ExtensionCatalogCategory::Mcp;
        if explicit_category.is_some_and(|value| value != category) {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "extension_catalog_category",
            )
            .into());
        }
        ensure_explicit_match(
            fields.organization.as_ref(),
            &manifest.organization,
            "extension_organization",
        )?;
        ensure_explicit_match(
            fields.artifact_id.as_ref(),
            &manifest.bundle_id,
            "extension_artifact_id",
        )?;
        ensure_explicit_match(
            fields.version.as_ref(),
            &manifest.bundle_version,
            "extension_version",
        )?;
        return Ok(UploadedExtensionArtifact {
            category,
            organization: manifest.organization,
            artifact_id: manifest.bundle_id,
            version: manifest.bundle_version,
            minimum_host_version: Some(manifest.minimum_host_version),
            node_plugin: false,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            application_action: domain::ExtensionApplicationAction::ImportMcp,
        });
    }

    let inspection = inspect_node_plugin(state, file_name, artifact_bytes, "uploaded").await?;
    if explicit_category.is_some_and(|value| value != inspection.category) {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "extension_catalog_category",
        )
        .into());
    }
    ensure_explicit_match(
        fields.organization.as_ref(),
        &inspection.organization,
        "extension_organization",
    )?;
    ensure_explicit_match(
        fields.artifact_id.as_ref(),
        &inspection.artifact_id,
        "extension_artifact_id",
    )?;
    ensure_explicit_match(
        fields.version.as_ref(),
        &inspection.version,
        "extension_version",
    )?;
    Ok(UploadedExtensionArtifact {
        category: inspection.category,
        organization: inspection.organization,
        artifact_id: inspection.artifact_id,
        version: inspection.version,
        minimum_host_version: Some(inspection.minimum_host_version),
        node_plugin: true,
        signature_status: inspection.signature_status,
        signature_algorithm: inspection.signature_algorithm,
        signing_key_id: inspection.signing_key_id,
        application_action: inspection.application_action,
    })
}

pub(super) fn upload_challenge(
    artifact: &UploadedExtensionArtifact,
) -> ExtensionRiskChallengeResponse {
    let mut warnings = vec![risk_warning(
        "checksum_missing",
        "The artifact does not include an expected checksum.",
    )];
    match artifact.signature_status {
        domain::ExtensionSignatureStatus::Verified => {}
        domain::ExtensionSignatureStatus::Missing => warnings.push(risk_warning(
            "signature_missing",
            "The artifact does not include a verifiable signature.",
        )),
        domain::ExtensionSignatureStatus::UnknownKey => warnings.push(risk_warning(
            "signing_key_unknown",
            "The artifact was signed by a key that is not configured as trusted.",
        )),
        domain::ExtensionSignatureStatus::Invalid => warnings.push(risk_warning(
            "signature_invalid",
            "The artifact signature is invalid.",
        )),
    }
    warnings.sort_by(|left, right| left.code.cmp(&right.code));
    ExtensionRiskChallengeResponse {
        warnings,
        compatibility: artifact
            .minimum_host_version
            .as_deref()
            .and_then(compatibility_for_requirement),
    }
}

async fn install_uploaded_artifact(
    state: &ApiState,
    actor_user_id: Uuid,
    mut fields: ExtensionUploadFields,
) -> Result<Response, ApiError> {
    let file_name = fields
        .file_name
        .take()
        .unwrap_or_else(|| "extension-upload.bin".to_string());
    let artifact_bytes = fields.artifact_bytes.take().ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("extension_file"),
    )?;
    let artifact = classify_uploaded_extension(state, &fields, &file_name, &artifact_bytes).await?;
    let identity = extension_identity(
        artifact.category,
        &artifact.organization,
        &artifact.artifact_id,
        &artifact.version,
        &state.api_node_id,
    )?;
    let install_service = extension_installation_service(state);
    if let Some(installation) = install_service.find_local_installation(&identity).await? {
        let node_plugin_installation_id = if artifact.node_plugin {
            Some(
                register_node_plugin(
                    state,
                    actor_user_id,
                    "extension_center.install.upload",
                    artifact.category,
                    &installation,
                    file_name,
                    "uploaded".to_string(),
                )
                .await?,
            )
        } else {
            None
        };
        return Ok((
            StatusCode::OK,
            Json(ApiSuccess::new(ExtensionInstallResponse {
                application_action: installation.application_action.as_str().to_string(),
                application_status: default_application_status(installation.application_action)
                    .to_string(),
                installation: to_local_inventory_entry(installation),
                local_artifact_was_present: true,
                node_plugin_installation_id,
            })),
        )
            .into_response());
    }
    let challenge = upload_challenge(&artifact);
    let declared_warnings = challenge
        .warnings
        .iter()
        .map(|warning| domain::ExtensionIntegrityWarning {
            code: warning.code.clone(),
            message: warning.message.clone(),
            overridable: warning.overridable,
        })
        .collect();
    let confirmation_receipt = match validate_preflight_overrides(
        &challenge,
        fields.risk_override.as_ref(),
        fields.compatibility_override.as_ref(),
    )? {
        PreflightDecision::Challenge => return Ok(challenge_response(challenge)),
        PreflightDecision::Accepted(receipt) => receipt,
    };
    let risk_override = fields.risk_override.map(|value| ExtensionRiskOverride {
        reason: value.reason,
        acknowledged_warnings: value.acknowledged_warnings,
    });
    let outcome = install_service
        .install_from_bytes(InstallExtensionArtifactCommand {
            actor_user_id,
            category: artifact.category,
            organization: artifact.organization,
            artifact_id: artifact.artifact_id,
            version: artifact.version,
            node_id: state.api_node_id.clone(),
            artifact_bytes,
            source: "upload".to_string(),
            trust: if artifact.signature_status == domain::ExtensionSignatureStatus::Verified {
                "trusted".to_string()
            } else {
                "unknown".to_string()
            },
            expected_checksum: None,
            signature_status: artifact.signature_status,
            signature_algorithm: artifact.signature_algorithm,
            signing_key_id: artifact.signing_key_id,
            declared_warnings,
            risk_override,
            confirmation_receipt,
            application_action: artifact.application_action,
        })
        .await?;
    let (installation, local_artifact_was_present) = match outcome {
        ExtensionArtifactInstallOutcome::RiskConfirmationRequired { risk_challenge } => {
            return Ok(domain_challenge_response(
                risk_challenge,
                challenge.compatibility,
            ));
        }
        ExtensionArtifactInstallOutcome::Installed {
            installation,
            local_artifact_was_present,
        } => (installation, local_artifact_was_present),
    };
    let node_plugin_installation_id = if artifact.node_plugin {
        Some(
            register_node_plugin(
                state,
                actor_user_id,
                "extension_center.install.upload",
                artifact.category,
                &installation,
                file_name,
                "uploaded".to_string(),
            )
            .await?,
        )
    } else {
        None
    };
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(ExtensionInstallResponse {
            application_action: installation.application_action.as_str().to_string(),
            application_status: default_application_status(installation.application_action)
                .to_string(),
            installation: to_local_inventory_entry(installation),
            local_artifact_was_present,
            node_plugin_installation_id,
        })),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/console/settings/extension-center/install-upload",
    operation_id = "extension_center_install_upload",
    request_body(content = inline(ExtensionUploadMultipartBody), content_type = "multipart/form-data"),
    responses((status = 201, body = ExtensionInstallResponse), (status = 409, body = ExtensionRiskChallengeErrorResponse), (status = 400, body = crate::error_response::ErrorBody))
)]
pub async fn install_uploaded_extension(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let fields = read_extension_upload(&mut multipart).await?;
    install_uploaded_artifact(&state, context.user.id, fields).await
}
