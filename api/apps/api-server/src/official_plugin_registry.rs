use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use async_trait::async_trait;
use control_plane::ports::{
    DownloadedOfficialPluginPackage, OfficialPluginArtifact, OfficialPluginCatalogSnapshot,
    OfficialPluginCatalogSource, OfficialPluginI18nSummary, OfficialPluginSourceEntry,
    OfficialPluginSourcePort,
};
use plugin_framework::RuntimeTarget;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::config::ResolvedOfficialPluginSourceConfig;

const GITHUB_RAW_CONTENT_BASE_URL: &str = "https://raw.githubusercontent.com/";
const OFFICIAL_PLUGIN_REGISTRY_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct ApiOfficialPluginRegistry {
    source_kind: String,
    source_label: String,
    registry_url: String,
    github_proxy_url: Option<String>,
    trust_mode: String,
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    client: Client,
    registry_cache: Arc<RwLock<Option<CachedOfficialRegistryDocument>>>,
}

#[derive(Clone)]
struct CachedOfficialRegistryDocument {
    fetched_at: Instant,
    document: OfficialRegistryDocument,
}

impl ApiOfficialPluginRegistry {
    pub fn new(
        source: ResolvedOfficialPluginSourceConfig,
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        let registry_url =
            rewrite_github_raw_url(&source.registry_url, source.github_proxy_url.as_deref());
        Self {
            source_kind: source.source_kind,
            source_label: source.source_label,
            registry_url,
            github_proxy_url: source.github_proxy_url,
            trust_mode: source.trust_mode,
            trusted_public_keys,
            client: Client::new(),
            registry_cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn fetch_registry(&self) -> Result<OfficialRegistryDocument> {
        if let Some(document) = self.cached_registry().await {
            return Ok(document);
        }

        let document = self
            .client
            .get(&self.registry_url)
            .send()
            .await
            .context("failed to request official plugin registry")?
            .error_for_status()
            .context("official plugin registry returned an error status")?
            .json::<OfficialRegistryDocument>()
            .await
            .context("failed to decode official plugin registry")?;
        *self.registry_cache.write().await = Some(CachedOfficialRegistryDocument {
            fetched_at: Instant::now(),
            document: document.clone(),
        });

        Ok(document)
    }

    async fn cached_registry(&self) -> Option<OfficialRegistryDocument> {
        self.registry_cache
            .read()
            .await
            .as_ref()
            .filter(|cached| cached.fetched_at.elapsed() <= OFFICIAL_PLUGIN_REGISTRY_CACHE_TTL)
            .map(|cached| cached.document.clone())
    }

    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request official plugin package from {url}"))?
            .error_for_status()
            .with_context(|| format!("official plugin package request failed for {url}"))?
            .bytes()
            .await
            .context("failed to read official plugin package response body")?
            .to_vec())
    }
}

#[async_trait]
impl OfficialPluginSourcePort for ApiOfficialPluginRegistry {
    async fn list_official_catalog(&self) -> Result<OfficialPluginCatalogSnapshot> {
        let document = self.fetch_registry().await?;
        let host = RuntimeTarget::current_host().unwrap_or_else(|_| {
            RuntimeTarget::from_rust_target_triple("x86_64-unknown-linux-musl").unwrap()
        });
        Ok(OfficialPluginCatalogSnapshot {
            source: OfficialPluginCatalogSource {
                source_kind: self.source_kind.clone(),
                source_label: self.source_label.clone(),
                registry_url: self.registry_url.clone(),
            },
            entries: document
                .plugins
                .into_iter()
                .filter_map(|entry| {
                    let selected = select_artifact_for_host(&entry, &host)?;
                    let namespace = format!("plugin.{}", entry.provider_code);
                    let i18n_summary = normalize_i18n_summary(&entry);
                    Some(OfficialPluginSourceEntry {
                        release_tag: format!("{}-v{}", entry.provider_code, entry.latest_version),
                        plugin_id: entry.plugin_id,
                        plugin_type: entry.plugin_type,
                        provider_code: entry.provider_code,
                        namespace,
                        protocol: entry.protocol,
                        latest_version: entry.latest_version,
                        minimum_host_version: entry.minimum_host_version,
                        icon: entry.icon.map(|url| {
                            rewrite_github_raw_url(&url, self.github_proxy_url.as_deref())
                        }),
                        selected_artifact: OfficialPluginArtifact {
                            os: selected.os,
                            arch: selected.arch,
                            libc: selected.libc,
                            rust_target: selected.rust_target,
                            download_url: rewrite_github_raw_url(
                                &selected.download_url,
                                self.github_proxy_url.as_deref(),
                            ),
                            checksum: selected.checksum,
                            signature_algorithm: selected.signature_algorithm,
                            signing_key_id: selected.signing_key_id,
                        },
                        i18n_summary,
                        trust_mode: self.trust_mode.clone(),
                        help_url: entry.help_url,
                        model_discovery_mode: entry.model_discovery_mode,
                    })
                })
                .collect(),
        })
    }

    async fn download_plugin(
        &self,
        entry: &OfficialPluginSourceEntry,
    ) -> Result<DownloadedOfficialPluginPackage> {
        let download_url = rewrite_github_raw_url(
            &entry.selected_artifact.download_url,
            self.github_proxy_url.as_deref(),
        );
        Ok(DownloadedOfficialPluginPackage {
            file_name: format!(
                "{}-{}.1flowbasepkg",
                entry.provider_code, entry.latest_version
            ),
            package_bytes: self.download_bytes(&download_url).await?,
        })
    }

    fn trusted_public_keys(&self) -> Vec<plugin_framework::TrustedPublicKey> {
        self.trusted_public_keys.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct OfficialRegistryDocument {
    #[allow(dead_code)]
    version: u32,
    #[allow(dead_code)]
    generated_at: Option<String>,
    #[serde(default)]
    plugins: Vec<OfficialRegistryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfficialRegistryArtifact {
    pub os: String,
    pub arch: String,
    #[serde(default)]
    pub libc: Option<String>,
    pub rust_target: String,
    pub download_url: String,
    pub checksum: String,
    #[serde(default)]
    pub signature_algorithm: Option<String>,
    #[serde(default)]
    pub signing_key_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfficialRegistryEntry {
    pub plugin_id: String,
    #[serde(default = "default_plugin_type")]
    pub plugin_type: String,
    pub provider_code: String,
    pub display_name: String,
    pub protocol: String,
    pub latest_version: String,
    #[serde(default = "default_minimum_host_version")]
    pub minimum_host_version: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub help_url: Option<String>,
    pub model_discovery_mode: String,
    #[serde(default)]
    pub i18n_summary: OfficialRegistryI18nSummary,
    #[serde(default)]
    pub artifacts: Vec<OfficialRegistryArtifact>,
}

pub fn select_artifact_for_host(
    entry: &OfficialRegistryEntry,
    host: &RuntimeTarget,
) -> Option<OfficialRegistryArtifact> {
    entry
        .artifacts
        .iter()
        .cloned()
        .max_by_key(|artifact| {
            if artifact.os != host.os || artifact.arch != host.arch {
                return 0_u8;
            }

            match (host.libc.as_deref(), artifact.libc.as_deref()) {
                (Some(left), Some(right)) if left == right => 3,
                (Some("gnu"), Some("musl")) if host.os == "linux" => 2,
                (_, None) => 1,
                (None, Some(_)) => 1,
                _ => 0,
            }
        })
        .filter(|artifact| artifact.os == host.os && artifact.arch == host.arch)
}

pub(crate) fn rewrite_github_raw_url(url: &str, github_proxy_url: Option<&str>) -> String {
    let Some(github_proxy_url) = github_proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return url.to_string();
    };
    let github_proxy_url = github_proxy_url.trim_end_matches('/');
    let proxied_raw_prefix = format!("{github_proxy_url}/{GITHUB_RAW_CONTENT_BASE_URL}");
    if url.starts_with(&proxied_raw_prefix) || !url.starts_with(GITHUB_RAW_CONTENT_BASE_URL) {
        return url.to_string();
    }

    format!("{github_proxy_url}/{url}")
}

fn default_plugin_type() -> String {
    "model_provider".to_string()
}

fn default_minimum_host_version() -> String {
    "0.1.0".to_string()
}

fn default_registry_locale() -> String {
    "en_US".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OfficialRegistryI18nSummary {
    #[serde(default = "default_registry_locale")]
    pub default_locale: String,
    #[serde(default)]
    pub available_locales: Vec<String>,
    #[serde(default)]
    pub bundles: BTreeMap<String, Value>,
}

fn normalize_i18n_summary(entry: &OfficialRegistryEntry) -> OfficialPluginI18nSummary {
    let mut bundles = entry.i18n_summary.bundles.clone();
    let default_locale = if entry.i18n_summary.default_locale.trim().is_empty() {
        default_registry_locale()
    } else {
        entry.i18n_summary.default_locale.clone()
    };
    if bundles.is_empty() {
        bundles.insert(
            default_locale.clone(),
            json!({
                "plugin": { "label": entry.display_name },
                "provider": { "label": entry.display_name },
            }),
        );
    }

    let mut available_locales = entry.i18n_summary.available_locales.clone();
    if available_locales.is_empty() {
        available_locales = bundles.keys().cloned().collect();
    }

    OfficialPluginI18nSummary {
        default_locale,
        available_locales,
        bundles,
    }
}
