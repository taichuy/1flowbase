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

use crate::{
    config::{ApiConfig, ResolvedOfficialExtensionCatalogSourceConfig},
    official_plugin_registry::rewrite_github_raw_url,
};

const EXTENSION_CATALOG_SCHEMA_VERSION: &str = "1flowbase.extension-catalog/v1";
const EXTENSION_CATALOG_SEARCH_SCHEMA_VERSION: &str = "1flowbase.extension-catalog-search/v1";
const MAX_EXTENSION_CATALOG_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXTENSION_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
const EXTENSION_SOURCE_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const EXTENSION_SOURCE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialExtensionCatalogFreshness {
    Fresh,
    Stale,
}

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
    pub snapshot_checksum: String,
    pub snapshot_locator: String,
    pub total_entries: usize,
    pub next_cursor: Option<String>,
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

    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage>;

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>>;

    fn resolve_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<OfficialExtensionArtifactDescriptor>;

    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact>;
}

#[derive(Clone)]
pub struct ApiOfficialRuntimeExtensionSource {
    catalog: Arc<dyn OfficialExtensionCatalogSourcePort>,
    trust_mode: String,
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    projection_cache: Arc<Mutex<RuntimeExtensionProjectionCache>>,
}

#[derive(Default)]
struct RuntimeExtensionProjectionCache {
    entries: HashMap<String, ProjectedRuntimeExtensionEntry>,
    snapshot: Option<OfficialPluginCatalogSnapshot>,
}

#[derive(Clone)]
struct ProjectedRuntimeExtensionEntry {
    catalog_entry: OfficialExtensionCatalogEntry,
    plugin_entry: OfficialPluginSourceEntry,
}

impl ApiOfficialRuntimeExtensionSource {
    pub fn new(
        catalog: Arc<dyn OfficialExtensionCatalogSourcePort>,
        trust_mode: String,
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        Self {
            catalog,
            trust_mode,
            trusted_public_keys,
            projection_cache: Arc::new(Mutex::new(RuntimeExtensionProjectionCache::default())),
        }
    }

    async fn runtime_extension_snapshot(&self) -> Result<OfficialPluginCatalogSnapshot> {
        let mut cursor = None;
        let mut visited_cursors = HashSet::new();
        let mut projected_entries = HashMap::new();
        let mut entries = Vec::new();
        let mut source_kind = None;
        let mut registry_url = None;
        let mut freshness = OfficialPluginCatalogFreshness::Fresh;

        loop {
            let page = self
                .catalog
                .list_page("runtime-extensions", cursor.as_deref())
                .await?;
            match source_kind.as_deref() {
                Some(expected) if expected != page.source_kind => {
                    bail!("runtime extension catalog source changed while paging")
                }
                None => source_kind = Some(page.source_kind.clone()),
                _ => {}
            }
            registry_url.get_or_insert_with(|| page.metadata.locator.clone());
            if page.metadata.freshness == OfficialExtensionCatalogFreshness::Stale {
                freshness = OfficialPluginCatalogFreshness::Stale;
            }
            for catalog_entry in page.entries {
                let plugin_entry = project_runtime_extension_entry(
                    &*self.catalog,
                    &catalog_entry,
                    &self.trust_mode,
                )?;
                if projected_entries
                    .insert(
                        plugin_entry.plugin_id.clone(),
                        ProjectedRuntimeExtensionEntry {
                            catalog_entry,
                            plugin_entry: plugin_entry.clone(),
                        },
                    )
                    .is_some()
                {
                    bail!(
                        "runtime extension catalog contains duplicate plugin_id: {}",
                        plugin_entry.plugin_id
                    );
                }
                entries.push(plugin_entry);
            }
            let Some(next_cursor) = page.metadata.next_cursor else {
                break;
            };
            if !visited_cursors.insert(next_cursor.clone()) {
                bail!("runtime extension catalog contains a cursor cycle");
            }
            cursor = Some(next_cursor);
        }

        let source_kind = source_kind.unwrap_or_else(|| "official_repository".to_string());
        let snapshot = OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_label: if source_kind == "official_repository" {
                    "Official source".to_string()
                } else {
                    "Mirror source".to_string()
                },
                source_kind,
                registry_url: registry_url.unwrap_or_default(),
            },
            freshness,
            entries,
        };
        let mut cache = self
            .projection_cache
            .lock()
            .map_err(|_| anyhow!("runtime extension projection cache is poisoned"))?;
        cache.entries = projected_entries;
        cache.snapshot = Some(snapshot.clone());
        Ok(snapshot)
    }
}

#[async_trait]
impl OfficialPluginSourcePort for ApiOfficialRuntimeExtensionSource {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        self.runtime_extension_snapshot().await
    }

    async fn cached_official_catalog(&self) -> Option<OfficialPluginCatalogSnapshot> {
        self.projection_cache.lock().ok()?.snapshot.clone()
    }

    async fn download_plugin(
        &self,
        entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        let catalog_entry = {
            let cache = self
                .projection_cache
                .lock()
                .map_err(|_| anyhow!("runtime extension projection cache is poisoned"))?;
            let projected = cache.entries.get(&entry.plugin_id).ok_or_else(|| {
                anyhow!("runtime extension entry was not projected by the catalog")
            })?;
            if projected.plugin_entry != *entry {
                bail!("runtime extension entry does not match its catalog projection");
            }
            projected.catalog_entry.clone()
        };
        let downloaded = self.catalog.download_artifact(&catalog_entry).await?;
        Ok(DownloadedOfficialPluginPackage {
            file_name: downloaded.file_name,
            package_bytes: downloaded.artifact_bytes,
        })
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        self.trusted_public_keys.clone()
    }
}

fn project_runtime_extension_entry(
    catalog: &dyn OfficialExtensionCatalogSourcePort,
    entry: &OfficialExtensionCatalogEntry,
    trust_mode: &str,
) -> Result<OfficialPluginSourceEntry> {
    if entry.category != "runtime-extensions" {
        bail!("runtime extension catalog returned an entry from another category");
    }
    let plugin_id = required_runtime_extension_metadata(entry, "plugin_id")?;
    let plugin_type = required_runtime_extension_metadata(entry, "plugin_type")?;
    let provider_code = required_runtime_extension_metadata(entry, "provider_code")?;
    let protocol = required_runtime_extension_metadata(entry, "protocol")?;
    let model_discovery_mode = required_runtime_extension_metadata(entry, "model_discovery_mode")?;
    let descriptor = catalog.resolve_artifact(entry)?;
    let platform = descriptor.platform.unwrap_or_else(|| {
        let host = plugin_framework::RuntimeTarget::current_host().unwrap_or_else(|_| {
            plugin_framework::RuntimeTarget::from_rust_target_triple("x86_64-unknown-linux-musl")
                .expect("fallback runtime extension target must be supported")
        });
        OfficialExtensionArtifactPlatform {
            os: host.os,
            arch: host.arch,
            libc: host.libc,
            rust_target: host.rust_target_triple,
        }
    });
    let checksum = descriptor
        .expected_checksum
        .ok_or_else(|| anyhow!("runtime extension catalog entry is missing checksum"))?;
    let signature_algorithm = descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("algorithm"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let signing_key_id = descriptor
        .signature
        .as_ref()
        .and_then(|signature| signature.get("key_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let i18n_summary = runtime_extension_i18n_summary(entry)?;
    Ok(OfficialPluginSourceEntry {
        plugin_id,
        plugin_type,
        provider_code: provider_code.clone(),
        namespace: optional_runtime_extension_metadata(entry, "namespace")
            .unwrap_or_else(|| format!("plugin.{provider_code}")),
        protocol,
        latest_version: entry.version.clone(),
        minimum_host_version: entry.host_version_requirement.clone(),
        icon: optional_runtime_extension_metadata(entry, "icon"),
        selected_artifact: OfficialPluginArtifact {
            os: platform.os,
            arch: platform.arch,
            libc: platform.libc,
            rust_target: platform.rust_target,
            download_url: descriptor.locator,
            checksum,
            signature_algorithm,
            signing_key_id,
        },
        i18n_summary,
        release_tag: optional_runtime_extension_metadata(entry, "release_tag")
            .unwrap_or_else(|| format!("{provider_code}-v{}", entry.version)),
        trust_mode: trust_mode.to_string(),
        help_url: optional_runtime_extension_metadata(entry, "help_url"),
        model_discovery_mode,
    })
}

fn required_runtime_extension_metadata(
    entry: &OfficialExtensionCatalogEntry,
    field: &'static str,
) -> Result<String> {
    optional_runtime_extension_metadata(entry, field)
        .ok_or_else(|| anyhow!("runtime extension catalog entry is missing {field}"))
}

fn optional_runtime_extension_metadata(
    entry: &OfficialExtensionCatalogEntry,
    field: &str,
) -> Option<String> {
    entry
        .source
        .metadata
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn runtime_extension_i18n_summary(
    entry: &OfficialExtensionCatalogEntry,
) -> Result<OfficialPluginI18nSummary> {
    let value = entry.source.metadata.get("i18n_summary");
    let default_locale = value
        .and_then(|value| value.get("default_locale"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en_US")
        .to_string();
    let mut bundles = value
        .and_then(|value| value.get("bundles"))
        .cloned()
        .map(serde_json::from_value::<BTreeMap<String, Value>>)
        .transpose()
        .context("runtime extension catalog i18n bundles must be an object")?
        .unwrap_or_default();
    if bundles.is_empty() {
        bundles.insert(
            default_locale.clone(),
            serde_json::json!({
                "plugin": { "label": entry.name },
                "provider": { "label": entry.name },
            }),
        );
    }
    let available_locales = value
        .and_then(|value| value.get("available_locales"))
        .cloned()
        .map(serde_json::from_value::<Vec<String>>)
        .transpose()
        .context("runtime extension catalog available_locales must be an array")?
        .filter(|locales| !locales.is_empty())
        .unwrap_or_else(|| bundles.keys().cloned().collect());
    Ok(OfficialPluginI18nSummary {
        default_locale,
        available_locales,
        bundles,
    })
}

#[derive(Clone)]
pub struct ApiOfficialExtensionCatalogSource {
    sources: Arc<BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>>,
    client: Client,
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

impl ApiOfficialExtensionCatalogSource {
    pub fn from_config(config: &ApiConfig) -> Self {
        Self {
            sources: Arc::new(config.official_extension_catalog_sources.clone()),
            client: extension_source_client(),
            last_success: Arc::new(Mutex::new(HashMap::new())),
            search_snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn new(sources: BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>) -> Self {
        Self {
            sources: Arc::new(sources),
            client: extension_source_client(),
            last_success: Arc::new(Mutex::new(HashMap::new())),
            search_snapshots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_request_timeout(
        sources: BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>,
        request_timeout: Duration,
    ) -> Self {
        Self {
            sources: Arc::new(sources),
            client: extension_source_client_with_timeout(request_timeout),
            last_success: Arc::new(Mutex::new(HashMap::new())),
            search_snapshots: Arc::new(Mutex::new(HashMap::new())),
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
        Ok((
            index,
            VerifiedSearchSnapshot {
                source_kind: endpoint.source_kind.clone(),
                endpoint: endpoint.clone(),
                checksum: index.search_index.checksum.clone(),
                locator,
                index: index.clone(),
                document,
            },
        ))
    }

    fn cached_search_snapshot(&self, category: &str) -> Option<VerifiedSearchSnapshot> {
        self.search_snapshots.lock().ok()?.get(category).cloned()
    }

    fn remember_search_snapshot(&self, category: &str, snapshot: &VerifiedSearchSnapshot) {
        if let Ok(mut cache) = self.search_snapshots.lock() {
            cache.insert(category.to_string(), snapshot.clone());
        }
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
        self.download_with_budget(
            url,
            MAX_EXTENSION_CATALOG_DOCUMENT_BYTES,
            "official extension catalog document",
        )
        .await
    }

    async fn download_artifact_bytes(&self, url: &str) -> Result<Vec<u8>> {
        self.download_with_budget(
            url,
            MAX_EXTENSION_ARTIFACT_BYTES,
            "official extension artifact",
        )
        .await
    }

    async fn download_with_budget(
        &self,
        url: &str,
        max_bytes: usize,
        resource: &'static str,
    ) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request {resource} from {url}"))?
            .error_for_status()
            .with_context(|| format!("{resource} returned an error status for {url}"))?;
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.with_context(|| format!("failed to read {resource} response body"))?;
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                bail!("{resource} exceeds download budget");
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            bail!("{resource} is empty");
        }
        Ok(bytes)
    }
}

fn extension_source_client() -> Client {
    extension_source_client_with_timeout(EXTENSION_SOURCE_REQUEST_TIMEOUT)
}

fn extension_source_client_with_timeout(request_timeout: Duration) -> Client {
    Client::builder()
        .connect_timeout(EXTENSION_SOURCE_CONNECT_TIMEOUT)
        .timeout(request_timeout)
        .build()
        .expect("extension source HTTP client configuration must be valid")
}

#[async_trait]
impl OfficialExtensionCatalogSourcePort for ApiOfficialExtensionCatalogSource {
    async fn search(
        &self,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult> {
        if query.limit == 0 || query.limit > 100 {
            bail!("official extension catalog search limit must be between 1 and 100");
        }
        let mut failures = Vec::new();
        for endpoint in self.endpoints(category)? {
            let loaded = if let Some(snapshot) = self.cached_search_snapshot(category) {
                Ok((snapshot.index.clone(), snapshot))
            } else {
                self.load_search_snapshot(category, &endpoint).await
            };
            let (index, snapshot) = match loaded {
                Ok(value) => value,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            self.remember_search_snapshot(category, &snapshot);
            let normalized_slot = normalized_filter(query.slot_code.as_deref());
            let normalized_q = normalized_filter(query.q.as_deref());
            let query_binding = search_query_binding(
                category,
                normalized_slot.as_deref(),
                normalized_q.as_deref(),
                query.limit,
            );
            let offset = decode_search_cursor(
                query.cursor.as_deref(),
                &snapshot.checksum,
                &snapshot.document.source_fingerprint,
                &query_binding,
            )?;
            let matches = snapshot
                .document
                .entries
                .iter()
                .filter(|entry| {
                    normalized_slot.as_ref().is_none_or(|slot| {
                        entry.slot_codes.iter().any(|candidate| candidate == slot)
                    }) && normalized_q
                        .as_ref()
                        .is_none_or(|needle| entry.matches(needle))
                })
                .collect::<Vec<_>>();
            if offset > matches.len() {
                bail!("official extension catalog search cursor is out of range");
            }
            let selected = matches
                .iter()
                .skip(offset)
                .take(query.limit)
                .copied()
                .collect::<Vec<_>>();
            let selected_ids = selected
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<HashSet<_>>();
            let mut entries_by_id = HashMap::new();
            let mut visited_pages = HashSet::new();
            for search_entry in &selected {
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
                        .fetch_page(&snapshot.endpoint, &index, page_reference)
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
            let entries = selected
                .into_iter()
                .map(|search_entry| {
                    entries_by_id.remove(&search_entry.id).ok_or_else(|| {
                        anyhow!("catalog search result is missing from its verified source page")
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let next_offset = offset + entries.len();
            let next_cursor = (next_offset < matches.len()).then(|| {
                encode_search_cursor(
                    &snapshot.checksum,
                    &snapshot.document.source_fingerprint,
                    &query_binding,
                    next_offset,
                )
            });
            return Ok(OfficialExtensionCatalogSearchResult {
                source_kind: snapshot.source_kind,
                category: category.to_string(),
                snapshot_checksum: snapshot.checksum,
                snapshot_locator: snapshot.locator,
                total_entries: matches.len(),
                next_cursor,
                entries,
            });
        }
        Err(failures
            .into_iter()
            .next()
            .unwrap_or_else(|| anyhow!("official extension catalog has no configured source")))
    }

    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        let key = Self::cache_key(category, cursor);
        let mut failures = Vec::new();
        for endpoint in self.endpoints(category)? {
            match self.load_page(category, cursor, &endpoint).await {
                Ok(page) => {
                    self.remember_page(key.clone(), &page);
                    return Ok(page);
                }
                Err(error) => failures.push(error),
            }
        }
        if let Some(page) = self.stale_page(&key) {
            return Ok(page);
        }
        Err(failures
            .into_iter()
            .next()
            .unwrap_or_else(|| anyhow!("official extension catalog has no configured source")))
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        if let Ok(cache) = self.last_success.lock() {
            if let Some(page) = cache.values().find(|page| {
                page.category == category && page.entries.iter().any(|entry| entry.id == catalog_id)
            }) {
                if let Some(entry) = page
                    .entries
                    .iter()
                    .find(|entry| entry.id == catalog_id)
                    .cloned()
                {
                    return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                        source_kind: page.source_kind.clone(),
                        entry,
                    }));
                }
            }
        }
        let mut failures = Vec::new();
        for endpoint in self.endpoints(category)? {
            let index = match self.fetch_index(category, &endpoint).await {
                Ok(index) => index,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            let mut failed = None;
            for page_reference in &index.pages {
                match self.fetch_page(&endpoint, &index, page_reference).await {
                    Ok(page) => {
                        self.remember_page(
                            Self::cache_key(category, Some(&page.metadata.cursor)),
                            &page,
                        );
                        if let Some(entry) = page
                            .entries
                            .into_iter()
                            .find(|entry| entry.id == catalog_id)
                        {
                            return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                                source_kind: endpoint.source_kind.clone(),
                                entry,
                            }));
                        }
                    }
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
            }
            if let Some(error) = failed {
                failures.push(error);
                continue;
            }
            return Ok(None);
        }
        if !failures.is_empty() {
            if let Ok(cache) = self.last_success.lock() {
                if let Some(page) = cache.values().find(|page| {
                    page.category == category
                        && page.entries.iter().any(|entry| entry.id == catalog_id)
                }) {
                    if let Some(entry) = page
                        .entries
                        .iter()
                        .find(|entry| entry.id == catalog_id)
                        .cloned()
                    {
                        return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                            source_kind: page.source_kind.clone(),
                            entry,
                        }));
                    }
                }
            }
            return Err(failures.remove(0));
        }
        Ok(None)
    }

    fn resolve_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<OfficialExtensionArtifactDescriptor> {
        let source = self.source(&entry.category)?;
        resolve_artifact_descriptor(entry, source.github_proxy_url.as_deref())
    }

    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact> {
        let descriptor = self.resolve_artifact(entry)?;
        let artifact_bytes = self.download_artifact_bytes(&descriptor.locator).await?;
        let file_name = descriptor
            .locator
            .split('?')
            .next()
            .and_then(|url| url.rsplit('/').next())
            .filter(|value| !value.is_empty())
            .unwrap_or("extension-artifact.bin")
            .to_string();
        Ok(DownloadedOfficialExtensionArtifact {
            descriptor,
            file_name,
            artifact_bytes,
        })
    }
}

#[derive(Debug, Deserialize)]
struct DownloadLocatorDocument {
    kind: String,
    locator: Option<String>,
    #[serde(default)]
    artifacts: Vec<PlatformArtifactDocument>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlatformArtifactDocument {
    os: String,
    arch: String,
    libc: Option<String>,
    rust_target: Option<String>,
    locator: String,
    checksum: Option<String>,
    signature: Option<Value>,
}

fn resolve_artifact_descriptor(
    entry: &OfficialExtensionCatalogEntry,
    github_proxy_url: Option<&str>,
) -> Result<OfficialExtensionArtifactDescriptor> {
    let locator: DownloadLocatorDocument =
        serde_json::from_value(entry.download_locator.clone())
            .context("failed to decode official extension download locator")?;
    match locator.kind.as_str() {
        "repository_file" | "release_asset" | "https" => {
            let url = locator.locator.ok_or_else(|| {
                anyhow!("official extension download locator has no artifact URL")
            })?;
            Ok(OfficialExtensionArtifactDescriptor {
                locator_kind: locator.kind,
                locator: rewrite_github_download_url(&url, github_proxy_url),
                expected_checksum: entry.checksum.clone(),
                signature: entry.signature.clone(),
                platform: None,
            })
        }
        "platform_release_assets" => {
            let selected = select_current_platform_artifact(&locator.artifacts)
                .ok_or_else(|| anyhow!("official extension has no artifact for this platform"))?;
            Ok(OfficialExtensionArtifactDescriptor {
                locator_kind: locator.kind,
                locator: rewrite_github_download_url(&selected.locator, github_proxy_url),
                expected_checksum: selected.checksum.clone().or_else(|| entry.checksum.clone()),
                signature: selected
                    .signature
                    .clone()
                    .or_else(|| entry.signature.clone()),
                platform: Some(OfficialExtensionArtifactPlatform {
                    os: selected.os.clone(),
                    arch: selected.arch.clone(),
                    libc: selected.libc.clone(),
                    rust_target: selected
                        .rust_target
                        .clone()
                        .unwrap_or_else(|| rust_target_for_platform(selected)),
                }),
            })
        }
        _ => bail!("unsupported official extension download locator kind"),
    }
}

fn rust_target_for_platform(artifact: &PlatformArtifactDocument) -> String {
    let architecture = match artifact.arch.as_str() {
        "amd64" => "x86_64",
        "arm64" => "aarch64",
        value => value,
    };
    match (artifact.os.as_str(), artifact.libc.as_deref()) {
        ("linux", Some("musl")) => format!("{architecture}-unknown-linux-musl"),
        ("linux", _) => format!("{architecture}-unknown-linux-gnu"),
        ("windows", _) => format!("{architecture}-pc-windows-msvc"),
        ("macos", _) => format!("{architecture}-apple-darwin"),
        (os, _) => format!("{architecture}-unknown-{os}"),
    }
}

fn select_current_platform_artifact(
    artifacts: &[PlatformArtifactDocument],
) -> Option<&PlatformArtifactDocument> {
    let host_os = std::env::consts::OS;
    let host_arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        value => value,
    };
    let host_libc = if host_os == "linux" {
        if cfg!(target_env = "musl") {
            Some("musl")
        } else {
            Some("gnu")
        }
    } else if host_os == "windows" {
        Some("msvc")
    } else {
        None
    };
    artifacts
        .iter()
        .filter(|artifact| artifact.os == host_os && artifact.arch == host_arch)
        .max_by_key(|artifact| match (host_libc, artifact.libc.as_deref()) {
            (Some(left), Some(right)) if left == right => 3_u8,
            (Some("gnu"), Some("musl")) if host_os == "linux" => 2,
            (_, None) => 1,
            _ => 0,
        })
}

fn rewrite_github_download_url(url: &str, github_proxy_url: Option<&str>) -> String {
    let raw_rewritten = rewrite_github_raw_url(url, github_proxy_url);
    if raw_rewritten != url {
        return raw_rewritten;
    }
    let Some(proxy) = github_proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return url.to_string();
    };
    let proxy = proxy.trim_end_matches('/');
    let proxied_prefix = format!("{proxy}/https://github.com/");
    if url.starts_with(&proxied_prefix) || !url.starts_with("https://github.com/") {
        return url.to_string();
    }
    format!("{proxy}/{url}")
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

fn validate_index(index: &CatalogIndexDocument, category: &str) -> Result<()> {
    if index.schema_version != EXTENSION_CATALOG_SCHEMA_VERSION || index.category != category {
        bail!("official extension catalog index contract mismatch");
    }
    if index.page_size == 0 || index.pages.is_empty() {
        bail!("official extension catalog index has no usable pages");
    }
    if index.search_index.schema_version != EXTENSION_CATALOG_SEARCH_SCHEMA_VERSION
        || index.search_index.entry_count != index.total_entries
        || index.search_index.checksum.trim().is_empty()
        || index.search_index.locator.trim().is_empty()
    {
        bail!("official extension catalog search reference contract mismatch");
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

fn validate_search_document(
    document: &CatalogSearchDocument,
    index: &CatalogIndexDocument,
) -> Result<()> {
    if document.schema_version != EXTENSION_CATALOG_SEARCH_SCHEMA_VERSION
        || document.category != index.category
        || document.entries.len() != index.search_index.entry_count
        || document.source_fingerprint.trim().is_empty()
        || document.entries.iter().any(|entry| {
            entry.category != index.category
                || entry.slot_codes.iter().any(|value| value.trim().is_empty())
                || entry.keywords.iter().any(|value| value.trim().is_empty())
                || domain::ExtensionCategory::parse(&index.category)
                    .and_then(|category| {
                        domain::ExtensionCatalogIdentity::parse(category, &entry.id)
                    })
                    .is_none_or(|identity| {
                        identity.organization() != entry.organization
                            || identity.artifact_id() != entry.artifact
                    })
                || !index.pages.iter().any(|page| {
                    page.page == entry.catalog_page.page
                        && page.cursor == entry.catalog_page.cursor
                        && page.checksum == entry.catalog_page.checksum
                        && page.locator == entry.catalog_page.locator
                })
        })
    {
        bail!("official extension catalog search document contract mismatch");
    }
    Ok(())
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
}

fn search_query_binding(
    category: &str,
    slot_code: Option<&str>,
    q: Option<&str>,
    limit: usize,
) -> String {
    digest_text(&format!(
        "{category}\u{0}{}\u{0}{}\u{0}{limit}",
        slot_code.unwrap_or_default(),
        q.unwrap_or_default()
    ))
}

fn digest_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn encode_search_cursor(
    checksum: &str,
    source_fingerprint: &str,
    query_binding: &str,
    offset: usize,
) -> String {
    format!(
        "v1.{}.{}.{}.{offset}",
        digest_text(checksum),
        digest_text(source_fingerprint),
        query_binding
    )
}

fn decode_search_cursor(
    cursor: Option<&str>,
    checksum: &str,
    source_fingerprint: &str,
    query_binding: &str,
) -> Result<usize> {
    let Some(cursor) = cursor.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(0);
    };
    let mut parts = cursor.split('.');
    let valid = parts.next() == Some("v1")
        && parts.next() == Some(digest_text(checksum).as_str())
        && parts.next() == Some(digest_text(source_fingerprint).as_str())
        && parts.next() == Some(query_binding)
        && parts.clone().count() == 1;
    if !valid {
        bail!("official extension catalog search cursor does not match this snapshot and query");
    }
    parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| anyhow!("official extension catalog search cursor is invalid"))
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
        || page.entries.iter().any(|entry| {
            entry.category != category
                || entry.catalog_page != page_reference.page
                || entry.slot_codes.iter().any(|value| value.trim().is_empty())
                || entry.keywords.iter().any(|value| value.trim().is_empty())
                || domain::ExtensionCategory::parse(category)
                    .and_then(|category| {
                        domain::ExtensionCatalogIdentity::parse(category, &entry.id)
                    })
                    .map_or(true, |identity| {
                        identity.organization() != entry.organization
                            || identity.artifact_id() != entry.artifact
                    })
        })
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
