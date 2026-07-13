use std::sync::Arc;

use axum::{
    routing::{get, patch, post},
    Router,
};

use crate::{
    app_state::ApiState,
    routes::{data_sources, docs, model_definitions},
};

pub fn router() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/settings/data-models/data-sources/catalog",
            get(data_sources::list_catalog),
        )
        .route(
            "/settings/data-models/data-sources",
            get(data_sources::list_data_sources).post(data_sources::create_data_source),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/defaults",
            patch(data_sources::update_defaults),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/validate",
            post(data_sources::validate_data_source),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/resources",
            get(data_sources::list_resources),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/resources/discover",
            post(data_sources::discover_resources),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/preview-read",
            post(data_sources::preview_read),
        )
        .route(
            "/settings/data-models/data-sources/:data_source_id/resources/map-to-model",
            post(data_sources::map_resource_to_model),
        )
        .route(
            "/settings/data-models/model-definitions",
            get(model_definitions::list_models).post(model_definitions::create_model),
        )
        .route(
            "/settings/data-models/model-definitions:batchDelete",
            post(model_definitions::batch_delete_models),
        )
        .route(
            "/settings/data-models/model-definitions/:id",
            patch(model_definitions::update_model).delete(model_definitions::delete_model),
        )
        .route(
            "/settings/data-models/model-definitions/:id/advisor-findings",
            get(model_definitions::get_advisor_findings),
        )
        .route(
            "/settings/data-models/model-definitions/:id/fields",
            post(model_definitions::create_field),
        )
        .route(
            "/settings/data-models/model-definitions/:id/fields/:field_id",
            patch(model_definitions::update_field).delete(model_definitions::delete_field),
        )
        .route(
            "/settings/data-models/model-definitions/:id/scope-grants",
            get(model_definitions::list_scope_grants).post(model_definitions::create_scope_grant),
        )
        .route(
            "/settings/data-models/model-definitions/:id/scope-grants/:grant_id",
            patch(model_definitions::update_scope_grant),
        )
        .route(
            "/settings/data-models/model-definitions/:model_id/openapi.json",
            get(docs::get_data_model_openapi),
        )
}
