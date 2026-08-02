use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    client: Client,
}

impl ApiOfficialMcpBundleRegistry {
    pub fn new(
        source: ResolvedOfficialMcpBundleSourceConfig,
        root: PathBuf,
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        Self {
            source_kind: source.source_kind,
            source_label: source.source_label,
            catalog_url: source.catalog_url,
            github_proxy_url: source.github_proxy_url,
            root,
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

    async fn selected_remote(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: Option<&str>,
    ) -> Result<McpCatalogVersion> {
        validate_identity(organization, bundle_id)?;
        let bundle = self
            .remote_catalog()
            .await?
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
        let existing = read_receipt(&self.root, organization, bundle_id, &version.bundle_version)?;
        if let Some(existing) = existing {
            if existing.checksum != version.checksum {
                bail!("same MCP bundle release has a different checksum");
            }
            if !repair {
                write_current(&self.root, organization, bundle_id, &version.bundle_version)?;
                return Ok(existing);
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
        write_release(&self.root, &receipt, &bytes)?;
        if !repair {
            write_current(&self.root, organization, bundle_id, &receipt.bundle_version)?;
        }
        Ok(receipt)
    }
}

#[async_trait]
impl OfficialMcpBundleSourcePort for ApiOfficialMcpBundleRegistry {
    async fn library_catalog(&self) -> Result<McpBundleLibraryCatalog> {
        let local = scan_local(&self.root)?;
        let remote = self.remote_catalog().await;
        let (remote_available, remote_error, remote_bundles) = match remote {
            Ok(document) => (true, None, document.bundles),
            Err(error) => (false, Some(error.to_string()), Vec::new()),
        };
        let mut entries = BTreeMap::new();
        for (organization, bundle_id, versions, current) in local {
            entries.insert(
                (organization.clone(), bundle_id.clone()),
                McpBundleLibraryEntry {
                    organization,
                    bundle_id,
                    source_path: None,
                    remote_versions: Vec::new(),
                    local_versions: versions,
                    current_bundle_version: current,
                },
            );
        }
        for bundle in remote_bundles {
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
        Ok(McpBundleLibraryCatalog {
            source: self.source(),
            remote_available,
            remote_error,
            bundles: entries.into_values().collect(),
        })
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
        if local_versions(&self.root, organization, bundle_id)?.is_empty() {
            self.sync(organization, bundle_id, bundle_version).await?;
        }
        let selected = match bundle_version {
            Some(version) => version.to_string(),
            None => read_current(&self.root, organization, bundle_id)?
                .ok_or_else(|| anyhow!("local MCP bundle current release is missing"))?,
        };
        let receipt = read_receipt(&self.root, organization, bundle_id, &selected)?
            .ok_or_else(|| anyhow!("local MCP bundle receipt is missing"))?;
        let bytes = fs::read(bundle_path(&self.root, organization, bundle_id, &selected))
            .context("failed to read local MCP bundle artifact")?;
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
        let receipt = read_receipt(&self.root, organization, bundle_id, bundle_version)?
            .ok_or_else(|| anyhow!("local MCP bundle release not found"))?;
        write_current(&self.root, organization, bundle_id, bundle_version)?;
        Ok(receipt)
    }

    async fn delete_local_version(
        &self,
        organization: &str,
        bundle_id: &str,
        bundle_version: &str,
    ) -> Result<()> {
        let directory = release_dir(&self.root, organization, bundle_id, bundle_version);
        if !directory.is_dir() {
            bail!("local MCP bundle release not found");
        }
        fs::remove_dir_all(directory)?;
        let remaining = local_versions(&self.root, organization, bundle_id)?;
        if remaining.is_empty() {
            fs::remove_dir_all(bundle_dir(&self.root, organization, bundle_id))?;
        } else if read_current(&self.root, organization, bundle_id)?.as_deref()
            == Some(bundle_version)
        {
            let highest = remaining
                .iter()
                .max_by(|left, right| semver_cmp(&left.bundle_version, &right.bundle_version))
                .expect("non-empty MCP release list must have a highest version");
            write_current(&self.root, organization, bundle_id, &highest.bundle_version)?;
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

fn write_release(root: &Path, receipt: &LocalMcpBundleReceipt, bytes: &[u8]) -> Result<()> {
    let directory = release_dir(
        root,
        &receipt.organization,
        &receipt.bundle_id,
        &receipt.bundle_version,
    );
    fs::create_dir_all(&directory)?;
    atomic_write(&directory.join("bundle.zip"), bytes)?;
    atomic_write(
        &directory.join("receipt.json"),
        &serde_json::to_vec_pretty(receipt)?,
    )
}

fn write_current(root: &Path, organization: &str, bundle_id: &str, version: &str) -> Result<()> {
    if read_receipt(root, organization, bundle_id, version)?.is_none() {
        bail!("local MCP bundle release not found");
    }
    atomic_write(
        &bundle_dir(root, organization, bundle_id).join("current"),
        version.as_bytes(),
    )
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

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("MCP bundle path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::now_v7()
    ));
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)?;
    Ok(())
}
