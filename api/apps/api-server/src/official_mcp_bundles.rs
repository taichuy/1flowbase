use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use control_plane::ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput};

use crate::{
    config::ResolvedOfficialMcpBundleSourceConfig, official_plugin_registry::rewrite_github_raw_url,
};

pub const MCP_CATALOG_SCHEMA_VERSION: &str = "1flowbase.mcp-catalog/v2";
const SOURCE_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const SOURCE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;

fn rewrite_github_release_url(url: &str, github_proxy_url: Option<&str>) -> String {
    let Some(github_proxy_url) = github_proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return url.to_string();
    };
    let github_proxy_url = github_proxy_url.trim_end_matches('/');
    let github_release_prefix = "https://github.com/";
    let proxied_release_prefix = format!("{github_proxy_url}/{github_release_prefix}");
    if url.starts_with(&proxied_release_prefix) || !url.starts_with(github_release_prefix) {
        return url.to_string();
    }
    format!("{github_proxy_url}/{url}")
}

#[derive(Debug, Clone, Serialize)]
pub struct OfficialMcpBundleCatalogSource {
    pub source_kind: String,
    pub source_label: String,
    pub catalog_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpCatalogVersion {
    pub bundle_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub exported_from_system_version: String,
    pub release_tag: String,
    pub download_url: String,
    pub checksum: String,
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize)]
struct McpCatalogBundle {
    organization: String,
    bundle_id: String,
    source_path: String,
    #[serde(default)]
    versions: Vec<McpCatalogVersion>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpCatalogDocument {
    schema_version: String,
    #[serde(default)]
    bundles: Vec<McpCatalogBundle>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalMcpBundleReceipt {
    pub organization: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub exported_from_system_version: String,
    pub release_tag: String,
    pub checksum: String,
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpBundleLibraryEntry {
    pub organization: String,
    pub bundle_id: String,
    pub source_path: Option<String>,
    pub remote_versions: Vec<McpCatalogVersion>,
    pub local_versions: Vec<LocalMcpBundleReceipt>,
    pub current_bundle_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpBundleLibraryCatalog {
    pub source: OfficialMcpBundleCatalogSource,
    pub remote_available: bool,
    pub remote_error: Option<String>,
    pub bundles: Vec<McpBundleLibraryEntry>,
}

// Legacy projection remains available while old clients move to the local-library routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficialMcpBundleCatalogEntry {
    pub organization: String,
    pub bundle_id: String,
    pub latest_version: String,
    pub locale: String,
    pub minimum_host_version: String,
    pub exported_from_system_version: String,
    pub release_tag: String,
    pub download_url: String,
    pub artifact_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OfficialMcpBundleCatalogSnapshot {
    pub source: OfficialMcpBundleCatalogSource,
    pub entries: Vec<OfficialMcpBundleCatalogEntry>,
}

#[derive(Debug, Clone)]
pub struct DownloadedOfficialMcpBundle {
    pub file_name: String,
    pub package_bytes: Vec<u8>,
}

#[async_trait]
pub trait OfficialMcpBundleSourcePort: Send + Sync {
    async fn library_catalog(&self) -> Result<McpBundleLibraryCatalog> {
        bail!("official MCP bundle library is unavailable")
    }
    async fn refresh_catalog(&self) -> Result<McpBundleLibraryCatalog> {
        self.library_catalog().await
    }
    async fn reconcile_local_installations(&self) -> Result<()> {
        Ok(())
    }
    async fn sync(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: Option<&str>,
    ) -> Result<LocalMcpBundleReceipt> {
        let _ = (organization, bundle_id, bundle_version);
        bail!("official MCP bundle library is unavailable")
    }
    async fn resolve_artifact(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: Option<&str>,
    ) -> Result<Vec<u8>> {
        let _ = (organization, bundle_id, bundle_version);
        bail!("official MCP bundle library is unavailable")
    }
    async fn switch_current(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<LocalMcpBundleReceipt> {
        let _ = (organization, bundle_id, bundle_version);
        bail!("official MCP bundle library is unavailable")
    }
    async fn delete_local_version(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<()> {
        let _ = (organization, bundle_id, bundle_version);
        bail!("official MCP bundle library is unavailable")
    }
    async fn repair(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<LocalMcpBundleReceipt> {
        let _ = (organization, bundle_id, bundle_version);
        bail!("official MCP bundle library is unavailable")
    }
    async fn list_catalog(&self) -> Result<OfficialMcpBundleCatalogSnapshot>;
    async fn download_bundle(
        &self,
        organization: &str,
        bundle_id: &str,
    ) -> Result<DownloadedOfficialMcpBundle>;
}

#[derive(Clone)]
pub struct ApiOfficialMcpBundleRegistry {
    source_kind: String,
    source_label: String,
    catalog_url: String,
    github_proxy_url: Option<String>,
    root: PathBuf,
    installation_repository: Arc<dyn ExtensionInstallationRepository>,
    node_id: String,
    actor_user_id: Uuid,
    remote_catalog_cache: Arc<tokio::sync::RwLock<Option<McpCatalogDocument>>>,
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    client: Client,
}

impl ApiOfficialMcpBundleRegistry {
    pub fn new(
        source: ResolvedOfficialMcpBundleSourceConfig,
        root: PathBuf,
        installation_repository: Arc<dyn ExtensionInstallationRepository>,
        node_id: String,
        actor_user_id: Uuid,
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        Self {
            source_kind: source.source_kind,
            source_label: source.source_label,
            catalog_url: source.catalog_url,
            github_proxy_url: source.github_proxy_url,
            root,
            installation_repository,
            node_id,
            actor_user_id,
            remote_catalog_cache: Arc::new(tokio::sync::RwLock::new(None)),
            trusted_public_keys,
            client: Client::builder()
                .connect_timeout(SOURCE_CONNECT_TIMEOUT)
                .timeout(SOURCE_REQUEST_TIMEOUT)
                .build()
                .expect("official MCP source HTTP client configuration must be valid"),
        }
    }

    fn source(&self) -> OfficialMcpBundleCatalogSource {
        OfficialMcpBundleCatalogSource {
            source_kind: self.source_kind.clone(),
            source_label: self.source_label.clone(),
            catalog_url: self.catalog_url.clone(),
        }
    }

    async fn download_once(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request official MCP source from {url}"))?
            .error_for_status()
            .with_context(|| format!("official MCP source returned an error for {url}"))?
            .bytes()
            .await
            .context("failed to read official MCP response body")?;
        if bytes.is_empty() || bytes.len() > MAX_BUNDLE_BYTES {
            bail!("official MCP bundle exceeds download budget");
        }
        Ok(bytes.to_vec())
    }

    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let direct_error = match self.download_once(url).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) => error,
        };
        let proxy = rewrite_github_release_url(
            &rewrite_github_raw_url(url, self.github_proxy_url.as_deref()),
            self.github_proxy_url.as_deref(),
        );
        if proxy == url {
            return Err(direct_error);
        }
        self.download_once(&proxy).await.with_context(|| {
            format!("official MCP direct source failed before proxy fallback: {direct_error}")
        })
    }

    async fn remote_catalog(&self) -> Result<McpCatalogDocument> {
        let bytes = self.download_bytes(&self.catalog_url).await?;
        let mut document: McpCatalogDocument =
            serde_json::from_slice(&bytes).context("failed to decode official MCP catalog")?;
        if document.schema_version != MCP_CATALOG_SCHEMA_VERSION {
            bail!("unsupported official MCP catalog schema");
        }
        for bundle in &mut document.bundles {
            validate_identity(&bundle.organization, &bundle.bundle_id)?;
            for version in &mut bundle.versions {
                Version::parse(&version.bundle_version)
                    .context("official MCP catalog contains invalid bundle version")?;
                version.download_url = resolve_download_url(
                    &self.catalog_url,
                    &bundle.source_path,
                    &version.download_url,
                )?;
            }
        }
        Ok(document)
    }

    fn installation_identity(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> domain::ExtensionInstallationIdentity {
        domain::ExtensionInstallationIdentity {
            category: domain::ExtensionCategory::Mcp,
            organization: organization.to_string(),
            artifact_id: bundle_id.to_string(),
            version: bundle_version.to_string(),
            node_id: self.node_id.clone(),
        }
    }

    async fn indexed_records(&self) -> Result<Vec<domain::ExtensionInstallationRecord>> {
        Ok(self
            .installation_repository
            .list_extension_installations_for_node(&self.node_id)
            .await?
            .into_iter()
            .filter(|record| {
                record.identity.category == domain::ExtensionCategory::Mcp
                    && record.status == domain::ExtensionInstallationStatus::Installed
            })
            .collect())
    }

    async fn indexed_record(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        Ok(self
            .installation_repository
            .find_extension_installation(&self.installation_identity(
                organization,
                bundle_id,
                bundle_version,
            ))
            .await?
            .filter(|record| record.status == domain::ExtensionInstallationStatus::Installed))
    }

    async fn index_receipt(
        &self,
        receipt: &LocalMcpBundleReceipt,
        is_current: bool,
    ) -> Result<domain::ExtensionInstallationRecord> {
        let identity = self.installation_identity(
            &receipt.organization,
            &receipt.bundle_id,
            &receipt.bundle_version,
        );
        let existing = self
            .installation_repository
            .find_extension_installation(&identity)
            .await?;
        Ok(self
            .installation_repository
            .upsert_extension_installation(&UpsertExtensionInstallationInput {
                installation_id: existing
                    .as_ref()
                    .map(|record| record.id)
                    .unwrap_or_else(Uuid::now_v7),
                identity,
                source: "official_mcp_catalog".to_string(),
                trust: "signed".to_string(),
                local_path: bundle_path(
                    &self.root,
                    &receipt.organization,
                    &receipt.bundle_id,
                    &receipt.bundle_version,
                )
                .to_string_lossy()
                .into_owned(),
                checksum: receipt.checksum.clone(),
                signature_status: domain::ExtensionSignatureStatus::Verified,
                signature_algorithm: Some(receipt.algorithm.clone()),
                signing_key_id: Some(receipt.key_id.clone()),
                warnings: Vec::new(),
                receipt: serde_json::to_value(receipt)?,
                application_action: domain::ExtensionApplicationAction::ImportMcp,
                status: domain::ExtensionInstallationStatus::Installed,
                is_current,
                installed_by: self.actor_user_id,
            })
            .await?)
    }

    fn receipt_from_record(
        record: &domain::ExtensionInstallationRecord,
    ) -> Result<LocalMcpBundleReceipt> {
        serde_json::from_value(record.receipt.clone())
            .context("MCP template installation receipt is invalid")
    }

    async fn selected_remote(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: Option<&str>,
    ) -> Result<McpCatalogVersion> {
        validate_identity(organization, bundle_id)?;
        let document = self.remote_catalog().await?;
        *self.remote_catalog_cache.write().await = Some(document.clone());
        let bundle = document
            .bundles
            .into_iter()
            .find(|item| item.organization == organization && item.bundle_id == bundle_id)
            .ok_or_else(|| anyhow!("official MCP bundle not found"))?;
        match bundle_version {
            Some(selected) => bundle
                .versions
                .into_iter()
                .find(|version| version.bundle_version == selected)
                .ok_or_else(|| anyhow!("official MCP bundle release not found")),
            None => bundle
                .versions
                .into_iter()
                .max_by(|left, right| semver_cmp(&left.bundle_version, &right.bundle_version))
                .ok_or_else(|| anyhow!("official MCP bundle has no releases")),
        }
    }

    async fn install_remote(
        &self,
        organization: &str,
        bundle_id: &str,
        version: McpCatalogVersion,
        repair: bool,
    ) -> Result<LocalMcpBundleReceipt> {
        let existing = self
            .indexed_record(organization, bundle_id, &version.bundle_version)
            .await?;
        if let Some(existing_record) = existing.as_ref() {
            let existing_receipt = Self::receipt_from_record(existing_record)?;
            if existing_receipt.checksum != version.checksum {
                bail!("same MCP bundle release has a different checksum");
            }
            if !repair && Path::new(&existing_record.local_path).is_file() {
                self.installation_repository
                    .select_current_extension_installation(&self.node_id, existing_record.id)
                    .await?;
                return Ok(existing_receipt);
            }
        } else if repair {
            bail!("local MCP bundle release not found for repair");
        }
        let bytes = self.download_bytes(&version.download_url).await?;
        plugin_framework::verify_trusted_ed25519_artifact(
            &bytes,
            &version.checksum,
            &version.algorithm,
            &version.key_id,
            &version.signature,
            &self.trusted_public_keys,
        )?;
        let receipt = LocalMcpBundleReceipt {
            organization: organization.to_string(),
            bundle_id: bundle_id.to_string(),
            bundle_version: version.bundle_version,
            locale: version.locale,
            minimum_host_version: version.minimum_host_version,
            exported_from_system_version: version.exported_from_system_version,
            release_tag: version.release_tag,
            checksum: version.checksum,
            algorithm: version.algorithm,
            key_id: version.key_id,
            signature: version.signature,
        };
        let release_directory = release_dir(
            &self.root,
            &receipt.organization,
            &receipt.bundle_id,
            &receipt.bundle_version,
        );
        let staged_directory = stage_release(&self.root, &receipt, &bytes)?;
        let backup_directory = activate_staged_release(&staged_directory, &release_directory)?;
        let is_current = if repair {
            existing.as_ref().is_some_and(|record| record.is_current)
        } else {
            true
        };
        if let Err(error) = self.index_receipt(&receipt, is_current).await {
            if let Err(rollback_error) =
                rollback_activated_release(&release_directory, backup_directory.as_deref())
            {
                return Err(error).context(format!(
                    "failed to index synchronized MCP template; release rollback also failed: {rollback_error:#}"
                ));
            }
            return Err(error).context("failed to index synchronized MCP template");
        }
        if let Some(backup_directory) = backup_directory {
            if let Err(error) = fs::remove_dir_all(&backup_directory) {
                tracing::warn!(
                    path = %backup_directory.display(),
                    error = %error,
                    "synchronized MCP template backup cleanup failed"
                );
            }
        }
        Ok(receipt)
    }

    async fn project_library(
        &self,
        remote: Option<McpCatalogDocument>,
        remote_error: Option<String>,
    ) -> Result<McpBundleLibraryCatalog> {
        let mut entries = BTreeMap::new();
        for record in self.indexed_records().await? {
            let receipt = Self::receipt_from_record(&record)?;
            let key = (
                record.identity.organization.clone(),
                record.identity.artifact_id.clone(),
            );
            let entry = entries.entry(key).or_insert_with(|| McpBundleLibraryEntry {
                organization: record.identity.organization.clone(),
                bundle_id: record.identity.artifact_id.clone(),
                source_path: None,
                remote_versions: Vec::new(),
                local_versions: Vec::new(),
                current_bundle_version: None,
            });
            if record.is_current {
                entry.current_bundle_version = Some(record.identity.version.clone());
            }
            entry.local_versions.push(receipt);
        }
        for entry in entries.values_mut() {
            entry
                .local_versions
                .sort_by(|left, right| semver_cmp(&right.bundle_version, &left.bundle_version));
        }
        if let Some(document) = remote {
            for bundle in document.bundles {
                let entry = entries
                    .entry((bundle.organization.clone(), bundle.bundle_id.clone()))
                    .or_insert_with(|| McpBundleLibraryEntry {
                        organization: bundle.organization.clone(),
                        bundle_id: bundle.bundle_id.clone(),
                        source_path: None,
                        remote_versions: Vec::new(),
                        local_versions: Vec::new(),
                        current_bundle_version: None,
                    });
                entry.source_path = Some(bundle.source_path);
                entry.remote_versions = bundle.versions;
                entry
                    .remote_versions
                    .sort_by(|left, right| semver_cmp(&right.bundle_version, &left.bundle_version));
            }
        }
        Ok(McpBundleLibraryCatalog {
            source: self.source(),
            remote_available: self.remote_catalog_cache.read().await.is_some(),
            remote_error,
            bundles: entries.into_values().collect(),
        })
    }
}

#[async_trait]
impl OfficialMcpBundleSourcePort for ApiOfficialMcpBundleRegistry {
    async fn library_catalog(&self) -> Result<McpBundleLibraryCatalog> {
        self.project_library(self.remote_catalog_cache.read().await.clone(), None)
            .await
    }

    async fn refresh_catalog(&self) -> Result<McpBundleLibraryCatalog> {
        match self.remote_catalog().await {
            Ok(document) => {
                *self.remote_catalog_cache.write().await = Some(document.clone());
                self.project_library(Some(document), None).await
            }
            Err(error) => {
                self.project_library(
                    self.remote_catalog_cache.read().await.clone(),
                    Some(error.to_string()),
                )
                .await
            }
        }
    }

    async fn reconcile_local_installations(&self) -> Result<()> {
        for (_, _, versions, current) in scan_local(&self.root)? {
            let selected_current = current.as_deref().or_else(|| {
                versions
                    .first()
                    .map(|receipt| receipt.bundle_version.as_str())
            });
            for receipt in
                versions
                    .iter()
                    .filter(|receipt| Some(receipt.bundle_version.as_str()) != selected_current)
                    .chain(versions.iter().filter(|receipt| {
                        Some(receipt.bundle_version.as_str()) == selected_current
                    }))
            {
                self.index_receipt(
                    receipt,
                    Some(receipt.bundle_version.as_str()) == selected_current,
                )
                .await?;
            }
        }
        for record in self
            .installation_repository
            .list_extension_installations_for_node(&self.node_id)
            .await?
            .into_iter()
            .filter(|record| record.identity.category == domain::ExtensionCategory::Mcp)
        {
            if record.status == domain::ExtensionInstallationStatus::Installed
                && !Path::new(&record.local_path).is_file()
            {
                self.installation_repository
                    .set_extension_installation_status(
                        record.id,
                        domain::ExtensionInstallationStatus::Missing,
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn sync(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: Option<&str>,
    ) -> Result<LocalMcpBundleReceipt> {
        let version = self
            .selected_remote(organization, bundle_id, bundle_version)
            .await?;
        self.install_remote(organization, bundle_id, version, false)
            .await
    }

    async fn resolve_artifact(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: Option<&str>,
    ) -> Result<Vec<u8>> {
        validate_identity(organization, bundle_id)?;
        let record = match bundle_version {
            Some(version) => {
                self.indexed_record(organization, bundle_id, version)
                    .await?
            }
            None => self.indexed_records().await?.into_iter().find(|record| {
                record.identity.organization == organization
                    && record.identity.artifact_id == bundle_id
                    && record.is_current
            }),
        };
        let record = record.ok_or_else(|| anyhow!("local MCP bundle release is not installed"))?;
        let receipt = Self::receipt_from_record(&record)?;
        let bytes =
            fs::read(&record.local_path).context("failed to read local MCP bundle artifact")?;
        plugin_framework::verify_trusted_ed25519_artifact(
            &bytes,
            &receipt.checksum,
            &receipt.algorithm,
            &receipt.key_id,
            &receipt.signature,
            &self.trusted_public_keys,
        )?;
        Ok(bytes)
    }

    async fn switch_current(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<LocalMcpBundleReceipt> {
        let record = self
            .indexed_record(organization, bundle_id, bundle_version)
            .await?
            .ok_or_else(|| anyhow!("local MCP bundle release not found"))?;
        self.installation_repository
            .select_current_extension_installation(&self.node_id, record.id)
            .await?
            .ok_or_else(|| anyhow!("local MCP bundle release not found"))?;
        Self::receipt_from_record(&record)
    }

    async fn delete_local_version(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<()> {
        let record = self
            .indexed_record(organization, bundle_id, bundle_version)
            .await?
            .ok_or_else(|| anyhow!("local MCP bundle release not found"))?;
        let directory = release_dir(&self.root, organization, bundle_id, bundle_version);
        let tombstone = directory.with_extension(format!("deleting-{}", Uuid::now_v7()));
        fs::rename(&directory, &tombstone)?;
        if let Err(error) = self
            .installation_repository
            .remove_extension_installation(&self.node_id, record.id)
            .await
        {
            let _ = fs::rename(&tombstone, &directory);
            return Err(error).context("failed to remove MCP template index");
        }
        if let Err(error) = fs::remove_dir_all(&tombstone) {
            tracing::warn!(
                path = %tombstone.display(),
                error = %error,
                "deleted MCP template tombstone cleanup failed"
            );
        }
        Ok(())
    }

    async fn repair(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<LocalMcpBundleReceipt> {
        let version = self
            .selected_remote(organization, bundle_id, Some(bundle_version))
            .await?;
        self.install_remote(organization, bundle_id, version, true)
            .await
    }

    async fn list_catalog(&self) -> Result<OfficialMcpBundleCatalogSnapshot> {
        let catalog = self.remote_catalog().await?;
        let entries = catalog
            .bundles
            .into_iter()
            .filter_map(|bundle| {
                bundle
                    .versions
                    .into_iter()
                    .max_by(|left, right| semver_cmp(&left.bundle_version, &right.bundle_version))
                    .map(|version| OfficialMcpBundleCatalogEntry {
                        organization: bundle.organization,
                        bundle_id: bundle.bundle_id,
                        latest_version: version.bundle_version,
                        locale: version.locale,
                        minimum_host_version: version.minimum_host_version,
                        exported_from_system_version: version.exported_from_system_version,
                        release_tag: version.release_tag,
                        download_url: version.download_url,
                        artifact_sha256: Some(version.checksum),
                    })
            })
            .collect();
        Ok(OfficialMcpBundleCatalogSnapshot {
            source: self.source(),
            entries,
        })
    }

    async fn download_bundle(
        &self,
        organization: &str,
        bundle_id: &str,
    ) -> Result<DownloadedOfficialMcpBundle> {
        let receipt = self.sync(organization, bundle_id, None).await?;
        let package_bytes = self
            .resolve_artifact(organization, bundle_id, Some(&receipt.bundle_version))
            .await?;
        Ok(DownloadedOfficialMcpBundle {
            file_name: format!(
                "{}-{}-v{}.zip",
                organization, bundle_id, receipt.bundle_version
            ),
            package_bytes,
        })
    }
}

fn validate_segment(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("invalid MCP bundle identity");
    }
    Ok(())
}

fn validate_identity(organization: &str, bundle_id: &str) -> Result<()> {
    validate_segment(organization)?;
    validate_segment(bundle_id)
}

fn semver_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn resolve_download_url(catalog_url: &str, source_path: &str, value: &str) -> Result<String> {
    if reqwest::Url::parse(value).is_ok() {
        return Ok(value.to_string());
    }
    let catalog = reqwest::Url::parse(catalog_url)?;
    let source = if source_path.trim().is_empty() {
        catalog
    } else {
        catalog.join(source_path)?
    };
    Ok(source.join(value)?.to_string())
}

fn bundle_dir(root: &Path, organization: &str, bundle_id: &str) -> PathBuf {
    root.join(format!("@{organization}")).join(bundle_id)
}

fn release_dir(root: &Path, organization: &str, bundle_id: &str, bundle_version: &str) -> PathBuf {
    bundle_dir(root, organization, bundle_id)
        .join("releases")
        .join(bundle_version)
}

fn bundle_path(root: &Path, organization: &str, bundle_id: &str, version: &str) -> PathBuf {
    release_dir(root, organization, bundle_id, version).join("bundle.zip")
}

fn receipt_path(root: &Path, organization: &str, bundle_id: &str, version: &str) -> PathBuf {
    release_dir(root, organization, bundle_id, version).join("receipt.json")
}

fn read_receipt(
    root: &Path,
    organization: &str,
    bundle_id: &str,
    version: &str,
) -> Result<Option<LocalMcpBundleReceipt>> {
    validate_identity(organization, bundle_id)?;
    Version::parse(version)?;
    let path = receipt_path(root, organization, bundle_id, version);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}

fn local_versions(
    root: &Path,
    organization: &str,
    bundle_id: &str,
) -> Result<Vec<LocalMcpBundleReceipt>> {
    let releases = bundle_dir(root, organization, bundle_id).join("releases");
    if !releases.is_dir() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(releases)? {
        let version = entry?.file_name().to_string_lossy().to_string();
        if Version::parse(&version).is_err() {
            continue;
        }
        if let Some(receipt) = read_receipt(root, organization, bundle_id, &version)? {
            result.push(receipt);
        }
    }
    result.sort_by(|left, right| semver_cmp(&right.bundle_version, &left.bundle_version));
    Ok(result)
}

fn scan_local(
    root: &Path,
) -> Result<Vec<(String, String, Vec<LocalMcpBundleReceipt>, Option<String>)>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for organization_entry in fs::read_dir(root)? {
        let organization_entry = organization_entry?;
        let name = organization_entry.file_name().to_string_lossy().to_string();
        let Some(organization) = name.strip_prefix('@') else {
            continue;
        };
        if validate_segment(organization).is_err() || !organization_entry.path().is_dir() {
            continue;
        }
        for bundle_entry in fs::read_dir(organization_entry.path())? {
            let bundle_entry = bundle_entry?;
            let bundle_id = bundle_entry.file_name().to_string_lossy().to_string();
            if validate_segment(&bundle_id).is_err() || !bundle_entry.path().is_dir() {
                continue;
            }
            let versions = local_versions(root, organization, &bundle_id)?;
            if !versions.is_empty() {
                result.push((
                    organization.to_string(),
                    bundle_id.clone(),
                    versions,
                    read_current(root, organization, &bundle_id)?,
                ));
            }
        }
    }
    result.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    Ok(result)
}

fn stage_release(root: &Path, receipt: &LocalMcpBundleReceipt, bytes: &[u8]) -> Result<PathBuf> {
    let directory = release_dir(
        root,
        &receipt.organization,
        &receipt.bundle_id,
        &receipt.bundle_version,
    );
    let parent = directory
        .parent()
        .ok_or_else(|| anyhow!("MCP bundle release path has no parent"))?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(
        ".{}.{}.staging",
        receipt.bundle_version,
        Uuid::now_v7()
    ));
    let receipt_bytes = serde_json::to_vec_pretty(receipt)?;
    let write_result = (|| -> Result<()> {
        fs::create_dir(&staging)?;
        fs::write(staging.join("bundle.zip"), bytes)?;
        fs::write(staging.join("receipt.json"), receipt_bytes)?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error).context("failed to stage synchronized MCP template release");
    }
    Ok(staging)
}

fn activate_staged_release(staging: &Path, release: &Path) -> Result<Option<PathBuf>> {
    let backup = if release.exists() {
        let backup = release.with_extension(format!("backup-{}", Uuid::now_v7()));
        fs::rename(release, &backup)?;
        Some(backup)
    } else {
        None
    };
    if let Err(error) = fs::rename(staging, release) {
        if let Some(backup) = backup.as_deref() {
            let _ = fs::rename(backup, release);
        }
        let _ = fs::remove_dir_all(staging);
        return Err(error).context("failed to activate synchronized MCP template release");
    }
    Ok(backup)
}

fn rollback_activated_release(release: &Path, backup: Option<&Path>) -> Result<()> {
    let discarded = release.with_extension(format!("rollback-{}", Uuid::now_v7()));
    fs::rename(release, &discarded)
        .context("failed to move synchronized MCP template out of the active path")?;
    if let Some(backup) = backup {
        if let Err(error) = fs::rename(backup, release) {
            let _ = fs::rename(&discarded, release);
            return Err(error).context("failed to restore the previous MCP template release");
        }
    }
    if let Err(error) = fs::remove_dir_all(&discarded) {
        tracing::warn!(
            path = %discarded.display(),
            error = %error,
            "rolled back MCP template tombstone cleanup failed"
        );
    }
    Ok(())
}

fn read_current(root: &Path, organization: &str, bundle_id: &str) -> Result<Option<String>> {
    let path = bundle_dir(root, organization, bundle_id).join("current");
    if !path.is_file() {
        return Ok(None);
    }
    let value = fs::read_to_string(path)?.trim().to_string();
    Version::parse(&value).context("invalid local MCP bundle current pointer")?;
    Ok(Some(value))
}
