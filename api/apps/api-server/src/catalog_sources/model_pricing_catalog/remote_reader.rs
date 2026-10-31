use super::{validate_catalog_rules, ApiError, PricingRuleBody};
use control_plane::errors::ControlPlaneError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Index {
    schema_version: String,
    catalog_version: String,
    currency_code: String,
    total_rules: usize,
    pages: Vec<PageReference>,
    search_index: Option<SearchReference>,
}

#[derive(Debug, Deserialize)]
struct PageReference {
    page: usize,
    rule_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Deserialize)]
struct SearchReference {
    schema_version: String,
    entry_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Deserialize)]
struct Page {
    schema_version: String,
    catalog_version: String,
    currency_code: String,
    page: usize,
    rules: Vec<PricingRuleBody>,
}

#[derive(Debug, Deserialize)]
struct SearchIndex {
    schema_version: String,
    catalog_version: String,
    entries: Vec<SearchEntry>,
}

#[derive(Debug, Deserialize)]
struct SearchEntry {
    id: Uuid,
    provider_code: String,
    upstream_model_id: String,
    catalog_page: SearchPageReference,
}

#[derive(Debug, Deserialize)]
struct SearchPageReference {
    page: usize,
    checksum: String,
    locator: String,
}

// Own only the small index and a shared HTTP client, never the complete rule catalog.
pub(crate) struct RemotePricingCatalogReader {
    client: reqwest::Client,
    index_url: String,
    index: Index,
}

fn invalid(field: &'static str) -> ApiError {
    ControlPlaneError::InvalidInput(field).into()
}

fn verify_checksum(bytes: &[u8], expected: &str, error: &'static str) -> Result<(), ApiError> {
    let actual = format!("sha256:{:x}", base16ct::HexDisplay(&Sha256::digest(bytes)));
    if actual != expected {
        return Err(ControlPlaneError::Conflict(error).into());
    }
    Ok(())
}

impl RemotePricingCatalogReader {
    pub(crate) async fn open(client: &reqwest::Client, index_url: &str) -> Result<Self, ApiError> {
        let bytes = fetch_document(client, index_url, 512 * 1024).await?;
        let index: Index = serde_json::from_slice(&bytes)?;
        if index.schema_version != "1flowbase.model-pricing-index/v2"
            || index.currency_code != "USD"
            || index.pages.len() > 1_000
            || index.total_rules > 100_000
        {
            return Err(invalid("pricing_catalog_index"));
        }
        let prefix = index_url
            .strip_suffix("index.json")
            .ok_or_else(|| invalid("pricing_catalog_index_url"))?;
        let mut total = 0usize;
        for (position, reference) in index.pages.iter().enumerate() {
            let number = position + 1;
            if reference.page != number
                || reference.locator != format!("{prefix}pages/{number}.json")
            {
                return Err(invalid("pricing_catalog_page_locator"));
            }
            total = total
                .checked_add(reference.rule_count)
                .ok_or_else(|| invalid("pricing_catalog_rule_count"))?;
        }
        if total != index.total_rules {
            return Err(invalid("pricing_catalog_rule_count"));
        }
        Ok(Self {
            client: client.clone(),
            index_url: index_url.to_owned(),
            index,
        })
    }

    pub(crate) fn catalog_version(&self) -> &str {
        &self.index.catalog_version
    }
    pub(crate) fn page_count(&self) -> usize {
        self.index.pages.len()
    }

    pub(crate) async fn read_page(&self, number: usize) -> Result<Vec<PricingRuleBody>, ApiError> {
        let reference = number
            .checked_sub(1)
            .and_then(|i| self.index.pages.get(i))
            .ok_or_else(|| invalid("pricing_catalog_page"))?;
        let bytes = fetch_document(&self.client, &reference.locator, 2 * 1024 * 1024).await?;
        verify_checksum(
            &bytes,
            &reference.checksum,
            "pricing_catalog_page_checksum_mismatch",
        )?;
        let page: Page = serde_json::from_slice(&bytes)?;
        if page.schema_version != "1flowbase.model-pricing-page/v2"
            || page.catalog_version != self.index.catalog_version
            || page.currency_code != "USD"
            || page.page != number
            || page.rules.len() != reference.rule_count
        {
            return Err(invalid("pricing_catalog_page"));
        }
        validate_catalog_rules(&page.rules)?;
        Ok(page.rules)
    }

    async fn search_index(&self) -> Result<SearchIndex, ApiError> {
        let reference = self
            .index
            .search_index
            .as_ref()
            .ok_or_else(|| invalid("pricing_catalog_search_index"))?;
        let prefix = self
            .index_url
            .strip_suffix("index.json")
            .ok_or_else(|| invalid("pricing_catalog_index_url"))?;
        if reference.schema_version != "1flowbase.model-pricing-search/v2"
            || reference.entry_count != self.index.total_rules
            || reference.locator != format!("{prefix}search-index.json")
        {
            return Err(invalid("pricing_catalog_search_index"));
        }
        // Metadata only: the publisher's maximum index cardinality bounds this document.
        let bytes = fetch_document(&self.client, &reference.locator, 64 * 1024 * 1024).await?;
        verify_checksum(
            &bytes,
            &reference.checksum,
            "pricing_catalog_search_checksum_mismatch",
        )?;
        let search: SearchIndex = serde_json::from_slice(&bytes)?;
        if search.schema_version != reference.schema_version
            || search.catalog_version != self.index.catalog_version
            || search.entries.len() != reference.entry_count
        {
            return Err(invalid("pricing_catalog_search_index"));
        }
        let mut ids = HashSet::new();
        let mut page_counts = vec![0usize; self.page_count()];
        let mut previous_page = 0;
        for entry in &search.entries {
            let page = entry
                .catalog_page
                .page
                .checked_sub(1)
                .and_then(|i| self.index.pages.get(i))
                .ok_or_else(|| invalid("pricing_catalog_search_page"))?;
            if entry.catalog_page.page < previous_page
                || entry.catalog_page.checksum != page.checksum
                || entry.catalog_page.locator != page.locator
                || !ids.insert(entry.id)
            {
                return Err(invalid("pricing_catalog_search_page"));
            }
            previous_page = page.page;
            page_counts[page.page - 1] += 1;
        }
        if self
            .index
            .pages
            .iter()
            .any(|page| page_counts[page.page - 1] != page.rule_count)
        {
            return Err(invalid("pricing_catalog_search_count"));
        }
        Ok(search)
    }

    async fn selected(&self, selected: &[&SearchEntry]) -> Result<Vec<PricingRuleBody>, ApiError> {
        let mut rules = HashMap::new();
        let pages: std::collections::BTreeSet<_> = selected
            .iter()
            .map(|entry| entry.catalog_page.page)
            .collect();
        let selected_pages: HashMap<_, _> = selected
            .iter()
            .map(|entry| (entry.id, entry.catalog_page.page))
            .collect();
        for number in pages {
            for rule in self.read_page(number).await? {
                let id = rule.id.ok_or_else(|| invalid("pricing_catalog_id"))?;
                if let Some(expected_page) = selected_pages.get(&id) {
                    if *expected_page != number {
                        return Err(invalid("pricing_catalog_search_page"));
                    }
                    if rules.insert(id, rule).is_some() {
                        return Err(invalid("pricing_catalog_duplicate_standard"));
                    }
                }
            }
        }
        let mut result = Vec::with_capacity(selected.len());
        for entry in selected {
            let rule = rules
                .remove(&entry.id)
                .ok_or_else(|| invalid("pricing_catalog_search_result_missing"))?;
            if rule.provider_code.to_lowercase() != entry.provider_code.to_lowercase()
                || rule.upstream_model_id.to_lowercase() != entry.upstream_model_id.to_lowercase()
            {
                return Err(invalid("pricing_catalog_search_identity"));
            }
            result.push(rule);
        }
        validate_catalog_rules(&result)?;
        Ok(result)
    }

    pub(crate) async fn query(
        &self,
        provider: Option<&str>,
        model: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<PricingRuleBody>, usize), ApiError> {
        let filter = |value: Option<&str>| {
            value
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_lowercase)
        };
        let provider = filter(provider);
        let model = filter(model);
        let page_size = page_size.clamp(1, 100);
        let offset = page.max(1).saturating_sub(1).saturating_mul(page_size);
        if provider.is_none() && model.is_none() {
            let end = offset.saturating_add(page_size);
            let mut start = 0usize;
            let mut selected = Vec::with_capacity(page_size);
            for reference in &self.index.pages {
                let next = start + reference.rule_count;
                if start < end && next > offset {
                    selected.extend(
                        self.read_page(reference.page)
                            .await?
                            .into_iter()
                            .skip(offset.saturating_sub(start))
                            .take(end.min(next) - offset.max(start)),
                    );
                }
                start = next;
                if start >= end {
                    break;
                }
            }
            validate_catalog_rules(&selected)?;
            return Ok((selected, self.index.total_rules));
        }
        let search = self.search_index().await?;
        let matches: Vec<_> = search
            .entries
            .iter()
            .filter(|entry| {
                provider
                    .as_ref()
                    .is_none_or(|p| entry.provider_code.to_lowercase().contains(p))
                    && model
                        .as_ref()
                        .is_none_or(|m| entry.upstream_model_id.to_lowercase().contains(m))
            })
            .collect();
        let total = matches.len();
        let selected: Vec<_> = matches.into_iter().skip(offset).take(page_size).collect();
        Ok((self.selected(&selected).await?, total))
    }

    pub(crate) async fn select_ids(&self, ids: &[Uuid]) -> Result<Vec<PricingRuleBody>, ApiError> {
        let wanted: HashSet<_> = ids.iter().copied().collect();
        if wanted.len() != ids.len() {
            return Err(invalid("pricing_catalog_id"));
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let search = self.search_index().await?;
        let selected: Vec<_> = search
            .entries
            .iter()
            .filter(|entry| wanted.contains(&entry.id))
            .collect();
        if selected.len() != ids.len() {
            return Err(invalid("pricing_catalog_id"));
        }
        self.selected(&selected).await
    }
}

async fn fetch_document(
    client: &reqwest::Client,
    url: &str,
    size_limit: usize,
) -> Result<Vec<u8>, ApiError> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > size_limit as u64)
    {
        return Err(invalid("pricing_catalog_too_large"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if chunk.len() > size_limit.saturating_sub(bytes.len()) {
            return Err(invalid("pricing_catalog_too_large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
