use super::*;

pub(super) const MODEL_PROVIDER_PLUGIN_TYPE: &str = "model_provider";

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/plugins/families",
    params(PluginCatalogQuery),
    operation_id = "model_provider_settings_list_plugin_families",
    responses((status = 200, body = PluginFamilyCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_families(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<PluginCatalogQuery>,
) -> Result<Json<ApiSuccess<PluginFamilyCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.families.v1",
        interface::PluginInterfaceInput::ModelFamilies { query, locale },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Families(families) = output else {
        unreachable!("model plugin families binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(families)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/plugins/official-catalog",
    params(OfficialPluginCatalogQuery),
    operation_id = "model_provider_settings_list_official_plugins",
    responses((status = 200, body = OfficialPluginCatalogResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_official_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<OfficialPluginCatalogQuery>,
) -> Result<Json<ApiSuccess<OfficialPluginCatalogResponse>>, ApiError> {
    let locale = crate::routes::console_interface::ConsoleLocaleHints::from_headers(&headers);
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.official-catalog.v1",
        interface::PluginInterfaceInput::ModelOfficial { query, locale },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Official(catalog) = output else {
        unreachable!("model plugin catalog binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(catalog)))
}

pub(super) fn project_model_provider_catalog_entry(
    source: &dyn crate::official_extension_catalog::OfficialExtensionCatalogSourcePort,
    entry: crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    installed: &std::collections::HashMap<String, &'static str>,
) -> anyhow::Result<Option<OfficialPluginCatalogEntryResponse>> {
    if catalog_metadata_optional(&entry, "plugin_type").as_deref()
        != Some(MODEL_PROVIDER_PLUGIN_TYPE)
    {
        return Ok(None);
    }
    let plugin_id = catalog_metadata_required(&entry, "plugin_id")?;
    let provider_code = catalog_metadata_required(&entry, "provider_code")?;
    let protocol = catalog_metadata_required(&entry, "protocol")?;
    let model_discovery_mode = catalog_metadata_required(&entry, "model_discovery_mode")?;
    let icon = catalog_metadata_optional(&entry, "icon");
    let help_url = catalog_metadata_optional(&entry, "help_url");
    let descriptor = source.resolve_artifact(&entry)?;
    let checksum = descriptor.expected_checksum.ok_or_else(|| {
        anyhow::anyhow!("official model-provider catalog entry is missing checksum")
    })?;
    let platform = descriptor.platform.ok_or_else(|| {
        anyhow::anyhow!("official model-provider catalog entry has no current-platform artifact")
    })?;
    let signature_algorithm = descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("algorithm"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let signing_key_id = descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let current_host_version = control_plane::plugin_management::current_plugin_host_version();
    let compatibility = control_plane::plugin_management::official_plugin_host_compatibility(
        &entry.host_version_requirement,
        &current_host_version,
    );
    let install_status = exact_catalog_install_status(installed, &entry.id).to_string();
    Ok(Some(OfficialPluginCatalogEntryResponse {
        plugin_id,
        plugin_type: MODEL_PROVIDER_PLUGIN_TYPE.to_string(),
        provider_code: provider_code.clone(),
        display_name: entry.name,
        description: (!entry.description.trim().is_empty()).then_some(entry.description),
        icon,
        protocol,
        latest_version: entry.version,
        minimum_host_version: compatibility.minimum_host_version,
        current_host_version: compatibility.current_host_version,
        compatibility_status: compatibility.status,
        compatibility_warning_reason: compatibility.warning_reason,
        selected_artifact: OfficialPluginArtifactResponse {
            os: platform.os,
            arch: platform.arch,
            libc: platform.libc,
            rust_target: platform.rust_target,
            download_url: descriptor.locator,
            checksum,
            signature_algorithm,
            signing_key_id,
        },
        help_url,
        model_discovery_mode,
        install_status,
    }))
}

pub(super) fn model_provider_catalog_id(installation: &domain::PluginInstallationRecord) -> String {
    canonical_model_provider_catalog_id(
        installation.category,
        &installation.organization,
        &installation.provider_code,
    )
}

fn canonical_model_provider_catalog_id(
    category: domain::ExtensionCategory,
    organization: &str,
    provider_code: &str,
) -> String {
    format!("{}:{}/{}", category.as_str(), organization, provider_code)
}

fn exact_catalog_install_status<'a>(
    installed: &'a std::collections::HashMap<String, &'static str>,
    catalog_id: &str,
) -> &'a str {
    installed
        .get(catalog_id)
        .copied()
        .unwrap_or("not_installed")
}

pub(super) fn model_provider_catalog_install_status(
    artifact_status: domain::PluginArtifactInstanceStatus,
    assigned_to_current_workspace: bool,
) -> &'static str {
    if artifact_status == domain::PluginArtifactInstanceStatus::Missing {
        "uninstalled"
    } else if assigned_to_current_workspace {
        "assigned"
    } else {
        "installed"
    }
}

#[cfg(test)]
mod catalog_identity_tests {
    use super::{
        canonical_model_provider_catalog_id, exact_catalog_install_status,
        model_provider_catalog_install_status,
    };
    use std::collections::HashMap;

    #[test]
    fn api_f2_same_provider_code_from_another_publisher_is_not_marked_installed() {
        let installed_catalog_id = canonical_model_provider_catalog_id(
            domain::ExtensionCategory::RuntimeExtensions,
            "publisher-a",
            "shared-provider",
        );
        let other_publisher_catalog_id = canonical_model_provider_catalog_id(
            domain::ExtensionCategory::RuntimeExtensions,
            "publisher-b",
            "shared-provider",
        );
        let installed = HashMap::from([(installed_catalog_id.clone(), "assigned")]);

        assert_eq!(
            exact_catalog_install_status(&installed, &installed_catalog_id),
            "assigned"
        );
        assert_eq!(
            exact_catalog_install_status(&installed, &other_publisher_catalog_id),
            "not_installed"
        );
    }

    #[test]
    fn ac_1785_only_a_missing_artifact_projects_as_uninstalled() {
        assert_eq!(
            model_provider_catalog_install_status(
                domain::PluginArtifactInstanceStatus::Missing,
                true,
            ),
            "uninstalled"
        );
        assert_eq!(
            model_provider_catalog_install_status(
                domain::PluginArtifactInstanceStatus::LoadFailed,
                true,
            ),
            "assigned"
        );
    }
}

fn catalog_metadata_required(
    entry: &crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    field: &'static str,
) -> anyhow::Result<String> {
    catalog_metadata_optional(entry, field)
        .ok_or_else(|| anyhow::anyhow!("official model-provider catalog entry is missing {field}"))
}

fn catalog_metadata_optional(
    entry: &crate::official_extension_catalog::OfficialExtensionCatalogEntry,
    field: &str,
) -> Option<String> {
    entry
        .source
        .metadata
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/plugins/install-official",
    operation_id = "model_provider_settings_install_official_plugin",
    request_body = InstallOfficialPluginBody,
    responses((status = 201, body = InstallPluginResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_official_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<InstallOfficialPluginBody>,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.install-official.v1",
        interface::PluginInterfaceInput::ModelInstallOfficial(body),
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Installed(result) = output else {
        unreachable!("model plugin install binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(result))))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/plugins/install-upload",
    operation_id = "model_provider_settings_install_uploaded_plugin",
    request_body(content = inline(PluginUploadMultipartBody), content_type = "multipart/form-data"),
    responses((status = 201, body = InstallPluginResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn install_uploaded_plugin(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<ApiSuccess<InstallPluginResponse>>), ApiError> {
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.install-upload.v1",
        interface::PluginInterfaceInput::ModelInstallUploaded {
            file_name,
            package_bytes,
        },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Installed(result) = output else {
        unreachable!("model plugin upload binding returned a different output")
    };
    Ok((StatusCode::CREATED, Json(ApiSuccess::new(result))))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/plugins/{installation_id}/artifact/refresh",
    operation_id = "model_provider_settings_refresh_plugin_artifact",
    responses((status = 200, body = PluginArtifactInstanceResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn refresh_current_node_artifact(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginArtifactInstanceResponse>>, ApiError> {
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.artifact-refresh.v1",
        interface::PluginInterfaceInput::ModelRefreshArtifact { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Artifact(artifact) = output else {
        unreachable!("model plugin refresh binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(artifact)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/plugins/{installation_id}/artifact/install-current-node",
    operation_id = "model_provider_settings_install_plugin_artifact",
    responses((status = 200, body = PluginArtifactInstanceResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn install_current_node_artifact(
    State(state): State<Arc<ApiState>>,
    Path(installation_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginArtifactInstanceResponse>>, ApiError> {
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.artifact-install.v1",
        interface::PluginInterfaceInput::ModelInstallArtifact { installation_id },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Artifact(artifact) = output else {
        unreachable!("model plugin artifact install binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(artifact)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/plugins/families/{provider_code}/upgrade-latest",
    operation_id = "model_provider_settings_upgrade_plugin_family",
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn upgrade_latest(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    body: Option<Json<UpgradeLatestPluginFamilyBody>>,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let body = body.map(|Json(body)| body);
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.family-upgrade.v1",
        interface::PluginInterfaceInput::ModelUpgradeLatest {
            provider_code,
            body,
        },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("model plugin upgrade binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/model-providers/plugins/families/{provider_code}/switch-version",
    operation_id = "model_provider_settings_switch_plugin_family_version",
    request_body = SwitchPluginVersionBody,
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn switch_version(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
    Json(body): Json<SwitchPluginVersionBody>,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.family-switch.v1",
        interface::PluginInterfaceInput::ModelSwitchVersion {
            provider_code,
            body,
        },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("model plugin switch binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    delete,
    path = "/api/console/settings/model-providers/plugins/families/{provider_code}",
    operation_id = "model_provider_settings_delete_plugin_family",
    responses((status = 200, body = PluginTaskResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn delete_family(
    State(state): State<Arc<ApiState>>,
    Path(provider_code): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.family-delete.v1",
        interface::PluginInterfaceInput::ModelDeleteFamily { provider_code },
        true,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("model plugin delete binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/plugins/tasks/{task_id}",
    operation_id = "model_provider_settings_get_plugin_task",
    responses((status = 200, body = PluginTaskResponse), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_task(
    State(state): State<Arc<ApiState>>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<PluginTaskResponse>>, ApiError> {
    let output = super::invoke_plugin_interface(
        state,
        headers,
        "http.console.model-provider-plugins.task.v1",
        interface::PluginInterfaceInput::ModelGetTask { task_id },
        false,
    )
    .await?;
    let interface::PluginInterfaceOutput::Task(task) = output else {
        unreachable!("model plugin task binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(task)))
}
