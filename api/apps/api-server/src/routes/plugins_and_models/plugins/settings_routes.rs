use super::*;

const MODEL_PROVIDER_PLUGIN_TYPE: &str = "model_provider";

pub(super) fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/settings/model-providers/plugins/families",
            get(list_families),
        )
        .route(
            "/settings/model-providers/plugins/official-catalog",
            get(list_official_catalog),
        )
        .route(
            "/settings/model-providers/plugins/install-official",
            post(install_official_plugin),
        )
        .route(
            "/settings/model-providers/plugins/install-upload",
            post(install_uploaded_plugin),
        )
        .route(
            "/settings/model-providers/plugins/:installation_id/artifact/refresh",
            post(refresh_current_node_artifact),
        )
        .route(
            "/settings/model-providers/plugins/:installation_id/artifact/install-current-node",
            post(install_current_node_artifact),
        )
        .route(
            "/settings/model-providers/plugins/families/:provider_code/upgrade-latest",
            post(upgrade_latest),
        )
        .route(
            "/settings/model-providers/plugins/families/:provider_code/switch-version",
            post(switch_version),
        )
        .route(
            "/settings/model-providers/plugins/families/:provider_code",
            delete(delete_family),
        )
        .route(
            "/settings/model-providers/plugins/tasks/:task_id",
            get(get_task),
        )
}

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
    let catalog = service(&state, "model_provider_plugins.official_catalog.view")
        .list_official_catalog(
            context.user.id,
            official_filter_from_query(&query),
            requested_locales(&locale_meta),
        )
        .await?;
    Ok(Json(ApiSuccess::new(to_official_catalog_response(
        locale_meta,
        catalog,
    ))))
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
    let result = service(&state, "model_provider_plugins.install.official")
        .install_official_plugin(InstallOfficialPluginCommand {
            actor_user_id: context.user.id,
            plugin_id: body.plugin_id,
            compatibility_override: to_compatibility_override(body.compatibility_override),
        })
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
    let task = service(&state, "model_provider_plugins.families.upgrade")
        .upgrade_latest(UpgradeLatestPluginFamilyCommand {
            actor_user_id: context.user.id,
            provider_code,
            compatibility_override: body
                .map(|Json(body)| body.compatibility_override)
                .and_then(to_compatibility_override),
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
