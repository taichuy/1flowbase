use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    config::ResolvedOfficialMcpBundleSourceConfig, official_plugin_registry::rewrite_github_raw_url,
};

const MAX_OFFICIAL_MCP_BUNDLE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct OfficialMcpBundleCatalogSource {
    pub source_kind: String,
    pub source_label: String,
    pub catalog_url: String,
}

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
    #[serde(default)]
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
    client: Client,
}

impl ApiOfficialMcpBundleRegistry {
    pub fn new(source: ResolvedOfficialMcpBundleSourceConfig) -> Self {
        Self {
            source_kind: source.source_kind,
            source_label: source.source_label,
            catalog_url: rewrite_github_raw_url(
                &source.catalog_url,
                source.github_proxy_url.as_deref(),
            ),
            github_proxy_url: source.github_proxy_url,
            client: Client::new(),
        }
    }

    async fn catalog_document(&self) -> Result<OfficialMcpCatalogDocument> {
        self.client
            .get(&self.catalog_url)
            .send()
            .await
            .context("failed to request official MCP bundle catalog")?
            .error_for_status()
            .context("official MCP bundle catalog returned an error status")?
            .json()
            .await
            .context("failed to decode official MCP bundle catalog")
    }

    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .context("failed to request official MCP bundle")?
            .error_for_status()
            .context("official MCP bundle download returned an error status")?
            .bytes()
            .await
            .context("failed to read official MCP bundle")?;
        if bytes.is_empty() || bytes.len() > MAX_OFFICIAL_MCP_BUNDLE_BYTES {
            bail!("official MCP bundle exceeds download budget");
        }
        Ok(bytes.to_vec())
    }
}

#[async_trait]
impl OfficialMcpBundleSourcePort for ApiOfficialMcpBundleRegistry {
    async fn list_catalog(&self) -> Result<OfficialMcpBundleCatalogSnapshot> {
        let mut document = self.catalog_document().await?;
        for entry in &mut document.bundles {
            entry.download_url =
                rewrite_github_raw_url(&entry.download_url, self.github_proxy_url.as_deref());
        }
        Ok(OfficialMcpBundleCatalogSnapshot {
            source: OfficialMcpBundleCatalogSource {
                source_kind: self.source_kind.clone(),
                source_label: self.source_label.clone(),
                catalog_url: self.catalog_url.clone(),
            },
            entries: document.bundles,
        })
    }

    async fn download_bundle(
        &self,
        organization: &str,
        bundle_id: &str,
    ) -> Result<DownloadedOfficialMcpBundle> {
        let catalog = self.list_catalog().await?;
        let entry = catalog
            .entries
            .into_iter()
            .find(|entry| entry.organization == organization && entry.bundle_id == bundle_id)
            .ok_or_else(|| anyhow::anyhow!("official MCP bundle not found"))?;
        let package_bytes = self.download_bytes(&entry.download_url).await?;
        if let Some(expected) = entry.artifact_sha256.as_deref() {
            let actual = format!("sha256:{:x}", Sha256::digest(&package_bytes));
            if actual != expected {
                bail!("official MCP bundle checksum mismatch");
            }
        }
        Ok(DownloadedOfficialMcpBundle {
            file_name: format!(
                "{}-{}-v{}.zip",
                entry.organization, entry.bundle_id, entry.latest_version
            ),
            package_bytes,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OfficialMcpCatalogDocument {
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    bundles: Vec<OfficialMcpBundleCatalogEntry>,
}
