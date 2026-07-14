use std::sync::Arc;

use access_control::{
    ConsoleRouteOwnership::ConsoleOperation, DATA_SOURCES_CREATE_OPERATION_ID,
    DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID, DATA_SOURCES_DISCOVER_OPERATION_ID,
    DATA_SOURCES_LIST_OPERATION_ID, DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID,
    DATA_SOURCES_PREVIEW_OPERATION_ID, DATA_SOURCES_VALIDATE_OPERATION_ID,
    DATA_SOURCES_VIEW_OPERATION_ID, MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID,
    MODEL_DEFINITIONS_CREATE_OPERATION_ID, MODEL_DEFINITIONS_DELETE_OPERATION_ID,
    MODEL_DEFINITIONS_LIST_OPERATION_ID, MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID,
    MODEL_DEFINITIONS_UPDATE_OPERATION_ID, MODEL_FIELDS_CREATE_OPERATION_ID,
    MODEL_FIELDS_DELETE_OPERATION_ID, MODEL_FIELDS_UPDATE_OPERATION_ID,
    MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID, MODEL_SCOPE_GRANTS_LIST_OPERATION_ID,
    MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID, SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION,
};
use axum::Router;

use crate::{
    app_state::ApiState,
    routes::{
        console_route_assembly::{ConsoleRouteAssembly, console_get, console_patch, console_post},
        data_sources, docs, model_definitions,
    },
};

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    ConsoleRouteAssembly::new()
        .route(
            "/settings/data-models/data-sources/catalog",
            console_get(
                data_sources::list_catalog,
                ConsoleOperation(SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources",
            console_get(
                data_sources::list_data_sources,
                ConsoleOperation(DATA_SOURCES_LIST_OPERATION_ID.to_string()),
            )
            .post(
                data_sources::create_data_source,
                ConsoleOperation(DATA_SOURCES_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/defaults",
            console_patch(
                data_sources::update_defaults,
                ConsoleOperation(DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/validate",
            console_post(
                data_sources::validate_data_source,
                ConsoleOperation(DATA_SOURCES_VALIDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/resources",
            console_get(
                data_sources::list_resources,
                ConsoleOperation(DATA_SOURCES_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/resources/discover",
            console_post(
                data_sources::discover_resources,
                ConsoleOperation(DATA_SOURCES_DISCOVER_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/preview-read",
            console_post(
                data_sources::preview_read,
                ConsoleOperation(DATA_SOURCES_PREVIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/resources/map-to-model",
            console_post(
                data_sources::map_resource_to_model,
                ConsoleOperation(DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions",
            console_get(
                model_definitions::list_models,
                ConsoleOperation(MODEL_DEFINITIONS_LIST_OPERATION_ID.to_string()),
            )
            .post(
                model_definitions::create_model,
                ConsoleOperation(MODEL_DEFINITIONS_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions:batchDelete",
            console_post(
                model_definitions::batch_delete_models,
                ConsoleOperation(MODEL_DEFINITIONS_DELETE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:id",
            console_patch(
                model_definitions::update_model,
                ConsoleOperation(MODEL_DEFINITIONS_UPDATE_OPERATION_ID.to_string()),
            )
            .delete(
                model_definitions::delete_model,
                ConsoleOperation(MODEL_DEFINITIONS_DELETE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:id/advisor-findings",
            console_get(
                model_definitions::get_advisor_findings,
                ConsoleOperation(MODEL_DEFINITIONS_ADVISOR_VIEW_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:id/fields",
            console_post(
                model_definitions::create_field,
                ConsoleOperation(MODEL_FIELDS_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:id/fields/:field_id",
            console_patch(
                model_definitions::update_field,
                ConsoleOperation(MODEL_FIELDS_UPDATE_OPERATION_ID.to_string()),
            )
            .delete(
                model_definitions::delete_field,
                ConsoleOperation(MODEL_FIELDS_DELETE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:id/scope-grants",
            console_get(
                model_definitions::list_scope_grants,
                ConsoleOperation(MODEL_SCOPE_GRANTS_LIST_OPERATION_ID.to_string()),
            )
            .post(
                model_definitions::create_scope_grant,
                ConsoleOperation(MODEL_SCOPE_GRANTS_CREATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:id/scope-grants/:grant_id",
            console_patch(
                model_definitions::update_scope_grant,
                ConsoleOperation(MODEL_SCOPE_GRANTS_UPDATE_OPERATION_ID.to_string()),
            ),
        )
        .route(
            "/settings/data-models/model-definitions/:model_id/openapi.json",
            console_get(
                docs::get_data_model_openapi,
                ConsoleOperation(MODEL_DEFINITIONS_OPENAPI_VIEW_OPERATION_ID.to_string()),
            ),
        )
}
