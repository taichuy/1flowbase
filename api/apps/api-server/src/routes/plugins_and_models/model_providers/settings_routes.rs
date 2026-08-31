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
            "/settings/model-providers/instances/:id/authenticate",
            console_post(
                authenticate_instance,
                ConsoleOperation("model_providers.instances.authenticate".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/usage",
            console_get(
                get_usage_windows,
                ConsoleOperation("model_providers.instances.usage.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/reset-credits",
            console_get(
                count_reset_credits,
                ConsoleOperation("model_providers.instances.reset_credits.view".to_string()),
            ),
        )
        .route(
            "/settings/model-providers/instances/:id/reset-credits/consume",
            console_post(
                consume_reset_credit,
                ConsoleOperation("model_providers.instances.reset_credits.consume".to_string()),
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
    let locale = catalog_logs_interface::ProviderLocaleHints::from_headers(&headers);
    let output = crate::routes::console_interface::invoke(
        Arc::clone(&state),
        "http.console.model-providers.settings-options.view.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        discovery_interface::ProviderDiscoveryInput::Options {
            query,
            locale,
            settings: true,
        },
    )
    .await?;
    let discovery_interface::ProviderDiscoveryOutput::Options(response) = output else {
        unreachable!("provider settings options binding returned a different output")
    };
    Ok(Json(ApiSuccess::new(response)))
}
