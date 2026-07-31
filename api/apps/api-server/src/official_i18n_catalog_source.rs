use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use control_plane::ports::{OfficialI18nCatalogReleaseDescriptor, OfficialI18nCatalogSourcePort};
use domain::CatalogDigest;
use reqwest::Client;
use sha2::{Digest, Sha256};

use crate::{
    config::ResolvedOfficialI18nCatalogSourceConfig,
    official_i18n_catalog_seed::{
        decode_downloaded_catalog_seed, inspect_catalog_seed, CatalogSeedInspection,
    },
    official_plugin_registry::rewrite_github_raw_url,
};

pub struct ApiOfficialI18nCatalogSource {
    latest_url: String,
    release_base_url: String,
    github_proxy_url: Option<String>,
    client: Client,
}

impl ApiOfficialI18nCatalogSource {
    pub fn new(config: ResolvedOfficialI18nCatalogSourceConfig) -> Self {
        Self {
            latest_url: rewrite_github_raw_url(
                &config.latest_url,
                config.github_proxy_url.as_deref(),
            ),
            release_base_url: config.release_base_url.trim_end_matches('/').to_owned(),
            github_proxy_url: config.github_proxy_url,
            client: Client::new(),
        }
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request official i18n catalog from {url}"))?
            .error_for_status()
            .with_context(|| format!("official i18n catalog request failed for {url}"))?
            .bytes()
            .await
            .context("failed to read official i18n catalog response body")?
            .to_vec())
    }

    pub(crate) fn fixed_release_urls(&self, catalog_version: &str) -> (String, String) {
        let release_tag = format!("i18n-catalog-v{catalog_version}");
        let asset = format!("i18n-catalog-seed-v{catalog_version}.json");
        let asset_url = format!("{}/{release_tag}/{asset}", self.release_base_url);
        let asset_url = rewrite_github_raw_url(&asset_url, self.github_proxy_url.as_deref());
        (asset_url.clone(), format!("{asset_url}.sha256"))
    }

    #[cfg(test)]
    pub(crate) fn latest_url(&self) -> &str {
        &self.latest_url
    }
}

#[async_trait]
impl OfficialI18nCatalogSourcePort for ApiOfficialI18nCatalogSource {
    async fn check_latest_release(&self) -> Result<OfficialI18nCatalogReleaseDescriptor> {
        let bytes = self.download(&self.latest_url).await?;
        let inspected = inspect_catalog_seed(&bytes)?;
        Ok(OfficialI18nCatalogReleaseDescriptor {
            catalog_version: inspected.catalog_version,
            semantic_sha256: inspected.semantic_sha256,
            seed_sha256: inspected.seed_sha256,
        })
    }

    async fn fetch_verified_release(
        &self,
        release: &OfficialI18nCatalogReleaseDescriptor,
    ) -> Result<control_plane::i18n_catalog::VerifiedOfficialCatalogSeed> {
        let (asset_url, sidecar_url) = self.fixed_release_urls(release.catalog_version.as_str());
        let sidecar = self.download(&sidecar_url).await?;
        validate_sidecar(
            &sidecar,
            release.seed_sha256.as_str(),
            release.catalog_version.as_str(),
        )?;
        let seed_bytes = self.download(&asset_url).await?;
        let actual = format!("sha256:{:x}", Sha256::digest(&seed_bytes));
        if actual != release.seed_sha256.as_str() {
            bail!("official i18n catalog Seed checksum mismatch");
        }
        decode_downloaded_catalog_seed(
            &seed_bytes,
            &CatalogSeedInspection {
                catalog_version: release.catalog_version.clone(),
                semantic_sha256: release.semantic_sha256.clone(),
                seed_sha256: release.seed_sha256.clone(),
            },
        )
    }
}

pub(crate) fn validate_sidecar(
    bytes: &[u8],
    expected_digest: &str,
    catalog_version: &str,
) -> Result<()> {
    let document = std::str::from_utf8(bytes).context("official Seed SHA sidecar is not UTF-8")?;
    let mut fields = document.split_whitespace();
    let digest = fields
        .next()
        .context("official Seed SHA sidecar has no digest")?;
    let file_name = fields
        .next()
        .context("official Seed SHA sidecar has no asset name")?;
    if fields.next().is_some()
        || format!("sha256:{digest}") != expected_digest
        || file_name != format!("i18n-catalog-seed-v{catalog_version}.json")
    {
        bail!("official Seed SHA sidecar does not match the fixed release descriptor");
    }
    CatalogDigest::new(format!("sha256:{digest}"))?;
    Ok(())
}
