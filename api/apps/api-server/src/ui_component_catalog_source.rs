use std::{collections::BTreeSet, time::Duration};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{
    OfficialUiComponentCatalogRecord, UiComponentCatalogIndex, UiComponentCatalogPage,
    UiComponentCatalogSearchEntry, UiComponentCatalogSearchResult, UiComponentCatalogSeed,
    UiComponentCatalogSource,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

pub const DEFAULT_UI_COMPONENT_CATALOG_INDEX_LOCATOR: &str = "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/ui_components/catalog/v1/index.json";

const INDEX_SCHEMA: &str = "1flowbase.ui-component-catalog-index/v1";
const PAGE_SCHEMA: &str = "1flowbase.ui-component-catalog-page/v1";
const SEARCH_SCHEMA: &str = "1flowbase.ui-component-catalog-search/v1";
const SEED_SCHEMA: &str = "1flowbase.ui-component-catalog-seed/v1";
const SOURCE_SCHEMA: &str = "1flowbase.ui-component-source/v1";
const MAX_CATALOG_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct ApiUiComponentCatalogSource {
    index_locator: String,
    client: Client,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexDocument {
    schema_version: String,
    catalog_version: String,
    generated_at: String,
    page_size: usize,
    total_components: usize,
    source_fingerprint: String,
    first_page: PageReference,
    search_index: SearchReference,
    download: DownloadReference,
    update: UpdateContract,
    pages: Vec<PageReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageReference {
    page: u32,
    cursor: String,
    component_count: Option<usize>,
    checksum: Option<String>,
    locator: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchReference {
    schema_version: String,
    entry_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadReference {
    schema_version: String,
    checksum: String,
    locator: String,
    release_tag: String,
    release_locator: String,
    release_catalog_locator: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateContract {
    strategy: String,
    identity_field: String,
    source_field: String,
    group_field: String,
    version_field: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageDocument {
    schema_version: String,
    catalog_version: String,
    page: u32,
    cursor: String,
    next_cursor: Option<String>,
    next_page_locator: Option<String>,
    components: Vec<PublishedRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublishedRecord {
    schema_version: String,
    component_code: String,
    name: String,
    description: String,
    import_code: String,
    source_code: String,
    origin: String,
    source: String,
    group: String,
    upstream: PublishedUpstream,
    version: String,
    keywords: Vec<String>,
    updated_at: String,
    source_locator: String,
    source_checksum: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PublishedUpstream {
    identity: String,
    version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchDocument {
    schema_version: String,
    catalog_version: String,
    generated_at: String,
    source_fingerprint: String,
    entries: Vec<SearchEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchEntry {
    component_code: String,
    name: String,
    description: String,
    origin: String,
    source: String,
    group: String,
    upstream: PublishedUpstream,
    version: String,
    keywords: Vec<String>,
    catalog_page: SearchPageReference,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchPageReference {
    page: u32,
    cursor: String,
    checksum: String,
    locator: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedDocument {
    manifest: SeedManifest,
    components: Vec<PublishedRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SeedManifest {
    schema_version: String,
    catalog_version: String,
    generated_at: String,
    page_size: usize,
    total_components: usize,
    components_sha256: String,
    semantic_sha256: String,
}

impl ApiUiComponentCatalogSource {
    pub fn new(index_locator: impl Into<String>) -> Self {
        Self {
            index_locator: index_locator.into(),
            client: Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .build()
                .expect("UI component catalog HTTP client configuration must be valid"),
        }
    }

    pub fn default_taichuy() -> Self {
        Self::new(DEFAULT_UI_COMPONENT_CATALOG_INDEX_LOCATOR)
    }

    async fn fetch_bytes(&self, locator: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(locator)
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .with_context(|| format!("failed to request UI component catalog {locator}"))?
            .error_for_status()
            .with_context(|| format!("UI component catalog returned an error for {locator}"))?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CATALOG_BYTES as u64)
        {
            bail!("UI component catalog document exceeds the download limit");
        }
        let bytes = response
            .bytes()
            .await
            .context("failed to read UI component catalog")?;
        if bytes.len() > MAX_CATALOG_BYTES {
            bail!("UI component catalog document exceeds the download limit");
        }
        Ok(bytes.to_vec())
    }

    async fn load_index(&self) -> Result<IndexDocument> {
        let bytes = self.fetch_bytes(&self.index_locator).await?;
        let document: IndexDocument =
            serde_json::from_slice(&bytes).context("invalid UI component catalog index JSON")?;
        validate_index(&document)?;
        Ok(document)
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_semver(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn valid_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(crate) fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>> {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort();
                Value::Object(Map::from_iter(
                    keys.into_iter()
                        .map(|key| (key.clone(), sorted(&object[key]))),
                ))
            }
            Value::Array(values) => Value::Array(values.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    let mut bytes = serde_json::to_vec_pretty(&sorted(value))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_index(index: &IndexDocument) -> Result<()> {
    if index.schema_version != INDEX_SCHEMA
        || !valid_semver(&index.catalog_version)
        || OffsetDateTime::parse(
            &index.generated_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
        || index.page_size == 0
        || !valid_digest(&index.source_fingerprint)
        || index.search_index.schema_version != SEARCH_SCHEMA
        || index.search_index.entry_count != index.total_components
        || !valid_digest(&index.search_index.checksum)
        || index.download.schema_version != SEED_SCHEMA
        || !valid_digest(&index.download.checksum)
        || index.download.locator.is_empty()
        || index.download.release_tag.is_empty()
        || index.download.release_locator.is_empty()
        || index.download.release_catalog_locator.is_empty()
        || index.update.strategy != "authoritative_source_group_replace"
        || index.update.identity_field != "component_code"
        || index.update.source_field != "source"
        || index.update.group_field != "group"
        || index.update.version_field != "version"
        || index.first_page.page != 1
        || index.first_page.cursor != "start"
        || index.pages.is_empty()
        || index.first_page.locator != index.pages[0].locator
    {
        bail!("invalid UI component catalog index schema or contract");
    }
    let mut total = 0usize;
    for (offset, page) in index.pages.iter().enumerate() {
        if page.page as usize != offset + 1
            || page.component_count.is_none()
            || !page.checksum.as_deref().is_some_and(valid_digest)
            || page.locator.is_empty()
        {
            bail!("invalid UI component catalog page reference");
        }
        total += page.component_count.unwrap_or_default();
    }
    if total != index.total_components {
        bail!("UI component catalog index total does not match page inventory");
    }
    Ok(())
}

fn source_value(record: &PublishedRecord) -> Result<Value> {
    Ok(serde_json::json!({
        "schema_version": record.schema_version,
        "component_code": record.component_code,
        "name": record.name,
        "description": record.description,
        "import_code": record.import_code,
        "source_code": record.source_code,
        "origin": record.origin,
        "source": record.source,
        "group": record.group,
        "upstream": record.upstream,
        "version": record.version,
        "keywords": record.keywords,
        "updated_at": record.updated_at,
    }))
}

fn validate_record(record: &PublishedRecord) -> Result<OfficialUiComponentCatalogRecord> {
    let updated_at = OffsetDateTime::parse(
        &record.updated_at,
        &time::format_description::well_known::Rfc3339,
    )
    .context("invalid UI component catalog record timestamp")?;
    let mut normalized_keywords = record.keywords.clone();
    normalized_keywords.sort();
    let unique = normalized_keywords.iter().collect::<BTreeSet<_>>();
    let expected_prefix = format!("ui_components/@{}/{}/", record.source, record.group);
    if record.schema_version != SOURCE_SCHEMA
        || record.origin != "official"
        || !valid_code(&record.component_code)
        || !valid_code(&record.source)
        || !valid_code(&record.group)
        || !valid_semver(&record.version)
        || record.name.is_empty()
        || record.description.is_empty()
        || record.import_code.is_empty()
        || record.source_code.is_empty()
        || record.upstream.identity.is_empty()
        || record.upstream.version.is_empty()
        || record
            .keywords
            .iter()
            .any(|keyword| keyword.trim().is_empty() || keyword.trim() != keyword)
        || unique.len() != record.keywords.len()
        || normalized_keywords != record.keywords
        || !record.source_locator.starts_with(&expected_prefix)
        || !record.source_locator.ends_with(".json")
        || !valid_digest(&record.source_checksum)
    {
        bail!("invalid UI component catalog record shape or authoritative identity");
    }
    if digest(&canonical_json_bytes(&source_value(record)?)?) != record.source_checksum {
        bail!("UI component catalog record source checksum mismatch");
    }
    Ok(OfficialUiComponentCatalogRecord {
        component_code: record.component_code.clone(),
        name: record.name.clone(),
        description: record.description.clone(),
        import_code: record.import_code.clone(),
        source_code: record.source_code.clone(),
        source: record.source.clone(),
        group: record.group.clone(),
        upstream: domain::UiComponentRecordUpstream {
            identity: record.upstream.identity.clone(),
            version: record.upstream.version.clone(),
        },
        version: record.version.clone(),
        keywords: record.keywords.clone(),
        catalog_updated_at: updated_at,
        source_locator: record.source_locator.clone(),
        source_checksum: record.source_checksum.clone(),
    })
}

#[async_trait]
impl UiComponentCatalogSource for ApiUiComponentCatalogSource {
    async fn index(&self) -> Result<UiComponentCatalogIndex> {
        let index = self.load_index().await?;
        Ok(UiComponentCatalogIndex {
            catalog_version: index.catalog_version,
            generated_at: OffsetDateTime::parse(
                &index.generated_at,
                &time::format_description::well_known::Rfc3339,
            )?,
            page_size: index.page_size,
            total_components: index.total_components,
            source_fingerprint: index.source_fingerprint,
        })
    }

    async fn page(&self, page: u32) -> Result<UiComponentCatalogPage> {
        let index = self.load_index().await?;
        let reference = index
            .pages
            .iter()
            .find(|reference| reference.page == page)
            .ok_or_else(|| anyhow::anyhow!("UI component catalog page not found"))?;
        let bytes = self.fetch_bytes(&reference.locator).await?;
        if digest(&bytes) != reference.checksum.as_deref().unwrap_or_default() {
            bail!("UI component catalog page checksum mismatch");
        }
        let document: PageDocument =
            serde_json::from_slice(&bytes).context("invalid UI component catalog page JSON")?;
        if document.schema_version != PAGE_SCHEMA
            || document.catalog_version != index.catalog_version
            || document.page != reference.page
            || document.cursor != reference.cursor
            || document.components.len() != reference.component_count.unwrap_or_default()
        {
            bail!("UI component catalog page does not match index metadata");
        }
        let next_reference = index.pages.iter().find(|value| value.page == page + 1);
        match (
            &document.next_cursor,
            &document.next_page_locator,
            next_reference,
        ) {
            (Some(cursor), Some(locator), Some(next))
                if cursor == &next.cursor && locator == &next.locator => {}
            (None, None, None) => {}
            _ => bail!("invalid UI component catalog page continuation"),
        }
        let mut previous = None::<String>;
        let mut records = Vec::with_capacity(document.components.len());
        for component in &document.components {
            if previous
                .as_deref()
                .is_some_and(|value| value >= component.component_code.as_str())
            {
                bail!("UI component catalog page component order is invalid");
            }
            previous = Some(component.component_code.clone());
            records.push(validate_record(component)?);
        }
        Ok(UiComponentCatalogPage {
            catalog_version: document.catalog_version,
            total_components: index.total_components,
            page_size: index.page_size,
            page: document.page,
            cursor: document.cursor,
            next_cursor: document.next_cursor,
            records,
        })
    }

    async fn search(
        &self,
        query: &str,
        page: u32,
        page_size: usize,
    ) -> Result<UiComponentCatalogSearchResult> {
        let index = self.load_index().await?;
        let bytes = self.fetch_bytes(&index.search_index.locator).await?;
        if digest(&bytes) != index.search_index.checksum {
            bail!("UI component catalog search checksum mismatch");
        }
        let search: SearchDocument = serde_json::from_slice(&bytes)
            .context("invalid UI component catalog search index JSON")?;
        if search.schema_version != SEARCH_SCHEMA
            || search.catalog_version != index.catalog_version
            || search.source_fingerprint != index.source_fingerprint
            || search.entries.len() != index.search_index.entry_count
            || OffsetDateTime::parse(
                &search.generated_at,
                &time::format_description::well_known::Rfc3339,
            )
            .is_err()
        {
            bail!("UI component catalog search index does not match catalog index");
        }
        for entry in &search.entries {
            let reference = index
                .pages
                .iter()
                .find(|page| page.page == entry.catalog_page.page);
            if entry.origin != "official"
                || !valid_code(&entry.component_code)
                || !valid_code(&entry.source)
                || !valid_code(&entry.group)
                || entry.name.is_empty()
                || entry.description.is_empty()
                || entry.upstream.identity.is_empty()
                || entry.upstream.version.is_empty()
                || reference.is_none()
                || reference.and_then(|value| value.checksum.as_deref())
                    != Some(entry.catalog_page.checksum.as_str())
                || reference.map(|value| value.cursor.as_str())
                    != Some(entry.catalog_page.cursor.as_str())
                || reference.map(|value| value.locator.as_str())
                    != Some(entry.catalog_page.locator.as_str())
            {
                bail!("invalid UI component catalog search entry");
            }
        }
        let normalized = query.trim().to_lowercase();
        let filtered = search
            .entries
            .into_iter()
            .filter(|entry| {
                normalized.is_empty()
                    || entry.name.contains(&normalized)
                    || entry.description.contains(&normalized)
                    || entry.component_code.contains(&normalized)
                    || entry
                        .keywords
                        .iter()
                        .any(|keyword| keyword.contains(&normalized))
            })
            .collect::<Vec<_>>();
        let total_entries = filtered.len();
        let offset = (page.saturating_sub(1) as usize).saturating_mul(page_size);
        let entries = filtered
            .into_iter()
            .skip(offset)
            .take(page_size)
            .map(|entry| UiComponentCatalogSearchEntry {
                component_code: entry.component_code,
                name: entry.name,
                description: entry.description,
                source: entry.source,
                group: entry.group,
                upstream: domain::UiComponentRecordUpstream {
                    identity: entry.upstream.identity,
                    version: entry.upstream.version,
                },
                version: entry.version,
                keywords: entry.keywords,
                catalog_page: entry.catalog_page.page,
            })
            .collect();
        Ok(UiComponentCatalogSearchResult {
            catalog_version: index.catalog_version,
            page,
            page_size,
            total_entries,
            entries,
        })
    }

    async fn seed(&self) -> Result<UiComponentCatalogSeed> {
        let index = self.load_index().await?;
        let bytes = self.fetch_bytes(&index.download.locator).await?;
        if digest(&bytes) != index.download.checksum {
            bail!("UI component catalog seed checksum mismatch");
        }
        let seed: SeedDocument =
            serde_json::from_slice(&bytes).context("invalid UI component catalog seed JSON")?;
        if seed.manifest.schema_version != SEED_SCHEMA
            || seed.manifest.catalog_version != index.catalog_version
            || seed.manifest.page_size != index.page_size
            || seed.manifest.total_components != seed.components.len()
            || seed.manifest.total_components != index.total_components
            || !valid_digest(&seed.manifest.components_sha256)
            || !valid_digest(&seed.manifest.semantic_sha256)
            || OffsetDateTime::parse(
                &seed.manifest.generated_at,
                &time::format_description::well_known::Rfc3339,
            )
            .is_err()
        {
            bail!("invalid UI component catalog seed manifest");
        }
        let components_value = serde_json::to_value(&seed.components)?;
        let components_digest = digest(&canonical_json_bytes(&components_value)?);
        if components_digest != seed.manifest.components_sha256
            || components_digest != index.source_fingerprint
        {
            bail!("UI component catalog components digest mismatch");
        }
        let semantic = serde_json::json!({
            "catalog_version": seed.manifest.catalog_version,
            "generated_at": seed.manifest.generated_at,
            "page_size": seed.manifest.page_size,
            "total_components": seed.manifest.total_components,
            "components_sha256": seed.manifest.components_sha256,
            "components": seed.components,
        });
        if digest(&canonical_json_bytes(&semantic)?) != seed.manifest.semantic_sha256 {
            bail!("UI component catalog semantic digest mismatch");
        }
        let semantic_components = semantic["components"]
            .as_array()
            .context("catalog semantic components are absent")?;
        let published = semantic_components
            .iter()
            .map(|value| serde_json::from_value::<PublishedRecord>(value.clone()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut previous = None::<String>;
        let mut records = Vec::with_capacity(published.len());
        for component in &published {
            if previous
                .as_deref()
                .is_some_and(|value| value >= component.component_code.as_str())
            {
                bail!("UI component catalog seed component order is invalid");
            }
            previous = Some(component.component_code.clone());
            records.push(validate_record(component)?);
        }
        Ok(UiComponentCatalogSeed {
            catalog_version: seed.manifest.catalog_version,
            source_fingerprint: components_digest,
            records,
        })
    }
}
