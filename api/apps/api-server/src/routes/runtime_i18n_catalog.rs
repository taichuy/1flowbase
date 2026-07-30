use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
};
use control_plane::{errors::ControlPlaneError, i18n_catalog::RuntimeI18nCatalogService};
use domain::CatalogLocale;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    routes::console_route_assembly::{console_get, ConsoleRouteAssembly},
};

const CATALOG_CACHE_CONTROL: &str = "no-cache";

#[derive(Debug, Deserialize)]
pub struct RuntimeI18nCatalogQuery {
    pub locale: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeI18nCatalogResponse {
    pub catalog_revision: i64,
    pub locale: String,
    pub digest: String,
    pub messages: std::collections::BTreeMap<String, String>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;
    ConsoleRouteAssembly::new().route(
        "/i18n/catalog",
        console_get(get_runtime_i18n_catalog, Authenticated),
    )
}

fn parse_locale(value: String) -> Result<CatalogLocale, ApiError> {
    CatalogLocale::new(value)
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_locale").into())
}

fn matches_if_none_match(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|candidate| candidate.trim() == expected)
        })
}

#[utoipa::path(
    get, path = "/api/console/i18n/catalog",
    summary = "Get the resolved runtime i18n catalog",
    description = "Returns one content-addressed, globally keyed catalog for the authenticated root workspace and requested locale.",
    params(("locale" = String, Query, description = "Requested catalog locale")),
    responses((status = 200, body = RuntimeI18nCatalogResponse), (status = 304, description = "Catalog not modified"), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_runtime_i18n_catalog(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<RuntimeI18nCatalogQuery>,
) -> Result<Response<Body>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    if workspace_id != state.bootstrap_workspace_id {
        return Err(ControlPlaneError::PermissionDenied("root_i18n_catalog_workspace").into());
    }
    let locale = parse_locale(query.locale)?;
    let manifest =
        RuntimeI18nCatalogService::new(state.store.clone(), state.bootstrap_workspace_id)
            .manifest(workspace_id, &locale)
            .await?;
    let etag = format!("\"{}\"", manifest.digest.as_str());
    let not_modified = matches_if_none_match(&headers, &etag);
    let mut response = Response::builder().status(if not_modified {
        StatusCode::NOT_MODIFIED
    } else {
        StatusCode::OK
    });
    let response_headers = response
        .headers_mut()
        .ok_or_else(|| anyhow::anyhow!("runtime i18n response builder has no headers"))?;
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(CATALOG_CACHE_CONTROL),
    );
    response_headers.insert(header::ETAG, HeaderValue::from_str(&etag)?);
    if not_modified {
        return Ok(response.body(Body::empty())?);
    }
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let payload = RuntimeI18nCatalogResponse {
        catalog_revision: manifest.revision.value(),
        locale: manifest.bundle.locale,
        digest: manifest.digest.as_str().to_owned(),
        messages: manifest.bundle.messages,
    };
    Ok(response.body(Body::from(serde_json::to_vec(&payload)?))?)
}
