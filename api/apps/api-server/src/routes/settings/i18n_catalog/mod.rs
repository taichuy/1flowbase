use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, Json, Router};
use control_plane::{
    errors::ControlPlaneError,
    i18n_catalog::{
        OfficialI18nCatalogUpdateCommand, OfficialI18nCatalogUpdateOutcome,
        OfficialI18nCatalogUpdateStatus,
    },
    ports::I18nCatalogRepository,
};
use domain::WorkspaceCatalogRevision;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::{require_csrf::require_csrf, require_session::require_session},
    response::ApiSuccess,
    routes::console_route_assembly::{console_get, console_post, ConsoleRouteAssembly},
};

pub(crate) mod management;

#[derive(Debug, Serialize, ToSchema)]
pub struct I18nCatalogStateResponse {
    pub active_catalog_version: Option<String>,
    pub revision: i64,
    pub source: &'static str,
    pub source_locale: String,
    pub locales: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct I18nCatalogUpdateStatusResponse {
    pub status: &'static str,
    pub active_catalog_version: Option<String>,
    pub latest_catalog_version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ActivateI18nCatalogBody {
    pub expected_revision: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivateI18nCatalogResponse {
    pub status: &'static str,
    pub catalog_version: String,
    pub revision: i64,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/settings/i18n/catalog",
            console_get(
                get_i18n_catalog_state,
                ConsoleOperation("i18n_catalog.state.get".to_string()),
            ),
        )
        .route(
            "/settings/i18n/update-check",
            console_get(
                get_i18n_catalog_update_status,
                ConsoleOperation("i18n_catalog.update.check".to_string()),
            ),
        )
        .route(
            "/settings/i18n/activate",
            console_post(
                activate_i18n_catalog_update,
                ConsoleOperation("i18n_catalog.update.activate".to_string()),
            ),
        )
        .merge(management::route_assembly())
}

pub(super) fn require_root_catalog_actor(
    state: &ApiState,
    actor: &domain::ActorContext,
) -> Result<(), ApiError> {
    if !actor.is_root || actor.current_workspace_id != state.bootstrap_workspace_id {
        return Err(ControlPlaneError::PermissionDenied("root_i18n_catalog_actor").into());
    }
    Ok(())
}

pub(super) fn invalid_input(name: &'static str) -> ApiError {
    ControlPlaneError::InvalidInput(name).into()
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/catalog",
    summary = "Get i18n catalog state",
    description = "Returns the active root i18n catalog manifest state.",
    responses((status = 200, body = I18nCatalogStateResponse), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_i18n_catalog_state(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<I18nCatalogStateResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_root_catalog_actor(&state, &context.actor)?;
    let workspace_id = context.actor.current_workspace_id;
    let catalog_state =
        I18nCatalogRepository::get_workspace_catalog_state(&state.store, workspace_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("workspace_i18n_catalog_state"))?;
    let descriptor = match catalog_state.active_release_id() {
        Some(release_id) => Some(
            I18nCatalogRepository::get_i18n_catalog_release_descriptor(
                &state.store,
                workspace_id,
                release_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("active_i18n_catalog_release"))?,
        ),
        None => None,
    };
    let response = match descriptor {
        Some(descriptor) => I18nCatalogStateResponse {
            active_catalog_version: Some(descriptor.catalog_version.as_str().to_owned()),
            revision: catalog_state.revision().value(),
            source: "official",
            source_locale: descriptor.source_locale.as_str().to_owned(),
            locales: descriptor
                .locales
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
        },
        None => I18nCatalogStateResponse {
            active_catalog_version: None,
            revision: catalog_state.revision().value(),
            source: "official",
            source_locale: domain::I18N_CATALOG_SOURCE_LOCALE.to_owned(),
            locales: Vec::new(),
        },
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    get,
    path = "/api/console/settings/i18n/update-check",
    summary = "Check i18n catalog update",
    description = "Checks the latest official i18n catalog without activating it.",
    responses((status = 200, body = I18nCatalogUpdateStatusResponse), (status = 403, body = crate::error_response::ErrorBody), (status = 502, body = crate::error_response::ErrorBody))
)]
pub async fn get_i18n_catalog_update_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<I18nCatalogUpdateStatusResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_root_catalog_actor(&state, &context.actor)?;
    let status = state
        .official_i18n_catalog_update_service
        .check_update(context.actor.current_workspace_id)
        .await?;
    let response = match status {
        OfficialI18nCatalogUpdateStatus::Current {
            active_catalog_version,
            latest_catalog_version,
        } => I18nCatalogUpdateStatusResponse {
            status: "current",
            active_catalog_version: Some(active_catalog_version.as_str().to_owned()),
            latest_catalog_version: latest_catalog_version.as_str().to_owned(),
        },
        OfficialI18nCatalogUpdateStatus::UpdateAvailable {
            active_catalog_version,
            latest_catalog_version,
        } => I18nCatalogUpdateStatusResponse {
            status: "update_available",
            active_catalog_version: active_catalog_version.map(|value| value.as_str().to_owned()),
            latest_catalog_version: latest_catalog_version.as_str().to_owned(),
        },
    };
    Ok(Json(ApiSuccess::new(response)))
}

#[utoipa::path(
    post,
    path = "/api/console/settings/i18n/activate",
    summary = "Activate latest i18n catalog",
    description = "Checks and activates the latest official root i18n catalog at the expected revision.",
    request_body = ActivateI18nCatalogBody,
    responses((status = 200, body = ActivateI18nCatalogResponse), (status = 400, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 409, body = crate::error_response::ErrorBody), (status = 502, body = crate::error_response::ErrorBody))
)]
pub async fn activate_i18n_catalog_update(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Json(body): Json<ActivateI18nCatalogBody>,
) -> Result<Json<ApiSuccess<ActivateI18nCatalogResponse>>, ApiError> {
    let context = require_session(&state, &headers).await?;
    require_csrf(&headers, &context)?;
    require_root_catalog_actor(&state, &context.actor)?;
    let expected_revision = WorkspaceCatalogRevision::new(body.expected_revision)
        .map_err(|_| invalid_input("expected_revision"))?;
    let outcome = state
        .official_i18n_catalog_update_service
        .check_and_activate(OfficialI18nCatalogUpdateCommand {
            workspace_id: context.actor.current_workspace_id,
            expected_revision,
        })
        .await?;
    let response = match outcome {
        OfficialI18nCatalogUpdateOutcome::Current { catalog_version } => {
            ActivateI18nCatalogResponse {
                status: "current",
                catalog_version: catalog_version.as_str().to_owned(),
                revision: expected_revision.value(),
            }
        }
        OfficialI18nCatalogUpdateOutcome::Activated {
            catalog_version,
            state,
        } => ActivateI18nCatalogResponse {
            status: "activated",
            catalog_version: catalog_version.as_str().to_owned(),
            revision: state.revision().value(),
        },
    };
    Ok(Json(ApiSuccess::new(response)))
}
