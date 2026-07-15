use std::{
    path::{Component, Path as FsPath},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::Response,
};

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
};

fn provider_icon_content_type(path: &FsPath) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

async fn path_is_file(path: &FsPath) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
}

async fn resolve_provider_icon_path(
    installed_path: &str,
    icon_path: &str,
) -> Result<std::path::PathBuf, ApiError> {
    let icon_relative_path = FsPath::new(icon_path);

    if icon_relative_path.is_absolute()
        || icon_relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput("icon").into());
    }

    let installed_root = FsPath::new(installed_path);
    let resolved_path = installed_root.join(icon_relative_path);
    if path_is_file(&resolved_path).await {
        return Ok(resolved_path);
    }

    // Official provider packages often store manifest file-name icons under _assets/.
    if icon_relative_path.components().count() == 1 {
        let assets_path = installed_root.join("_assets").join(icon_relative_path);
        if path_is_file(&assets_path).await {
            return Ok(assets_path);
        }
    }

    Ok(resolved_path)
}

async fn installed_manifest_icon(installed_path: &str) -> Option<String> {
    let manifest_path = FsPath::new(installed_path).join("manifest.yaml");
    let manifest_raw = tokio::fs::read_to_string(manifest_path).await.ok()?;
    let manifest = plugin_framework::parse_plugin_manifest(&manifest_raw).ok()?;
    let icon = manifest.icon?;
    let icon = icon.trim().to_string();
    if icon.is_empty() {
        return None;
    }
    Some(icon)
}

async fn installation_icon_path(
    installed_path: &str,
    metadata_json: &serde_json::Value,
) -> Option<String> {
    if let Some(icon) = metadata_json
        .get("icon")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    {
        return Some(icon);
    }

    installed_manifest_icon(installed_path).await
}

pub async fn read_provider_icon(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    Path(provider_code): Path<String>,
) -> Result<Response, ApiError> {
    let context = require_session(&state, &headers).await?;
    let source = super::service(
        &state,
        "other.model-providers",
        "model_providers.icons.view",
    )
    .provider_icon_source(context.user.id, &provider_code)
    .await?;
    let icon_path = installation_icon_path(&source.installed_path, &source.metadata_json)
        .await
        .ok_or(control_plane::errors::ControlPlaneError::NotFound(
            "plugin_icon",
        ))?;
    let resolved_path = resolve_provider_icon_path(&source.installed_path, &icon_path).await?;
    let content = tokio::fs::read(&resolved_path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                control_plane::errors::ControlPlaneError::NotFound("plugin_icon").into()
            }
            _ => ApiError::from(anyhow::Error::from(error)),
        })?;

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, provider_icon_content_type(&resolved_path))
        .body(Body::from(content))
        .map_err(ApiError::from)
}
