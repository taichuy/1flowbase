use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
};
use control_plane::{errors::ControlPlaneError, i18n_catalog::RuntimeI18nCatalogService};
use domain::{CatalogDigest, CatalogLocale, CatalogModuleId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    middleware::require_session::require_session,
    routes::{
        console_route_assembly::{console_get, ConsoleRouteAssembly},
        runtime_i18n_catalog_cache::runtime_i18n_bundle_cache,
    },
};

const MANIFEST_CACHE_CONTROL: &str = "no-cache";
const BUNDLE_CACHE_CONTROL: &str = "public,max-age=31536000,immutable";

#[derive(Debug, Deserialize)]
pub struct RuntimeI18nLocaleQuery {
    pub locale: String,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeI18nBundleQuery {
    pub module: String,
    pub locale: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeI18nManifestModuleResponse {
    pub module: String,
    pub digest: String,
    pub href: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeI18nManifestResponse {
    pub catalog_revision: i64,
    pub locale: String,
    pub modules: Vec<RuntimeI18nManifestModuleResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeI18nBundleResponse {
    pub module: String,
    pub locale: String,
    pub messages: std::collections::BTreeMap<String, String>,
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::Authenticated;

    ConsoleRouteAssembly::new()
        .route(
            "/i18n/manifest",
            console_get(get_runtime_i18n_manifest, Authenticated),
        )
        .route(
            "/i18n/bundles/:digest",
            console_get(get_runtime_i18n_bundle, Authenticated),
        )
}

fn parse_locale(value: String) -> Result<CatalogLocale, ApiError> {
    CatalogLocale::new(value)
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_locale").into())
}

fn parse_module(value: String) -> Result<CatalogModuleId, ApiError> {
    CatalogModuleId::new(value)
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_module").into())
}

fn parse_digest(value: String) -> Result<CatalogDigest, ApiError> {
    CatalogDigest::new(value)
        .map_err(|_| ControlPlaneError::InvalidInput("i18n_catalog_digest").into())
}

fn etag(digest: &str) -> String {
    format!("\"{digest}\"")
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

fn json_response(
    status: StatusCode,
    cache_control: &'static str,
    etag_value: &str,
    body: Vec<u8>,
) -> Result<Response<Body>, ApiError> {
    let mut response = Response::builder().status(status);
    let headers = response
        .headers_mut()
        .ok_or_else(|| anyhow::anyhow!("runtime i18n response builder has no headers"))?;
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control),
    );
    headers.insert(header::ETAG, HeaderValue::from_str(etag_value)?);
    if status != StatusCode::NOT_MODIFIED {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    Ok(response.body(Body::from(body))?)
}

fn bundle_href(module: &str, locale: &str, digest: &str) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("module", module)
        .append_pair("locale", locale);
    let query = serializer.finish();
    format!("/api/console/i18n/bundles/{digest}?{query}")
}

fn require_runtime_workspace(state: &ApiState, workspace_id: uuid::Uuid) -> Result<(), ApiError> {
    if workspace_id != state.bootstrap_workspace_id {
        return Err(ControlPlaneError::PermissionDenied("root_i18n_catalog_workspace").into());
    }
    Ok(())
}

#[utoipa::path(
    get, path = "/api/console/i18n/manifest",
    summary = "Get runtime i18n catalog manifest",
    description = "Returns content-addressed root-workspace catalog module links for an authenticated runtime consumer locale.",
    params(("locale" = String, Query, description = "Requested catalog locale")),
    responses((status = 200, body = RuntimeI18nManifestResponse), (status = 304, description = "Manifest not modified"), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody))
)]
pub async fn get_runtime_i18n_manifest(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Query(query): Query<RuntimeI18nLocaleQuery>,
) -> Result<Response<Body>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    require_runtime_workspace(&state, workspace_id)?;
    let locale = parse_locale(query.locale)?;
    let manifest =
        RuntimeI18nCatalogService::new(state.store.clone(), state.bootstrap_workspace_id)
            .manifest(workspace_id, &locale)
            .await?;
    let modules = manifest
        .modules
        .into_iter()
        .map(|entry| {
            let digest = entry.digest.as_str().to_owned();
            let body = Arc::<[u8]>::from(entry.bundle.canonical_body()?);
            runtime_i18n_bundle_cache().insert(
                workspace_id,
                &entry.bundle.module,
                &entry.bundle.locale,
                &digest,
                body,
            );
            Ok(RuntimeI18nManifestModuleResponse {
                href: bundle_href(&entry.bundle.module, &entry.bundle.locale, &digest),
                module: entry.bundle.module,
                digest,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let payload = RuntimeI18nManifestResponse {
        catalog_revision: manifest.revision.value(),
        locale: locale.as_str().to_owned(),
        modules,
    };
    let body = serde_json::to_vec(&payload)?;
    let etag_value = etag(&format!("sha256:{:x}", Sha256::digest(&body)));
    let not_modified = matches_if_none_match(&headers, &etag_value);
    json_response(
        if not_modified {
            StatusCode::NOT_MODIFIED
        } else {
            StatusCode::OK
        },
        MANIFEST_CACHE_CONTROL,
        &etag_value,
        if not_modified { Vec::new() } else { body },
    )
}

#[utoipa::path(
    get, path = "/api/console/i18n/bundles/{digest}",
    summary = "Get a content-addressed runtime i18n module bundle",
    description = "Returns canonical resolved messages for exactly one root-workspace module and locale when the requested content digest exists.",
    params(("digest" = String, Path, description = "SHA-256 content digest"), ("module" = String, Query, description = "Catalog module identity"), ("locale" = String, Query, description = "Requested catalog locale")),
    responses((status = 200, body = RuntimeI18nBundleResponse), (status = 304, description = "Bundle not modified"), (status = 400, body = crate::error_response::ErrorBody), (status = 401, body = crate::error_response::ErrorBody), (status = 403, body = crate::error_response::ErrorBody), (status = 404, body = crate::error_response::ErrorBody))
)]
pub async fn get_runtime_i18n_bundle(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(requested_digest): Path<String>,
    Query(query): Query<RuntimeI18nBundleQuery>,
) -> Result<Response<Body>, ApiError> {
    let context = require_session(&state, &headers).await?;
    let workspace_id = context.actor.current_workspace_id;
    require_runtime_workspace(&state, workspace_id)?;
    let module = parse_module(query.module)?;
    let locale = parse_locale(query.locale)?;
    let digest = parse_digest(requested_digest)?;
    let etag_value = etag(digest.as_str());
    let body = match runtime_i18n_bundle_cache().get(
        workspace_id,
        module.as_str(),
        locale.as_str(),
        digest.as_str(),
    ) {
        Some(body) => body,
        None => {
            let current =
                RuntimeI18nCatalogService::new(state.store.clone(), state.bootstrap_workspace_id)
                    .current_bundle(workspace_id, &module, &locale)
                    .await?
                    .filter(|entry| entry.digest == digest)
                    .ok_or(ControlPlaneError::NotFound("runtime_i18n_catalog_bundle"))?;
            let body = Arc::<[u8]>::from(current.bundle.canonical_body()?);
            runtime_i18n_bundle_cache().insert(
                workspace_id,
                module.as_str(),
                locale.as_str(),
                digest.as_str(),
                body.clone(),
            );
            body
        }
    };
    let not_modified = matches_if_none_match(&headers, &etag_value);
    json_response(
        if not_modified {
            StatusCode::NOT_MODIFIED
        } else {
            StatusCode::OK
        },
        BUNDLE_CACHE_CONTROL,
        &etag_value,
        if not_modified {
            Vec::new()
        } else {
            body.to_vec()
        },
    )
}
