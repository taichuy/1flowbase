use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{
    DownloadedOfficialPluginPackage, OfficialPluginArtifact, OfficialPluginCatalogFreshness,
    OfficialPluginCatalogSnapshot, OfficialPluginCatalogSource, OfficialPluginI18nSummary,
    OfficialPluginSourceEntry, OfficialPluginSourcePort,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    config::{
        ApiConfig, ResolvedOfficialExtensionCatalogSourceConfig,
        DEFAULT_OFFICIAL_EXTENSION_ARTIFACT_DOWNLOAD_TIMEOUT,
        DEFAULT_OFFICIAL_EXTENSION_CATALOG_REQUEST_TIMEOUT,
        DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
    },
    network_egress_client::{NetworkEgressExecutionScope, NetworkEgressHttpClientResolver},
    official_plugin_registry::rewrite_github_raw_url,
};

#[path = "official_extension_catalog/catalog_port.rs"]
mod catalog_port;
#[path = "official_extension_catalog/runtime_projection.rs"]
mod runtime_projection;

pub use runtime_projection::ApiOfficialRuntimeExtensionSource;

const EXTENSION_CATALOG_SCHEMA_VERSION: &str = "1flowbase.extension-catalog/v1";
const EXTENSION_CATALOG_SEARCH_SCHEMA_VERSION: &str = "1flowbase.extension-catalog-search/v1";
const MAX_EXTENSION_CATALOG_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXTENSION_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialExtensionCatalogFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Error)]
#[error("official extension catalog snapshot is temporarily inconsistent: {source}")]
pub struct OfficialExtensionCatalogUnavailable {
    #[source]
    source: anyhow::Error,
}

#[derive(Debug, Error)]
pub enum OfficialExtensionArtifactError {
    #[error("extension release is not published on {host} (HTTP {status}); retry after the publisher completes the release")]
    NotFound { host: String, status: u16 },
    #[error("extension source {host} rejected the artifact request (HTTP {status}); retry or check the configured mirror")]
    UpstreamRejected { host: String, status: u16 },
    #[error(
        "extension source {host} is unreachable; retry or check the configured mirror: {source}"
    )]
    NetworkUnavailable {
        host: String,
        #[source]
        source: reqwest::Error,
    },
    #[error(
        "extension source {host} interrupted the artifact response; retry the download: {source}"
    )]
    ResponseInterrupted {
        host: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("extension artifact exceeds the allowed download size")]
    DownloadBudgetExceeded,
    #[error("extension source returned an empty artifact")]
    EmptyArtifact,
    #[error("extension artifact download failed; retry or check the configured source")]
    DownloadUnavailable {
        #[source]
        source: anyhow::Error,
    },
    #[error("extension artifact checksum verification failed; the file was not installed")]
    ChecksumMismatch,
    #[error("extension artifact signature verification failed; the file was not installed")]
    SignatureInvalid,
}

impl OfficialExtensionArtifactError {
    pub fn api_code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "extension_artifact_not_published",
            Self::ChecksumMismatch => "extension_artifact_checksum_mismatch",
            Self::SignatureInvalid => "extension_artifact_signature_invalid",
            Self::UpstreamRejected { .. } => "extension_artifact_upstream_rejected",
            Self::NetworkUnavailable { .. } | Self::ResponseInterrupted { .. } => {
                "extension_artifact_network_unavailable"
            }
            Self::DownloadBudgetExceeded | Self::EmptyArtifact => {
                "extension_artifact_invalid_response"
            }
            Self::DownloadUnavailable { .. } => "extension_artifact_download_unavailable",
        }
    }

    pub fn host(&self) -> Option<&str> {
        match self {
            Self::NotFound { host, .. }
            | Self::UpstreamRejected { host, .. }
            | Self::NetworkUnavailable { host, .. }
            | Self::ResponseInterrupted { host, .. } => Some(host),
            _ => None,
        }
    }

    pub fn upstream_status(&self) -> Option<u16> {
        match self {
            Self::NotFound { status, .. } | Self::UpstreamRejected { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl OfficialExtensionCatalogUnavailable {
    pub fn new(source: anyhow::Error) -> Self {
        Self { source }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OfficialExtensionCatalogEntrySource {
    pub kind: String,
    pub locator: String,
    #[serde(flatten)]
    pub metadata: BTreeMap<String, Value>,
}

impl OfficialExtensionCatalogEntrySource {
    pub(crate) fn client_collector(&self) -> Result<Option<domain::ClientCollectorDescriptor>> {
        let Some(value) = self.metadata.get("distribution_kind") else {
            return Ok(None);
        };
        let _: domain::ClientCollectorDistributionKind = serde_json::from_value(value.clone())?;
        let descriptor: domain::ClientCollectorDescriptor = serde_json::from_value(
            self.metadata
                .get("client_collector")
                .context("client collector descriptor missing")?
                .clone(),
        )?;
        if descriptor.execution_target != "client"
            || descriptor.protocol_version != "1flowbase.agent-logs/v1"
            || descriptor.collector_code.is_empty()
            || descriptor.source_client.is_empty()
            || descriptor.display_name.is_empty()
        {
            bail!("invalid client collector descriptor");
        }
        if self.metadata.contains_key("platform_release_assets") {
            bail!("client collector distribution must be host independent");
        }
        Ok(Some(descriptor))
    }
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
    pub slot_codes: Vec<String>,
    pub keywords: Vec<String>,
    pub source: OfficialExtensionCatalogEntrySource,
    pub signature: Option<Value>,
    pub checksum: Option<String>,
    pub download_locator: Value,
    pub catalog_page: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialExtensionCatalogSearchQuery {
    pub slot_code: Option<String>,
    pub q: Option<String>,
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OfficialExtensionCatalogSearchResult {
    pub source_kind: String,
    pub category: String,
    pub freshness: OfficialExtensionCatalogFreshness,
    pub snapshot_checksum: String,
    pub snapshot_locator: String,
    pub total_entries: usize,
    pub next_cursor: Option<String>,
    pub entries: Vec<OfficialExtensionCatalogEntry>,
}

#[derive(Debug, Clone)]
pub struct CachedOfficialExtensionCatalogEntries {
    pub source_kind: String,
    pub snapshot_locator: String,
    pub entries: Vec<OfficialExtensionCatalogEntry>,
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
    pub freshness: OfficialExtensionCatalogFreshness,
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

#[derive(Debug, Clone, PartialEq)]
pub struct OfficialExtensionArtifactDescriptor {
    pub locator_kind: String,
    pub locator: String,
    pub expected_checksum: Option<String>,
    pub signature: Option<Value>,
    pub platform: Option<OfficialExtensionArtifactPlatform>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialExtensionArtifactPlatform {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target: String,
}

#[derive(Debug, Clone)]
pub struct DownloadedOfficialExtensionArtifact {
    pub descriptor: OfficialExtensionArtifactDescriptor,
    pub file_name: String,
    pub artifact_bytes: Vec<u8>,
}

#[async_trait]
pub trait OfficialExtensionCatalogSourcePort: Send + Sync {
    async fn search(
        &self,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult>;

    async fn search_for_workspace(
        &self,
        _workspace_id: Uuid,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult> {
        self.search(category, query).await
    }

    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage>;

    async fn list_page_for_workspace(
        &self,
        _workspace_id: Uuid,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        self.list_page(category, cursor).await
    }

    fn cached_verified_entries(
        &self,
        _category: &str,
        _slot_code: Option<&str>,
    ) -> Option<CachedOfficialExtensionCatalogEntries> {
        None
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>>;

    async fn find_entry_for_workspace(
        &self,
        _workspace_id: Uuid,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        self.find_entry(category, catalog_id).await
    }

    fn resolve_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<OfficialExtensionArtifactDescriptor>;

    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact>;

    async fn download_artifact_for_workspace(
        &self,
        _workspace_id: Uuid,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact> {
        self.download_artifact(entry).await
    }
}

#[derive(Clone)]
pub struct ApiOfficialExtensionCatalogSource {
    sources: Arc<BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>>,
    catalog_client: Client,
    artifact_client: Client,
    source_connect_timeout: Duration,
    catalog_request_timeout: Duration,
    artifact_download_timeout: Duration,
    network_egress: Option<NetworkEgressHttpClientResolver>,
    last_success: Arc<Mutex<HashMap<CatalogCacheKey, OfficialExtensionCatalogPage>>>,
    search_snapshots: Arc<Mutex<HashMap<String, VerifiedSearchSnapshot>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CatalogCacheKey {
    category: String,
    cursor: Option<String>,
}

#[derive(Debug, Clone)]
struct CatalogEndpoint {
    source_kind: String,
    index_url: String,
    github_proxy_url: Option<String>,
}

#[derive(Debug, Clone)]
struct VerifiedSearchSnapshot {
    source_kind: String,
    endpoint: CatalogEndpoint,
    checksum: String,
    locator: String,
    index: CatalogIndexDocument,
    document: CatalogSearchDocument,
}

struct CatalogSearchSelection<'a> {
    selected: Vec<&'a CatalogSearchEntry>,
    total_entries: usize,
    next_cursor: Option<String>,
}

impl ApiOfficialExtensionCatalogSource {
    pub fn from_config(config: &ApiConfig) -> Self {
        Self {
            sources: Arc::new(config.official_extension_catalog_sources.clone()),
            catalog_client: extension_source_client(
                config.official_extension_source_connect_timeout,
                config.official_extension_catalog_request_timeout,
            ),
            artifact_client: extension_source_client(
                config.official_extension_source_connect_timeout,
                config.official_extension_artifact_download_timeout,
            ),
            source_connect_timeout: config.official_extension_source_connect_timeout,
            catalog_request_timeout: config.official_extension_catalog_request_timeout,
            artifact_download_timeout: config.official_extension_artifact_download_timeout,
            network_egress: None,
            last_success: Arc::new(Mutex::new(HashMap::new())),
            search_snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new(sources: BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>) -> Self {
        Self {
            sources: Arc::new(sources),
            catalog_client: extension_source_client(
                DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
                DEFAULT_OFFICIAL_EXTENSION_CATALOG_REQUEST_TIMEOUT,
            ),
            artifact_client: extension_source_client(
                DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
                DEFAULT_OFFICIAL_EXTENSION_ARTIFACT_DOWNLOAD_TIMEOUT,
            ),
            source_connect_timeout: DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
            catalog_request_timeout: DEFAULT_OFFICIAL_EXTENSION_CATALOG_REQUEST_TIMEOUT,
            artifact_download_timeout: DEFAULT_OFFICIAL_EXTENSION_ARTIFACT_DOWNLOAD_TIMEOUT,
            network_egress: None,
            last_success: Arc::new(Mutex::new(HashMap::new())),
            search_snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_request_timeouts(
        sources: BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>,
        catalog_request_timeout: Duration,
        artifact_download_timeout: Duration,
    ) -> Self {
        Self {
            sources: Arc::new(sources),
            catalog_client: extension_source_client(
                DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
                catalog_request_timeout,
            ),
            artifact_client: extension_source_client(
                DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
                artifact_download_timeout,
            ),
            source_connect_timeout: DEFAULT_OFFICIAL_EXTENSION_SOURCE_CONNECT_TIMEOUT,
            catalog_request_timeout,
            artifact_download_timeout,
            network_egress: None,
            last_success: Arc::new(Mutex::new(HashMap::new())),
            search_snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_network_egress(mut self, resolver: NetworkEgressHttpClientResolver) -> Self {
        self.network_egress = Some(resolver);
        self
    }

    async fn source_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<(Self, Option<NetworkEgressExecutionScope>)> {
        let Some(resolver) = self.network_egress.as_ref() else {
            return Ok((self.clone(), None));
        };
        let lease = resolver
            .acquire(
                workspace_id,
                domain::NetworkEgressConsumerSelector::GithubOfficialSources,
            )
            .await?;
        let Some(lease) = lease else {
            return Ok((self.clone(), None));
        };
        let mut routed = self.clone();
        routed.catalog_client = lease
            .http_client_with_timeouts(self.source_connect_timeout, self.catalog_request_timeout)?;
        routed.network_egress = None;
        Ok((routed, Some(lease)))
    }

    async fn finish_workspace_request<T>(
        result: Result<T>,
        lease: Option<NetworkEgressExecutionScope>,
    ) -> Result<T> {
        let release = match lease {
            Some(lease) => lease.release().await,
            None => Ok(()),
        };
        match (result, release) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }

    fn source(&self, category: &str) -> Result<&ResolvedOfficialExtensionCatalogSourceConfig> {
        self.sources.get(category).ok_or_else(|| {
            anyhow!("official extension catalog category is not configured: {category}")
        })
    }

    fn endpoints(&self, category: &str) -> Result<Vec<CatalogEndpoint>> {
        let source = self.source(category)?;
        let primary_url =
            rewrite_github_raw_url(&source.index_url, source.github_proxy_url.as_deref());
        let primary = CatalogEndpoint {
            source_kind: if primary_url != source.index_url {
                "configured_proxy".to_string()
            } else {
                source.source_kind.clone()
            },
            index_url: primary_url.clone(),
            github_proxy_url: source.github_proxy_url.clone(),
        };
        if primary_url == source.official_index_url {
            return Ok(vec![primary]);
        }
        Ok(vec![
            primary,
            CatalogEndpoint {
                source_kind: "official_repository".to_string(),
                index_url: source.official_index_url.clone(),
                github_proxy_url: None,
            },
        ])
    }

    async fn fetch_index(
        &self,
        category: &str,
        endpoint: &CatalogEndpoint,
    ) -> Result<CatalogIndexDocument> {
        let bytes = self.download_document(&endpoint.index_url).await?;
        let document = serde_json::from_slice::<CatalogIndexDocument>(&bytes)
            .context("failed to decode official extension catalog index")?;
        validate_index(&document, category)?;
        Ok(document)
    }

    async fn fetch_page(
        &self,
        endpoint: &CatalogEndpoint,
        index: &CatalogIndexDocument,
        page_reference: &CatalogPageReference,
    ) -> Result<OfficialExtensionCatalogPage> {
        let locator = rewrite_github_raw_url(
            &page_reference.locator,
            endpoint.github_proxy_url.as_deref(),
        );
        let bytes = self.download_document(&locator).await?;
        ensure_sha256(&bytes, &page_reference.checksum)?;
        let document = serde_json::from_slice::<CatalogPageDocument>(&bytes)
            .context("failed to decode official extension catalog page")?;
        validate_page(&document, &index.category, page_reference)?;

        Ok(OfficialExtensionCatalogPage {
            source_kind: endpoint.source_kind.clone(),
            category: index.category.clone(),
            metadata: OfficialExtensionCatalogPageMetadata {
                page: document.page,
                cursor: document.cursor,
                checksum: page_reference.checksum.clone(),
                locator,
                next_cursor: document.next_cursor,
                page_size: index.page_size,
                total_entries: index.total_entries,
                freshness: OfficialExtensionCatalogFreshness::Fresh,
            },
            entries: document.entries,
        })
    }

    async fn load_page(
        &self,
        category: &str,
        cursor: Option<&str>,
        endpoint: &CatalogEndpoint,
    ) -> Result<OfficialExtensionCatalogPage> {
        let index = self.fetch_index(category, endpoint).await?;
        let requested_cursor = cursor
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&index.first_page.cursor);
        let page_reference = index
            .pages
            .iter()
            .find(|page| page.cursor == requested_cursor)
            .ok_or_else(|| anyhow!("official extension catalog cursor was not found"))?;
        self.fetch_page(endpoint, &index, page_reference).await
    }

    async fn load_search_snapshot(
        &self,
        category: &str,
        endpoint: &CatalogEndpoint,
    ) -> Result<(CatalogIndexDocument, VerifiedSearchSnapshot)> {
        let index = self.fetch_index(category, endpoint).await?;
        let locator = rewrite_github_raw_url(
            &index.search_index.locator,
            endpoint.github_proxy_url.as_deref(),
        );
        let bytes = self.download_document(&locator).await?;
        ensure_sha256(&bytes, &index.search_index.checksum)?;
        let document = serde_json::from_slice::<CatalogSearchDocument>(&bytes)
            .context("failed to decode official extension catalog search index")?;
        validate_search_document(&document, &index)?;
        let snapshot = VerifiedSearchSnapshot {
            source_kind: endpoint.source_kind.clone(),
            endpoint: endpoint.clone(),
            checksum: index.search_index.checksum.clone(),
            locator,
            index: index.clone(),
            document,
        };
        Ok((index, snapshot))
    }

    fn cached_search_snapshot(&self, category: &str) -> Option<VerifiedSearchSnapshot> {
        self.search_snapshots.lock().ok()?.get(category).cloned()
    }

    fn remember_search_snapshot(&self, category: &str, snapshot: &VerifiedSearchSnapshot) {
        if let Ok(mut cache) = self.search_snapshots.lock() {
            cache.insert(category.to_string(), snapshot.clone());
        }
    }

    async fn fetch_selected_search_entries(
        &self,
        category: &str,
        index: &CatalogIndexDocument,
        snapshot: &VerifiedSearchSnapshot,
        selected: &[&CatalogSearchEntry],
    ) -> Result<Vec<OfficialExtensionCatalogEntry>> {
        let selected_ids = selected
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<HashSet<_>>();
        let mut entries_by_id = HashMap::new();
        let mut visited_pages = HashSet::new();
        for search_entry in selected {
            if !visited_pages.insert(search_entry.catalog_page.cursor.clone()) {
                continue;
            }
            let page_reference = index
                .pages
                .iter()
                .find(|reference| {
                    reference.page == search_entry.catalog_page.page
                        && reference.cursor == search_entry.catalog_page.cursor
                        && reference.checksum == search_entry.catalog_page.checksum
                        && reference.locator == search_entry.catalog_page.locator
                })
                .ok_or_else(|| {
                    anyhow!("catalog search page reference is not in the catalog index")
                })?;
            let page = if let Some(page) =
                self.cached_verified_page(category, &search_entry.catalog_page)
            {
                page
            } else {
                let page = self
                    .fetch_page(&snapshot.endpoint, index, page_reference)
                    .await?;
                self.remember_page(
                    Self::cache_key(category, Some(&page.metadata.cursor)),
                    &page,
                );
                page
            };
            for entry in page.entries {
                if selected_ids.contains(entry.id.as_str()) {
                    entries_by_id.insert(entry.id.clone(), entry);
                }
            }
        }
        selected
            .iter()
            .map(|search_entry| {
                entries_by_id.remove(&search_entry.id).ok_or_else(|| {
                    anyhow!("catalog search result is missing from its verified source page")
                })
            })
            .collect()
    }

    fn cached_selected_search_entries(
        &self,
        category: &str,
        selected: &[&CatalogSearchEntry],
    ) -> Option<Vec<OfficialExtensionCatalogEntry>> {
        selected
            .iter()
            .map(|search_entry| {
                self.cached_verified_page(category, &search_entry.catalog_page)?
                    .entries
                    .into_iter()
                    .find(|entry| entry.id == search_entry.id)
            })
            .collect()
    }

    fn cached_verified_page(
        &self,
        category: &str,
        reference: &CatalogSearchPageReference,
    ) -> Option<OfficialExtensionCatalogPage> {
        let page = self
            .last_success
            .lock()
            .ok()?
            .values()
            .find(|page| {
                page.category == category
                    && page.metadata.cursor == reference.cursor
                    && page.metadata.page == reference.page
                    && page.metadata.checksum == reference.checksum
                    && page.metadata.locator.ends_with(&reference.locator)
            })?
            .clone();
        Some(page)
    }

    fn cache_key(category: &str, cursor: Option<&str>) -> CatalogCacheKey {
        CatalogCacheKey {
            category: category.to_string(),
            cursor: cursor
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        }
    }

    fn remember_page(&self, key: CatalogCacheKey, page: &OfficialExtensionCatalogPage) {
        if let Ok(mut cache) = self.last_success.lock() {
            cache.insert(key, page.clone());
        }
    }

    fn stale_page(&self, key: &CatalogCacheKey) -> Option<OfficialExtensionCatalogPage> {
        let mut page = self.last_success.lock().ok()?.get(key)?.clone();
        page.metadata.freshness = OfficialExtensionCatalogFreshness::Stale;
        Some(page)
    }

    async fn download_document(&self, url: &str) -> Result<Vec<u8>> {
        self.download_once_with_budget(
            &self.catalog_client,
            url,
            MAX_EXTENSION_CATALOG_DOCUMENT_BYTES,
            "official extension catalog document",
        )
        .await
        .map_err(anyhow::Error::new)
    }

    async fn download_artifact_bytes(&self, url: &str, maximum_bytes: usize) -> Result<Vec<u8>> {
        let mut last_error = None;
        for attempt in 0..2 {
            match self
                .download_once_with_budget(
                    &self.artifact_client,
                    url,
                    maximum_bytes,
                    "official extension artifact",
                )
                .await
            {
                Ok(bytes) => return Ok(bytes),
                Err(error) => {
                    let retry = attempt == 0 && error.is_retryable();
                    last_error = Some(error);
                    if !retry {
                        break;
                    }
                }
            }
        }
        Err(anyhow::Error::new(artifact_download_error(
            last_error.expect("artifact download loop always records a failed attempt"),
        )))
    }

    async fn download_artifact_bytes_for_workspace(
        &self,
        workspace_id: Uuid,
        url: &str,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>> {
        let Some(resolver) = self.network_egress.as_ref() else {
            return self.download_artifact_bytes(url, maximum_bytes).await;
        };
        let lease = resolver
            .acquire(
                workspace_id,
                domain::NetworkEgressConsumerSelector::GithubOfficialSources,
            )
            .await?;
        let Some(lease) = lease else {
            return self.download_artifact_bytes(url, maximum_bytes).await;
        };
        self.download_artifact_with_lease(url, lease, maximum_bytes)
            .await
    }

    async fn download_artifact_with_lease(
        &self,
        url: &str,
        lease: NetworkEgressExecutionScope,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>> {
        let client = lease.http_client_with_timeouts(
            self.source_connect_timeout,
            self.artifact_download_timeout,
        )?;
        let result =
            download_with_client_budget(&client, url, maximum_bytes, "official extension artifact")
                .await;
        let release = lease.release().await;
        match (result, release) {
            (Ok(bytes), Ok(())) => Ok(bytes),
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }

    async fn download_once_with_budget(
        &self,
        client: &Client,
        url: &str,
        max_bytes: usize,
        resource: &'static str,
    ) -> std::result::Result<Vec<u8>, ExtensionDownloadFailure> {
        let response =
            client
                .get(url)
                .send()
                .await
                .map_err(|source| ExtensionDownloadFailure::Request {
                    resource,
                    url: url.to_string(),
                    source,
                })?;
        let status = response.status();
        if !status.is_success() {
            return Err(ExtensionDownloadFailure::Status {
                resource,
                url: url.to_string(),
                status,
            });
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|source| ExtensionDownloadFailure::Body {
                resource,
                url: url.to_string(),
                source,
            })?;
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                return Err(ExtensionDownloadFailure::Budget { resource });
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(ExtensionDownloadFailure::Empty { resource });
        }
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogIndexDocument {
    schema_version: String,
    category: String,
    #[allow(dead_code)]
    generated_at: String,
    page_size: usize,
    total_entries: usize,
    first_page: CatalogFirstPage,
    pages: Vec<CatalogPageReference>,
    search_index: CatalogSearchReference,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogFirstPage {
    page: u32,
    cursor: String,
    locator: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogPageReference {
    page: u32,
    cursor: String,
    entry_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogSearchReference {
    schema_version: String,
    entry_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogSearchDocument {
    schema_version: String,
    category: String,
    #[allow(dead_code)]
    generated_at: String,
    source_fingerprint: String,
    #[serde(default)]
    entries: Vec<CatalogSearchEntry>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct CatalogSearchEntry {
    id: String,
    name: String,
    category: String,
    organization: String,
    artifact: String,
    version: String,
    description: String,
    host_version_requirement: String,
    source: OfficialExtensionCatalogEntrySource,
    signature: Option<Value>,
    checksum: Option<String>,
    #[serde(default)]
    slot_codes: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    catalog_page: CatalogSearchPageReference,
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogSearchPageReference {
    page: u32,
    cursor: String,
    checksum: String,
    locator: String,
}

impl CatalogSearchEntry {
    fn matches(&self, needle: &str) -> bool {
        let source_value = |key: &str| {
            self.source
                .metadata
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
        };
        [
            self.id.as_str(),
            self.name.as_str(),
            self.organization.as_str(),
            self.artifact.as_str(),
            source_value("provider_code"),
            source_value("protocol"),
            self.description.as_str(),
        ]
        .into_iter()
        .chain(self.keywords.iter().map(String::as_str))
        .any(|value| value.to_lowercase().contains(needle))
    }
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

fn ensure_sha256(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)));
    if actual != expected {
        bail!("official extension catalog page checksum mismatch");
    }
    Ok(())
}

fn artifact_download_budget(entry: &OfficialExtensionCatalogEntry) -> Result<usize> {
    // Collector releases include opaque native archives for every client platform.
    Ok(if entry.source.client_collector()?.is_some() {
        usize::MAX
    } else {
        MAX_EXTENSION_ARTIFACT_BYTES
    })
}

#[path = "official_extension_catalog/catalog_validation.rs"]
mod catalog_validation;

#[path = "official_extension_catalog/artifact_download.rs"]
mod artifact_download;

use catalog_validation::*;

use artifact_download::*;
