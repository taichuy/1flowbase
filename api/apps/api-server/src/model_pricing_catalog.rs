use std::{fs, path::Path, time::Duration};

use anyhow::{bail, Context, Result};
use control_plane::ports::{BillingRepository, UpsertPricingRuleInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    error_response::ApiError,
    routes::billing::{body_to_rule, PricingRuleBody},
};

const SOURCE_SCHEMA_VERSION: &str = "1flowbase.model-pricing-source/v1";
const INDEX_SCHEMA_VERSION: &str = "1flowbase.model-pricing-index/v1";
const PAGE_SCHEMA_VERSION: &str = "1flowbase.model-pricing-page/v1";
const BUILTIN_ZERO_SOURCE: &[u8] =
    include_bytes!("../resources/model-pricing/@zero/any/pricing.json");

#[derive(Debug, Deserialize)]
struct ModelPricingSource {
    schema_version: String,
    provider_code: String,
    upstream_model_id: String,
    currency_code: String,
    rules: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct CatalogSourceMetadata {
    catalog_version: String,
}

#[derive(Debug, Deserialize)]
struct RemotePricingCatalogIndex {
    schema_version: String,
    catalog_version: String,
    currency_code: String,
    total_rules: usize,
    pages: Vec<RemotePricingCatalogPageReference>,
}

#[derive(Debug, Deserialize)]
struct RemotePricingCatalogPageReference {
    page: usize,
    rule_count: usize,
    checksum: String,
    locator: String,
}

#[derive(Debug, Deserialize)]
struct RemotePricingCatalogPage {
    schema_version: String,
    catalog_version: String,
    currency_code: String,
    page: usize,
    rules: Vec<PricingRuleBody>,
}

#[derive(Debug)]
pub(crate) struct RemotePricingCatalog {
    pub(crate) catalog_version: String,
    pub(crate) rules: Vec<PricingRuleBody>,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub(crate) struct PricingRuleInstallSummary {
    pub(crate) inserted: usize,
    pub(crate) skipped: usize,
    pub(crate) updated: usize,
    pub(crate) deleted: usize,
}

pub(crate) fn builtin_pricing_rules() -> Result<Vec<PricingRuleBody>> {
    decode_source(BUILTIN_ZERO_SOURCE, None, Some("zero"))
}

pub(crate) fn load_bootstrap_pricing_rules(root: &Path) -> Result<Vec<PricingRuleBody>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let source_version = fs::read(root.join("catalog-source.json"))
        .ok()
        .map(|bytes| serde_json::from_slice::<CatalogSourceMetadata>(&bytes))
        .transpose()
        .context("invalid model pricing bootstrap catalog-source.json")?
        .map(|metadata| metadata.catalog_version);
    let mut rules = Vec::new();
    let mut providers = fs::read_dir(root)
        .with_context(|| format!("read model pricing bootstrap root {}", root.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    providers.sort_by_key(|entry| entry.file_name());
    for provider in providers {
        if !provider.file_type()?.is_dir()
            || !provider.file_name().to_string_lossy().starts_with('@')
        {
            continue;
        }
        let provider_code = provider
            .file_name()
            .to_string_lossy()
            .trim_start_matches('@')
            .to_owned();
        let mut models = fs::read_dir(provider.path())?.collect::<std::io::Result<Vec<_>>>()?;
        models.sort_by_key(|entry| entry.file_name());
        for model in models {
            if !model.file_type()?.is_dir() {
                continue;
            }
            let source_path = model.path().join("pricing.json");
            if !source_path.is_file() {
                continue;
            }
            rules.extend(decode_source(
                &fs::read(&source_path)?,
                source_version.as_deref(),
                Some(&provider_code),
            )?);
        }
    }
    Ok(rules)
}

fn decode_source(
    bytes: &[u8],
    source_version: Option<&str>,
    expected_provider_code: Option<&str>,
) -> Result<Vec<PricingRuleBody>> {
    let source: ModelPricingSource = serde_json::from_slice(bytes)?;
    if source.schema_version != SOURCE_SCHEMA_VERSION || source.currency_code != "USD" {
        bail!("invalid model pricing source document");
    }
    if expected_provider_code.is_some_and(|expected| source.provider_code != expected) {
        bail!("model pricing source provider does not match its directory");
    }
    source
        .rules
        .into_iter()
        .map(|mut source_rule| {
            source_rule.insert(
                "provider_code".to_owned(),
                serde_json::Value::String(source.provider_code.clone()),
            );
            source_rule.insert(
                "upstream_model_id".to_owned(),
                serde_json::Value::String(source.upstream_model_id.clone()),
            );
            let mut rule =
                serde_json::from_value::<PricingRuleBody>(serde_json::Value::Object(source_rule))?;
            let id = rule
                .id
                .context("model pricing source rule id is required")?;
            rule.currency_code = Some("USD".to_owned());
            rule.source_kind = Some("official".to_owned());
            rule.source_catalog_id = Some(id.to_string());
            rule.source_version = source_version.map(str::to_owned);
            Ok(rule)
        })
        .collect()
}

pub(crate) async fn install_pricing_rules_if_absent<R: BillingRepository>(
    repository: &R,
    actor_user_id: Uuid,
    rules: Vec<PricingRuleBody>,
) -> Result<PricingRuleInstallSummary, ApiError> {
    let mut summary = PricingRuleInstallSummary::default();
    for body in rules {
        let rule = body_to_rule(body, actor_user_id, None)?;
        if rule.source_kind != "official" || rule.source_catalog_id.is_none() {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_catalog_rule",
            )
            .into());
        }
        if repository
            .insert_pricing_rule_if_absent(&UpsertPricingRuleInput { rule })
            .await?
            .is_some()
        {
            summary.inserted += 1;
        } else {
            summary.skipped += 1;
        }
    }
    Ok(summary)
}

pub(crate) async fn fetch_remote_pricing_catalog(
    catalog_index_url: &str,
) -> Result<RemotePricingCatalog, ApiError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(8))
        .build()?;
    let index_bytes = fetch_document(&client, catalog_index_url, 512 * 1024).await?;
    let index: RemotePricingCatalogIndex = serde_json::from_slice(&index_bytes)?;
    if index.schema_version != INDEX_SCHEMA_VERSION
        || index.currency_code != "USD"
        || index.pages.len() > 1_000
        || index.total_rules > 100_000
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_index",
        )
        .into());
    }
    let page_locator_prefix = catalog_index_url.strip_suffix("index.json").ok_or(
        control_plane::errors::ControlPlaneError::InvalidInput("pricing_catalog_index_url"),
    )?;
    let mut rules = Vec::with_capacity(index.total_rules);
    for (position, reference) in index.pages.into_iter().enumerate() {
        let expected_page = position + 1;
        let expected_locator = format!("{page_locator_prefix}pages/{expected_page}.json");
        if reference.page != expected_page || reference.locator != expected_locator {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "pricing_catalog_page_locator",
            )
            .into());
        }
        let bytes = fetch_document(&client, &reference.locator, 2 * 1024 * 1024).await?;
        let actual_checksum = format!("sha256:{:x}", Sha256::digest(&bytes));
        if actual_checksum != reference.checksum {
            return Err(control_plane::errors::ControlPlaneError::Conflict(
                "pricing_catalog_page_checksum_mismatch",
            )
            .into());
        }
        let page: RemotePricingCatalogPage = serde_json::from_slice(&bytes)?;
        if page.schema_version != PAGE_SCHEMA_VERSION
            || page.catalog_version != index.catalog_version
            || page.currency_code != "USD"
            || page.page != reference.page
            || page.rules.len() != reference.rule_count
        {
            return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "pricing_catalog_page",
            )
            .into());
        }
        rules.extend(page.rules);
    }
    if rules.len() != index.total_rules {
        return Err(control_plane::errors::ControlPlaneError::Conflict(
            "pricing_catalog_rule_count_mismatch",
        )
        .into());
    }
    Ok(RemotePricingCatalog {
        catalog_version: index.catalog_version,
        rules,
    })
}

async fn fetch_document(
    client: &reqwest::Client,
    url: &str,
    size_limit: usize,
) -> Result<Vec<u8>, ApiError> {
    let response = client.get(url).send().await?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > size_limit as u64)
    {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_too_large",
        )
        .into());
    }
    let bytes = response.bytes().await?;
    if bytes.len() > size_limit {
        return Err(control_plane::errors::ControlPlaneError::InvalidInput(
            "pricing_catalog_too_large",
        )
        .into());
    }
    Ok(bytes.to_vec())
}
