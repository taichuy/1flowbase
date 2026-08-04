use super::*;

const MODEL_PROVIDER_PLUGIN_TYPE: &str = "model_provider";

fn service(
    state: &ApiState,
    operation_id: &'static str,
) -> PluginManagementService<MainDurableStore, ApiProviderRuntime> {
    super::base_service(state).for_model_provider_console_operation(operation_id)
}

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
    Query(mut query): Query<PluginCatalogQuery>,
) -> Result<Json<ApiSuccess<PluginFamilyCatalogResponse>>, ApiError> {
    query.plugin_type = Some(MODEL_PROVIDER_PLUGIN_TYPE.to_string());
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(
        &headers,
        query.locale.clone(),
        context.user.preferred_locale,
    );
    let families = service(&state, "model_provider_plugins.families.view")
        .list_families(
            context.user.id,
            filter_from_query(&query),
            requested_locales(&locale_meta),
        )
        .await?;
    let i18n_catalog = serde_json::to_value(families.i18n_catalog)?;
    Ok(Json(ApiSuccess::new(PluginFamilyCatalogResponse {
        locale_meta,
        i18n_catalog,
        entries: families
            .entries
            .into_iter()
            .map(to_family_response)
            .collect(),
    })))
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
    Query(mut query): Query<OfficialPluginCatalogQuery>,
) -> Result<Json<ApiSuccess<OfficialPluginCatalogResponse>>, ApiError> {
    query.plugin_type = Some(MODEL_PROVIDER_PLUGIN_TYPE.to_string());
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(
        &headers,
        query.locale.clone(),
        context.user.preferred_locale,
    );
    let local_catalog = service(&state, "model_provider_plugins.official_catalog.view")
        .list_catalog(
            context.user.id,
            filter_from_query(&PluginCatalogQuery {
                plugin_type: query.plugin_type.clone(),
                locale: query.locale.clone(),
            }),
            requested_locales(&locale_meta),
        )
        .await?;
    let filter = official_filter_from_query(&query);
    let page = state
        .official_extension_catalog_source
        .search(
            "runtime-extensions",
            crate::official_extension_catalog::OfficialExtensionCatalogSearchQuery {
                slot_code: Some(MODEL_PROVIDER_PLUGIN_TYPE.to_string()),
                q: filter.search_query.clone(),
                limit: filter.limit,
                cursor: query.cursor.clone(),
            },
        )
        .await?;
    let installed = local_catalog
        .entries
        .into_iter()
        .map(|entry| {
            (
                model_provider_catalog_id(&entry.installation),
                if entry.assigned_to_current_workspace {
                    "assigned"
                } else {
                    "installed"
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let entries = page
        .entries
        .into_iter()
        .filter_map(
            |entry| match project_model_provider_catalog_entry(&state, entry, &installed) {
                Ok(Some(entry)) => Some(Ok(entry)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect::<anyhow::Result<Vec<_>>>()?;
    let locale = domain::CatalogLocale::new(locale_meta.resolved_locale.clone())
        .expect("runtime profile must resolve a supported catalog locale");
    let source_label = crate::app_state::resolve_official_source_label(
        &state,
        &locale,
        &page.source_kind,
        page.source_kind.clone(),
    )
    .await?;
    Ok(Json(ApiSuccess::new(OfficialPluginCatalogResponse {
        source_kind: page.source_kind,
        source_label,
        registry_url: page.snapshot_locator,
        source_freshness: "fresh".to_string(),
        locale_meta,
        page: OfficialPluginCatalogPageResponse {
            limit: filter.limit,
            next_cursor: page.next_cursor,
        },
        entries,
    })))
}

fn project_model_provider_catalog_entry(
    state: &ApiState,
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
    let descriptor = state
        .official_extension_catalog_source
        .resolve_artifact(&entry)?;
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

fn model_provider_catalog_id(installation: &domain::PluginInstallationRecord) -> String {
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

#[cfg(test)]
mod catalog_identity_tests {
    use super::{canonical_model_provider_catalog_id, exact_catalog_install_status};
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let command = resolved_official_plugin_install_command(
        &state,
        context.user.id,
        body.plugin_id,
        to_compatibility_override(body.compatibility_override),
        to_risk_override(body.risk_override),
    )
    .await?;
    let result = service(&state, "model_provider_plugins.install.official")
        .install_resolved_official_plugin(command)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_install_response(result))),
    ))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let (file_name, package_bytes) = read_upload_file(&mut multipart).await?;
    let result = service(&state, "model_provider_plugins.install.upload")
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: context.user.id,
            file_name,
            package_bytes,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiSuccess::new(to_install_response(result))),
    ))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let artifact = service(&state, "model_provider_plugins.artifact.refresh")
        .refresh_current_node_artifact(RefreshCurrentNodePluginArtifactCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_artifact_instance_response(
        artifact,
    ))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let artifact = service(&state, "model_provider_plugins.artifact.install")
        .install_current_node_artifact(InstallCurrentNodePluginArtifactCommand {
            actor_user_id: context.user.id,
            installation_id: parse_uuid(&installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_artifact_instance_response(
        artifact,
    ))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let body = body.map(|Json(body)| body);
    let compatibility_override = body
        .as_ref()
        .and_then(|body| to_compatibility_override(body.compatibility_override.clone()));
    let risk_override = body.and_then(|body| to_risk_override(body.risk_override));
    let task = service(&state, "model_provider_plugins.families.upgrade")
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: context.user.id,
            provider_code,
            compatibility_override,
            risk_override,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "model_provider_plugins.families.switch")
        .switch_version(SwitchPluginVersionCommand {
            actor_user_id: context.user.id,
            provider_code,
            target_installation_id: parse_uuid(&body.installation_id, "installation_id")?,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    let task = service(&state, "model_provider_plugins.families.delete")
        .delete_family(DeletePluginFamilyCommand {
            actor_user_id: context.user.id,
            provider_code,
        })
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
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
    let context = require_session(&state, &headers).await?;
    let task = service(&state, "model_provider_plugins.tasks.view")
        .get_task(context.user.id, parse_uuid(&task_id, "task_id")?)
        .await?;
    Ok(Json(ApiSuccess::new(to_task_response(task))))
}
