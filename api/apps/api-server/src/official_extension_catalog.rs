use std::{collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    config::{ApiConfig, ResolvedOfficialExtensionCatalogSourceConfig},
    official_plugin_registry::rewrite_github_raw_url,
};

const EXTENSION_CATALOG_SCHEMA_VERSION: &str = "1flowbase.extension-catalog/v1";
const MAX_EXTENSION_CATALOG_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OfficialExtensionCatalogEntrySource {
    pub kind: String,
    pub locator: String,
    #[serde(flatten)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OfficialExtensionCatalogEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub organization: String,
    pub artifact: String,
    pub version: String,
    pub description: String,
    pub host_version_requirement: String,
    pub source: OfficialExtensionCatalogEntrySource,
    pub signature: Option<Value>,
    pub checksum: Option<String>,
    pub download_locator: Value,
    pub catalog_page: u32,
}

impl OfficialExtensionCatalogEntry {
    pub fn signing_key_id(&self) -> Option<&str> {
        self.signature
            .as_ref()?
            .get("key_id")?
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialExtensionCatalogPageMetadata {
    pub page: u32,
    pub cursor: String,
    pub checksum: String,
    pub locator: String,
    pub next_cursor: Option<String>,
    pub page_size: usize,
    pub total_entries: usize,
}

#[derive(Debug, Clone)]
pub struct OfficialExtensionCatalogPage {
    pub source_kind: String,
    pub category: String,
    pub metadata: OfficialExtensionCatalogPageMetadata,
    pub entries: Vec<OfficialExtensionCatalogEntry>,
}

#[derive(Debug, Clone)]
pub struct LocatedOfficialExtensionCatalogEntry {
    pub source_kind: String,
    pub entry: OfficialExtensionCatalogEntry,
}

#[async_trait]
pub trait OfficialExtensionCatalogSourcePort: Send + Sync {
    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage>;

    async fn find_entry(
        &self,
        category: &str,
        artifact_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>>;
}

#[derive(Clone)]
pub struct ApiOfficialExtensionCatalogSource {
    sources: Arc<BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>>,
    client: Client,
}

impl ApiOfficialExtensionCatalogSource {
    pub fn from_config(config: &ApiConfig) -> Self {
        Self {
            sources: Arc::new(config.official_extension_catalog_sources.clone()),
            client: Client::new(),
        }
    }

    pub fn new(sources: BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>) -> Self {
        Self {
            sources: Arc::new(sources),
            client: Client::new(),
        }
    }

    fn source(&self, category: &str) -> Result<&ResolvedOfficialExtensionCatalogSourceConfig> {
        self.sources.get(category).ok_or_else(|| {
            anyhow!("official extension catalog category is not configured: {category}")
        })
    }

    async fn fetch_index(
        &self,
        category: &str,
    ) -> Result<(
        ResolvedOfficialExtensionCatalogSourceConfig,
        CatalogIndexDocument,
    )> {
        let source = self.source(category)?.clone();
        let index_url =
            rewrite_github_raw_url(&source.index_url, source.github_proxy_url.as_deref());
        let bytes = self.download_document(&index_url).await?;
        let document = serde_json::from_slice::<CatalogIndexDocument>(&bytes)
            .context("failed to decode official extension catalog index")?;
        validate_index(&document, category)?;
        Ok((source, document))
    }

    async fn fetch_page(
        &self,
        source: &ResolvedOfficialExtensionCatalogSourceConfig,
        index: &CatalogIndexDocument,
        page_reference: &CatalogPageReference,
    ) -> Result<OfficialExtensionCatalogPage> {
        let locator =
            rewrite_github_raw_url(&page_reference.locator, source.github_proxy_url.as_deref());
        let bytes = self.download_document(&locator).await?;
        ensure_sha256(&bytes, &page_reference.checksum)?;
        let document = serde_json::from_slice::<CatalogPageDocument>(&bytes)
            .context("failed to decode official extension catalog page")?;
        validate_page(&document, &index.category, page_reference)?;

        Ok(OfficialExtensionCatalogPage {
            source_kind: source.source_kind.clone(),
            category: index.category.clone(),
            metadata: OfficialExtensionCatalogPageMetadata {
                page: document.page,
                cursor: document.cursor,
                checksum: page_reference.checksum.clone(),
                locator,
                next_cursor: document.next_cursor,
                page_size: index.page_size,
                total_entries: index.total_entries,
            },
            entries: document.entries,
        })
    }

    async fn download_document(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request official extension catalog from {url}"))?
            .error_for_status()
            .with_context(|| {
                format!("official extension catalog returned an error status for {url}")
            })?
            .bytes()
            .await
            .context("failed to read official extension catalog response body")?;
        if bytes.is_empty() || bytes.len() > MAX_EXTENSION_CATALOG_DOCUMENT_BYTES {
            bail!("official extension catalog document exceeds download budget");
        }
        Ok(bytes.to_vec())
    }
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for ApiOfficialExtensionCatalogSource {
    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        let (source, index) = self.fetch_index(category).await?;
        let requested_cursor = cursor
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&index.first_page.cursor);
        let page_reference = index
            .pages
            .iter()
            .find(|page| page.cursor == requested_cursor)
            .ok_or_else(|| anyhow!("official extension catalog cursor was not found"))?;
        self.fetch_page(&source, &index, page_reference).await
    }

    async fn find_entry(
        &self,
        category: &str,
        artifact_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        let (source, index) = self.fetch_index(category).await?;
        for page_reference in &index.pages {
            let page = self.fetch_page(&source, &index, page_reference).await?;
            if let Some(entry) = page
                .entries
                .into_iter()
                .find(|entry| entry.id == artifact_id)
            {
                return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                    source_kind: source.source_kind.clone(),
                    entry,
                }));
            }
        }
        Ok(None)
    }
}

#[derive(Debug, Deserialize)]
struct CatalogIndexDocument {
    schema_version: String,
    category: String,
    #[allow(dead_code)]
    generated_at: String,
    page_size: usize,
    total_entries: usize,
    first_page: CatalogFirstPage,
    pages: Vec<CatalogPageReference>,
}

#[derive(Debug, Deserialize)]
struct CatalogFirstPage {
    page: u32,
    cursor: String,
    locator: String,
}

#[derive(Debug, Deserialize)]
struct CatalogPageReference {
    page: u32,
    cursor: String,
    entry_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Deserialize)]
struct CatalogPageDocument {
    schema_version: String,
    category: String,
    page: u32,
    cursor: String,
    next_cursor: Option<String>,
    #[allow(dead_code)]
    next_page_locator: Option<String>,
    #[serde(default)]
    entries: Vec<OfficialExtensionCatalogEntry>,
}

fn validate_index(index: &CatalogIndexDocument, category: &str) -> Result<()> {
    if index.schema_version != EXTENSION_CATALOG_SCHEMA_VERSION || index.category != category {
        bail!("official extension catalog index contract mismatch");
    }
    if index.page_size == 0 || index.pages.is_empty() {
        bail!("official extension catalog index has no usable pages");
    }
    let first = index
        .pages
        .iter()
        .find(|page| page.page == index.first_page.page)
        .ok_or_else(|| anyhow!("official extension catalog first page is missing"))?;
    if first.cursor != index.first_page.cursor || first.locator != index.first_page.locator {
        bail!("official extension catalog first page reference mismatch");
    }
    Ok(())
}

fn validate_page(
    page: &CatalogPageDocument,
    category: &str,
    page_reference: &CatalogPageReference,
) -> Result<()> {
    if page.schema_version != EXTENSION_CATALOG_SCHEMA_VERSION
        || page.category != category
        || page.page != page_reference.page
        || page.cursor != page_reference.cursor
        || page.entries.len() != page_reference.entry_count
        || page
            .entries
            .iter()
            .any(|entry| entry.category != category || entry.catalog_page != page_reference.page)
    {
        bail!("official extension catalog page contract mismatch");
    }
    Ok(())
}

fn ensure_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = format!("sha256:{:x}", Sha256::digest(bytes));
    if actual != expected {
        bail!("official extension catalog page checksum mismatch");
    }
    Ok(())
}
