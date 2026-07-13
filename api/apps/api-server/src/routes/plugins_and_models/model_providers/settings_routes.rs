use super::*;

pub(super) fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/settings/model-providers/catalog", get(list_catalog))
        .route(
            "/settings/model-providers/request-logs",
            get(list_request_logs).delete(delete_selected_request_logs),
        )
        .route(
            "/settings/model-providers/request-logs/clear",
            post(clear_request_logs_batch),
        )
        .route(
            "/settings/model-providers/instances",
            get(list_instances).post(create_instance),
        )
        .route(
            "/settings/model-providers/providers/:provider_code/main-instance",
            get(get_main_instance).put(update_main_instance),
        )
        .route(
            "/settings/model-providers/preview-models",
            post(preview_models),
        )
        .route(
            "/settings/model-providers/options",
            get(list_settings_options),
        )
        .route(
            "/settings/model-providers/instances/:id",
            patch(update_instance).delete(delete_instance),
        )
        .route(
            "/settings/model-providers/instances/:id/validate",
            post(validate_instance),
        )
        .route(
            "/settings/model-providers/instances/:id/secrets/reveal",
            post(reveal_secret),
        )
        .route(
            "/settings/model-providers/instances/:id/models",
            get(list_models),
        )
        .route(
            "/settings/model-providers/instances/:id/models/refresh",
            post(refresh_models),
        )
}

pub(super) fn settings_service(
    state: &ApiState,
) -> ModelProviderService<MainDurableStore, ApiProviderRuntime> {
    ModelProviderService::for_model_provider_settings(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.provider_secret_master_key.clone(),
    )
    .with_node_artifact_context(
        state.api_node_id.clone(),
        state.provider_install_root.clone(),
    )
}

#[utoipa::path(
    get,
    path = "/api/console/settings/model-providers/options",
    operation_id = "model_provider_settings_list_options",
    params(ModelProviderCatalogQuery),
    responses((status = 200, body = ModelProviderOptionsResponse), (status = 401, body = crate::error_response::ErrorBody))
)]
pub async fn list_settings_options(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<ModelProviderCatalogQuery>,
) -> Result<Json<ApiSuccess<ModelProviderOptionsResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let locale_meta = resolve_locale_meta(&headers, query.locale, context.user.preferred_locale);
    let options = settings_service(&state)
        .options(context.user.id, requested_locales(&locale_meta))
        .await?;
    Ok(Json(ApiSuccess::new(to_options_view_response(
        locale_meta,
        options,
    ))))
}
