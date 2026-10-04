use super::{archive, plugins::TemplateDependencies, releases};
use crate::official_extension_catalog::{
    OfficialExtensionCatalogEntry, OfficialExtensionCatalogSearchQuery,
};
use anyhow::{ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use control_plane::portable_template::{
    PortableTemplateIdentityRepository, PortableTemplatePackage,
};
use serde::Deserialize;
use serde_json::{json, Value};
const CATEGORY: &str = "applications-demo";
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogQuery {
    pub category: Option<String>,
    pub cursor: Option<String>,
    pub q: Option<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportQuery {
    pub format: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TemplateRequest {
    Catalog(CatalogRequest),
    Archive(ArchiveRequest),
    Package(Box<PortableTemplatePackage>),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRequest {
    pub catalog_id: String,
    pub release_version: u64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveRequest {
    pub archive_base64: String,
}
fn official_metadata(entry: &OfficialExtensionCatalogEntry) -> Result<Value> {
    ensure!(
        entry.category == CATEGORY && entry.source.kind == "application_template_release",
        "application_template_catalog_category"
    );
    let release_version = entry.version.parse::<u64>()?;
    ensure!(release_version > 0, "application_template_release_version");
    Ok(
        json!({"template_id":format!("@{}/{}",entry.organization,entry.artifact),"release_version":release_version,"name":entry.name,"description":entry.description,"checksum":entry.checksum,"catalog_id":entry.id,"source":"official"}),
    )
}
pub(crate) async fn list(
    dependencies: &TemplateDependencies,
    actor: &domain::ActorContext,
    query: CatalogQuery,
) -> Result<Value> {
    if query.category.as_deref() != Some(CATEGORY) {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "application_template_catalog_category",
        )
        .into());
    }
    let builtin_cursor = query
        .cursor
        .as_deref()
        .is_some_and(|c| c.starts_with("builtin:"));
    let remote = if builtin_cursor {
        None
    } else {
        Some(
            dependencies
                .official_catalog_source
                .search_for_workspace(
                    actor.current_workspace_id,
                    CATEGORY,
                    OfficialExtensionCatalogSearchQuery {
                        slot_code: None,
                        q: query.q.clone(),
                        limit: 100,
                        cursor: query.cursor.clone(),
                    },
                )
                .await,
        )
    };
    let (mut entries, next_cursor, total) = match remote {
        Some(Ok(result)) if result.total_entries > 0 || query.cursor.is_some() => (
            result
                .entries
                .iter()
                .map(official_metadata)
                .collect::<Result<Vec<_>>>()?,
            result.next_cursor,
            result.total_entries,
        ),
        Some(Err(error)) if query.cursor.is_some() => return Err(error),
        other => {
            let builtins = releases::discover(&dependencies.application_template_root).await?;
            if let Some(Err(error)) = other {
                if builtins.is_empty() {
                    return Err(error);
                }
                tracing::warn!(error=%error,"application template catalog unavailable; built-in releases remain available");
            }
            let filtered = builtins
                .into_iter()
                .filter(|item| {
                    query.q.as_ref().is_none_or(|q| {
                        format!(
                            "{} {} {}",
                            item.release.template_id, item.release.name, item.release.description
                        )
                        .to_lowercase()
                        .contains(&q.to_lowercase())
                    })
                })
                .collect::<Vec<_>>();
            let offset = query
                .cursor
                .as_deref()
                .map(|c| {
                    c.strip_prefix("builtin:")
                        .context("application_template_cursor")
                        .and_then(|v| Ok(v.parse::<usize>()?))
                })
                .transpose()?
                .unwrap_or(0);
            ensure!(offset <= filtered.len(), "application_template_cursor");
            let total = filtered.len();
            let entries=filtered.into_iter().skip(offset).take(100).map(|item|json!({"template_id":item.release.template_id,"release_version":item.release.release_version,"name":item.release.name,"description":item.release.description,"checksum":item.checksum,"catalog_id":format!("builtin:{}",item.release.template_id),"source":"builtin"})).collect::<Vec<_>>();
            let next = (offset + entries.len() < total)
                .then(|| format!("builtin:{}", offset + entries.len()));
            (entries, next, total)
        }
    };
    let repository = dependencies.store.for_actor(actor.clone());
    for entry in &mut entries {
        let records = repository
            .load_application_template_releases(
                actor.current_workspace_id,
                entry["template_id"]
                    .as_str()
                    .context("application_template_id")?,
            )
            .await?;
        let installed = records
            .iter()
            .filter(|r| r.successful)
            .max_by_key(|r| r.release_version);
        entry["installed_release_version"] = json!(installed.map(|r| r.release_version));
        entry["installed_checksum"] = json!(installed.map(|r| &r.checksum));
    }
    Ok(json!({"application_templates":entries,"next_cursor":next_cursor,"total":total}))
}
pub(crate) async fn resolve(
    dependencies: &TemplateDependencies,
    actor: &domain::ActorContext,
    request: TemplateRequest,
) -> Result<PortableTemplatePackage> {
    match request {
        TemplateRequest::Package(package) => Ok(*package),
        TemplateRequest::Archive(request) => decode_uploaded_archive(request.archive_base64).await,
        TemplateRequest::Catalog(request) => {
            if let Some(id) = request.catalog_id.strip_prefix("builtin:") {
                let item = releases::discover(&dependencies.application_template_root)
                    .await?
                    .into_iter()
                    .find(|r| {
                        r.release.template_id == id
                            && r.release.release_version == request.release_version
                    })
                    .context("application_template_release_not_found")?;
                return item.load().await;
            }
            let located = dependencies
                .official_catalog_source
                .find_entry_for_workspace(actor.current_workspace_id, CATEGORY, &request.catalog_id)
                .await?
                .context("application_template_catalog_not_found")?;
            let metadata = official_metadata(&located.entry)?;
            ensure!(
                metadata["release_version"].as_u64() == Some(request.release_version),
                "application_template_release_changed"
            );
            let downloaded = dependencies
                .official_catalog_source
                .download_artifact_for_workspace(actor.current_workspace_id, &located.entry)
                .await?;
            let template_id = metadata["template_id"]
                .as_str()
                .context("application_template_id")?
                .to_owned();
            let trusted_keys = dependencies.official_plugin_source.trusted_public_keys();
            tokio::task::spawn_blocking(move || {
                decode_verified_archive(
                    &downloaded,
                    &template_id,
                    request.release_version,
                    &trusted_keys,
                )
            })
            .await
            .context("application_template_verified_decode_task")?
        }
    }
}

pub(super) async fn decode_uploaded_archive(
    archive_base64: String,
) -> Result<PortableTemplatePackage> {
    // Decode and checksum/decompression all run on the blocking pool. Join failures
    // remain task errors; only invalid upload contents become the public input error.
    tokio::task::spawn_blocking(move || {
        let bytes = STANDARD.decode(archive_base64)?;
        archive::decode(&bytes)
    })
    .await
    .context("application_template_upload_decode_task")?
    .map_err(|error| {
        tracing::debug!(error=%error,"invalid application template upload");
        control_plane::errors::ControlPlaneError::InvalidInput(
            "application_template_archive_invalid",
        )
        .into()
    })
}

pub(crate) fn decode_verified_archive(
    downloaded: &crate::official_extension_catalog::DownloadedOfficialExtensionArtifact,
    template_id: &str,
    release_version: u64,
    trusted_keys: &[plugin_framework::TrustedPublicKey],
) -> Result<PortableTemplatePackage> {
    let signature = downloaded
        .descriptor
        .signature
        .as_ref()
        .context("application_template_signature_missing")?;
    let field = |key: &str| {
        signature
            .get(key)
            .and_then(Value::as_str)
            .context("application_template_signature_field")
    };
    plugin_framework::verify_trusted_ed25519_artifact(
        &downloaded.artifact_bytes,
        downloaded
            .descriptor
            .expected_checksum
            .as_deref()
            .context("application_template_checksum_missing")?,
        field("algorithm")?,
        field("key_id")?,
        field("signature")?,
        trusted_keys,
    )
    .map_err(|_| {
        crate::official_extension_catalog::OfficialExtensionArtifactError::SignatureInvalid
    })?;
    let package = archive::decode(&downloaded.artifact_bytes)?;
    let release = package
        .release
        .as_ref()
        .context("application_template_release_missing")?;
    ensure!(
        release.template_id == template_id && release.release_version == release_version,
        "application_template_release_mismatch"
    );
    Ok(package)
}
