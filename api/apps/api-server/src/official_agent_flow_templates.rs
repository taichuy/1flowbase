use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    flow::{AgentFlowTemplateApplication, AgentFlowTemplatePackage},
    ports::CacheStore,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::Duration;

use crate::{
    config::ResolvedOfficialAgentFlowTemplateSourceConfig,
    official_plugin_registry::rewrite_github_raw_url,
};

fn official_agent_flow_template_catalog_cache_ttl() -> Duration {
    Duration::hours(2)
}

#[derive(Debug, Clone)]
pub struct OfficialAgentFlowTemplateCatalogSource {
    pub source_kind: String,
    pub source_label: String,
    pub index_url: String,
}

#[derive(Debug, Clone)]
pub struct OfficialAgentFlowTemplateCatalogEntry {
    pub workflow_id: String,
    pub schema_version: String,
    pub application: AgentFlowTemplateApplication,
    pub template_url: String,
    pub template_sha256: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct OfficialAgentFlowTemplateCatalogPage {
    pub page: u32,
    pub page_size: usize,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OfficialAgentFlowTemplateCatalogSnapshot {
    pub source: OfficialAgentFlowTemplateCatalogSource,
    pub page: OfficialAgentFlowTemplateCatalogPage,
    pub entries: Vec<OfficialAgentFlowTemplateCatalogEntry>,
}

#[async_trait]
pub trait OfficialAgentFlowTemplateSourcePort: Send + Sync {
    async fn list_catalog_page(
        &self,
        cursor: Option<String>,
    ) -> Result<OfficialAgentFlowTemplateCatalogSnapshot>;
    async fn download_template(&self, workflow_id: &str) -> Result<AgentFlowTemplatePackage>;
}

#[derive(Clone)]
pub struct ApiOfficialAgentFlowTemplateRegistry {
    source_kind: String,
    source_label: String,
    index_url: String,
    github_proxy_url: Option<String>,
    client: Client,
    catalog_cache: Arc<dyn CacheStore>,
}

impl ApiOfficialAgentFlowTemplateRegistry {
    pub fn new(
        source: ResolvedOfficialAgentFlowTemplateSourceConfig,
        catalog_cache: Arc<dyn CacheStore>,
    ) -> Self {
        let index_url =
            rewrite_github_raw_url(&source.index_url, source.github_proxy_url.as_deref());
        Self {
            source_kind: source.source_kind,
            source_label: source.source_label,
            index_url,
            github_proxy_url: source.github_proxy_url,
            client: Client::new(),
            catalog_cache,
        }
    }

    async fn fetch_index(&self) -> Result<AgentFlowCatalogIndexDocument> {
        if let Some(index) = self.cached_index().await {
            return Ok(index);
        }

        let bytes = self.download_bytes(&self.index_url).await?;
        let index = serde_json::from_slice::<AgentFlowCatalogIndexDocument>(&bytes)
            .context("failed to decode official AgentFlow template catalog index")?;
        self.store_index(&index).await;
        Ok(index)
    }

    async fn cached_index(&self) -> Option<AgentFlowCatalogIndexDocument> {
        self.catalog_cache
            .get_json(&self.index_cache_key())
            .await
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    async fn store_index(&self, index: &AgentFlowCatalogIndexDocument) {
        if let Ok(value) = serde_json::to_value(index) {
            let _ = self
                .catalog_cache
                .set_json(
                    &self.index_cache_key(),
                    value,
                    Some(official_agent_flow_template_catalog_cache_ttl()),
                )
                .await;
        }
    }

    async fn cached_page(&self, page: u32) -> Option<AgentFlowCatalogPageDocument> {
        self.catalog_cache
            .get_json(&self.page_cache_key(page))
            .await
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    async fn store_page(&self, page: u32, document: &AgentFlowCatalogPageDocument) {
        if let Ok(value) = serde_json::to_value(document) {
            let _ = self
                .catalog_cache
                .set_json(
                    &self.page_cache_key(page),
                    value,
                    Some(official_agent_flow_template_catalog_cache_ttl()),
                )
                .await;
        }
    }

    async fn fetch_page(&self, page: u32) -> Result<AgentFlowCatalogPageDocument> {
        if let Some(document) = self.cached_page(page).await {
            return Ok(document);
        }

        let index = self.fetch_index().await?;
        let page_ref = index
            .pages
            .iter()
            .find(|entry| entry.page == page)
            .ok_or_else(|| anyhow!("official AgentFlow template catalog page not found"))?;
        let page_url = rewrite_github_raw_url(&page_ref.url, self.github_proxy_url.as_deref());
        let bytes = self.download_bytes(&page_url).await?;
        ensure_sha256(&bytes, &page_ref.sha256)?;
        let mut document = serde_json::from_slice::<AgentFlowCatalogPageDocument>(&bytes)
            .context("failed to decode official AgentFlow template catalog page")?;
        normalize_page_urls(&mut document, self.github_proxy_url.as_deref());
        self.store_page(page, &document).await;

        Ok(document)
    }

    fn source_cache_fingerprint(&self) -> String {
        format!("{:x}", Sha256::digest(self.index_url.as_bytes()))
    }

    fn index_cache_key(&self) -> String {
        format!(
            "official-agent-flow-templates:v1:index:{}",
            self.source_cache_fingerprint()
        )
    }

    fn page_cache_key(&self, page: u32) -> String {
        format!(
            "official-agent-flow-templates:v1:page:{}:{}",
            self.source_cache_fingerprint(),
            page
        )
    }

    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| {
                format!("failed to request official AgentFlow template source from {url}")
            })?
            .error_for_status()
            .with_context(|| {
                format!("official AgentFlow template source returned an error status for {url}")
            })?
            .bytes()
            .await
            .context("failed to read official AgentFlow template response body")?
            .to_vec())
    }
}

#[async_trait]
impl OfficialAgentFlowTemplateSourcePort for ApiOfficialAgentFlowTemplateRegistry {
    async fn list_catalog_page(
        &self,
        cursor: Option<String>,
    ) -> Result<OfficialAgentFlowTemplateCatalogSnapshot> {
        let page = parse_page_cursor(cursor.as_deref())?;
        let document = self.fetch_page(page).await?;

        Ok(OfficialAgentFlowTemplateCatalogSnapshot {
            source: OfficialAgentFlowTemplateCatalogSource {
                source_kind: self.source_kind.clone(),
                source_label: self.source_label.clone(),
                index_url: self.index_url.clone(),
            },
            page: OfficialAgentFlowTemplateCatalogPage {
                page: document.page,
                page_size: document.page_size,
                next_cursor: document
                    .next_page_url
                    .as_ref()
                    .map(|_| document.page.saturating_add(1).to_string()),
            },
            entries: document
                .entries
                .into_iter()
                .map(|entry| OfficialAgentFlowTemplateCatalogEntry {
                    workflow_id: entry.workflow_id,
                    schema_version: entry.schema_version,
                    application: entry.application,
                    template_url: entry.template_url,
                    template_sha256: entry.template_sha256,
                    updated_at: entry.updated_at,
                })
                .collect(),
        })
    }

    async fn download_template(&self, workflow_id: &str) -> Result<AgentFlowTemplatePackage> {
        let index = self.fetch_index().await?;
        for page_ref in index.pages {
            let page = self.fetch_page(page_ref.page).await?;
            if let Some(entry) = page
                .entries
                .into_iter()
                .find(|entry| entry.workflow_id == workflow_id)
            {
                let bytes = self.download_bytes(&entry.template_url).await?;
                ensure_sha256(&bytes, &entry.template_sha256)?;
                return serde_json::from_slice::<AgentFlowTemplatePackage>(&bytes)
                    .context("failed to decode official AgentFlow template package");
            }
        }

        Err(ControlPlaneError::NotFound("official_agent_flow_template").into())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentFlowCatalogIndexDocument {
    #[allow(dead_code)]
    version: u32,
    #[allow(dead_code)]
    generated_at: Option<String>,
    #[allow(dead_code)]
    page_size: usize,
    #[allow(dead_code)]
    total_entries: usize,
    #[allow(dead_code)]
    first_page_url: Option<String>,
    #[serde(default)]
    pages: Vec<AgentFlowCatalogIndexPage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentFlowCatalogIndexPage {
    page: u32,
    url: String,
    #[allow(dead_code)]
    entry_count: usize,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentFlowCatalogPageDocument {
    #[allow(dead_code)]
    version: u32,
    page: u32,
    page_size: usize,
    next_page_url: Option<String>,
    #[serde(default)]
    entries: Vec<AgentFlowCatalogPageEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentFlowCatalogPageEntry {
    workflow_id: String,
    schema_version: String,
    application: AgentFlowTemplateApplication,
    template_url: String,
    template_sha256: String,
    updated_at: String,
}

fn parse_page_cursor(cursor: Option<&str>) -> Result<u32> {
    let Some(cursor) = cursor.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(1);
    };

    let page = cursor
        .parse::<u32>()
        .context("invalid official AgentFlow template catalog cursor")?;
    if page == 0 {
        bail!("invalid official AgentFlow template catalog cursor");
    }
    Ok(page)
}

fn normalize_page_urls(
    document: &mut AgentFlowCatalogPageDocument,
    github_proxy_url: Option<&str>,
) {
    document.next_page_url = document
        .next_page_url
        .as_deref()
        .map(|url| rewrite_github_raw_url(url, github_proxy_url));
    for entry in &mut document.entries {
        entry.template_url = rewrite_github_raw_url(&entry.template_url, github_proxy_url);
    }
}

fn ensure_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = format!("sha256:{:x}", Sha256::digest(bytes));
    if actual != expected {
        bail!("official AgentFlow template checksum mismatch");
    }
    Ok(())
}
