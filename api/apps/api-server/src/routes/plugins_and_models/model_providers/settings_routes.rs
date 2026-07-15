use super::*;

use crate::routes::console_route_assembly::{
    console_get, console_patch, console_post, ConsoleRouteAssembly,
};

pub(super) fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/model-providers/catalog",
            console_get(
                list_catalog,
                ConsoleOperation("model_providers.catalog.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/request-logs",
            console_get(
                list_request_logs,
                ConsoleOperation("model_providers.request_logs.view".to_string()),
            )
            .delete(
                delete_selected_request_logs,
                ConsoleOperation("model_providers.request_logs.delete".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/request-logs/clear",
            console_post(
                clear_request_logs_batch,
                ConsoleOperation("model_providers.request_logs.clear".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances",
            console_get(
                list_instances,
                ConsoleOperation("model_providers.instances.view".to_string()),
            )
            .post(
                create_instance,
                ConsoleOperation("model_providers.instances.create".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/providers/:provider_code/main-instance",
            console_get(
                get_main_instance,
                ConsoleOperation("model_providers.main_instance.view".to_string()),
            )
            .put(
                update_main_instance,
                ConsoleOperation("model_providers.main_instance.update".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/preview-models",
            console_post(
                preview_models,
                ConsoleOperation("model_providers.preview.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/options",
            console_get(
                list_settings_options,
                ConsoleOperation("model_providers.settings_options.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id",
            console_patch(
                update_instance,
                ConsoleOperation("model_providers.instances.update".to_string()),
            )
            .delete(
                delete_instance,
                ConsoleOperation("model_providers.instances.delete".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/validate",
            console_post(
                validate_instance,
                ConsoleOperation("model_providers.instances.validate".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/secrets/reveal",
            console_post(
                reveal_secret,
                ConsoleOperation("model_providers.instances.secrets.reveal".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/models",
            console_get(
                list_models,
                ConsoleOperation("model_providers.instances.models.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/models/refresh",
            console_post(
                refresh_models,
                ConsoleOperation("model_providers.instances.models.refresh".to_string()),
            ),
        )
}

pub(super) fn settings_service(
    state: &ApiState,
    operation_id: &'static str,
) -> ModelProviderService<MainDurableStore, ApiProviderRuntime> {
    ModelProviderService::for_console_operation(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.provider_secret_master_key.clone(),
        domain::ConsolePolicyGroup::settings_feature("system.model-providers")
            .expect("compiled model-provider settings group must be valid"),
        operation_id,
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
    let options = settings_service(&state, "model_providers.settings_options.view")
        .options(context.user.id, requested_locales(&locale_meta))
        .await?;
    Ok(Json(ApiSuccess::new(to_options_view_response(
        locale_meta,
        options,
    ))))
}
