use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::ResolvedOfficialAgentFlowTemplateSourceConfig,
    official_plugin_registry::rewrite_github_raw_url,
};

pub const AGENT_FLOW_CATALOG_SCHEMA_VERSION: &str = "1flowbase.agent-flow-catalog/v1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentFlowCatalogApplication {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct AgentFlowCatalogVersion {
    pub template_id: String,
    pub release_version: u64,
    pub exported_from_system_version: String,
    pub exported_at: String,
    pub application: AgentFlowCatalogApplication,
    pub download_url: String,
    pub checksum: String,
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentFlowCatalogTemplate {
    template_id: String,
    source_path: String,
    #[serde(default)]
    versions: Vec<AgentFlowCatalogVersion>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct AgentFlowCatalogDocument {
    schema_version: String,
    #[serde(default)]
    templates: Vec<AgentFlowCatalogTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalAgentFlowTemplateReceipt {
    pub template_id: String,
    pub release_version: u64,
    pub exported_from_system_version: String,
    pub exported_at: String,
    pub application: AgentFlowCatalogApplication,
    pub checksum: String,
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

impl From<&AgentFlowCatalogVersion> for LocalAgentFlowTemplateReceipt {
    fn from(version: &AgentFlowCatalogVersion) -> Self {
        Self {
            template_id: version.template_id.clone(),
            release_version: version.release_version,
            exported_from_system_version: version.exported_from_system_version.clone(),
            exported_at: version.exported_at.clone(),
            application: version.application.clone(),
            checksum: version.checksum.clone(),
            algorithm: version.algorithm.clone(),
            key_id: version.key_id.clone(),
            signature: version.signature.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentFlowTemplateLibraryEntry {
    pub template_id: String,
    pub source_path: Option<String>,
    pub remote_versions: Vec<AgentFlowCatalogVersion>,
    pub local_versions: Vec<LocalAgentFlowTemplateReceipt>,
    pub current_release_version: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AgentFlowTemplateLibraryCatalog {
    pub remote_available: bool,
    pub remote_error: Option<String>,
    pub templates: Vec<AgentFlowTemplateLibraryEntry>,
}

#[async_trait]
pub trait AgentFlowTemplateLibraryPort: Send + Sync {
    async fn catalog(&self) -> Result<AgentFlowTemplateLibraryCatalog> {
        bail!("Agent Flow template library is unavailable")
    }
    async fn sync(
        &self,
        template_id: &str,
        release_version: Option<u64>,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let _ = (template_id, release_version);
        bail!("Agent Flow template library is unavailable")
    }
    async fn resolve_artifact(
        &self,
        template_id: &str,
        release_version: Option<u64>,
    ) -> Result<Vec<u8>> {
        let _ = (template_id, release_version);
        bail!("Agent Flow template library is unavailable")
    }
    async fn switch_current(
        &self,
        template_id: &str,
        release_version: u64,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let _ = (template_id, release_version);
        bail!("Agent Flow template library is unavailable")
    }
    async fn delete_local_version(&self, template_id: &str, release_version: u64) -> Result<()> {
        let _ = (template_id, release_version);
        bail!("Agent Flow template library is unavailable")
    }
    async fn repair(
        &self,
        template_id: &str,
        release_version: u64,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let _ = (template_id, release_version);
        bail!("Agent Flow template library is unavailable")
    }
}

#[derive(Clone)]
pub struct ApiAgentFlowTemplateLibrary {
    catalog_url: String,
    github_proxy_url: Option<String>,
    root: PathBuf,
    trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    client: Client,
}

impl ApiAgentFlowTemplateLibrary {
    pub fn new(
        source: ResolvedOfficialAgentFlowTemplateSourceConfig,
        root: PathBuf,
        trusted_public_keys: Vec<plugin_framework::TrustedPublicKey>,
    ) -> Self {
        Self {
            catalog_url: rewrite_github_raw_url(
                &source.index_url,
                source.github_proxy_url.as_deref(),
            ),
            github_proxy_url: source.github_proxy_url,
            root,
            trusted_public_keys,
            client: Client::new(),
        }
    }

    async fn remote_catalog(&self) -> Result<AgentFlowCatalogDocument> {
        let bytes = self.download_bytes(&self.catalog_url).await?;
        let mut catalog: AgentFlowCatalogDocument = serde_json::from_slice(&bytes)
            .context("failed to decode official Agent Flow catalog")?;
        if catalog.schema_version != AGENT_FLOW_CATALOG_SCHEMA_VERSION {
            bail!("unsupported official Agent Flow catalog schema");
        }
        for template in &mut catalog.templates {
            validate_template_id(&template.template_id)?;
            for version in &mut template.versions {
                if version.template_id != template.template_id || version.release_version == 0 {
                    bail!("official Agent Flow catalog version identity mismatch");
                }
                version.download_url = rewrite_github_raw_url(
                    &resolve_download_url(
                        &self.catalog_url,
                        &template.source_path,
                        &version.download_url,
                    )?,
                    self.github_proxy_url.as_deref(),
                );
            }
        }
        Ok(catalog)
    }

    async fn download_bytes(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("failed to request official Agent Flow source from {url}"))?
            .error_for_status()
            .with_context(|| format!("official Agent Flow source returned an error for {url}"))?
            .bytes()
            .await
            .context("failed to read official Agent Flow response body")?
            .to_vec())
    }

    async fn selected_remote_version(
        &self,
        template_id: &str,
        release_version: Option<u64>,
    ) -> Result<AgentFlowCatalogVersion> {
        validate_template_id(template_id)?;
        let catalog = self.remote_catalog().await?;
        let template = catalog
            .templates
            .into_iter()
            .find(|template| template.template_id == template_id)
            .ok_or_else(|| anyhow!("official Agent Flow template not found"))?;
        match release_version {
            Some(release_version) => template
                .versions
                .into_iter()
                .find(|version| version.release_version == release_version)
                .ok_or_else(|| anyhow!("official Agent Flow template release not found")),
            None => template
                .versions
                .into_iter()
                .max_by_key(|version| version.release_version)
                .ok_or_else(|| anyhow!("official Agent Flow template has no releases")),
        }
    }

    async fn install_remote_version(
        &self,
        version: AgentFlowCatalogVersion,
        repair: bool,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let root = self.root.clone();
        let template_id = version.template_id.clone();
        let release_version = version.release_version;
        let existing =
            tokio::task::spawn_blocking(move || read_receipt(&root, &template_id, release_version))
                .await
                .context("Agent Flow receipt task failed")??;
        if let Some(existing) = &existing {
            if existing.checksum != version.checksum {
                bail!("same Agent Flow template release has a different checksum");
            }
            if !repair {
                let root = self.root.clone();
                let template_id = version.template_id.clone();
                let release_version = version.release_version;
                tokio::task::spawn_blocking(move || {
                    write_current(&root, &template_id, release_version)
                })
                .await
                .context("Agent Flow current pointer task failed")??;
                return Ok(existing.clone());
            }
        } else if repair {
            bail!("local Agent Flow template release not found for repair");
        }

        let bytes = self.download_bytes(&version.download_url).await?;
        plugin_framework::verify_trusted_ed25519_artifact(
            &bytes,
            &version.checksum,
            &version.algorithm,
            &version.key_id,
            &version.signature,
            &self.trusted_public_keys,
        )?;
        let artifact: serde_json::Value =
            serde_json::from_slice(&bytes).context("official Agent Flow artifact is not JSON")?;
        validate_artifact_identity(&artifact, &version)?;
        let receipt = LocalAgentFlowTemplateReceipt::from(&version);
        let root = self.root.clone();
        let stored_receipt = receipt.clone();
        tokio::task::spawn_blocking(move || {
            write_release(&root, &stored_receipt, &bytes)?;
            if !repair {
                write_current(
                    &root,
                    &stored_receipt.template_id,
                    stored_receipt.release_version,
                )?;
            }
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("Agent Flow release write task failed")??;
        Ok(receipt)
    }
}

#[async_trait]
impl AgentFlowTemplateLibraryPort for ApiAgentFlowTemplateLibrary {
    async fn catalog(&self) -> Result<AgentFlowTemplateLibraryCatalog> {
        let root = self.root.clone();
        let local = tokio::task::spawn_blocking(move || scan_local_library(&root))
            .await
            .context("Agent Flow local catalog task failed")??;
        let remote = self.remote_catalog().await;
        let (remote_available, remote_error, remote_templates) = match remote {
            Ok(catalog) => (true, None, catalog.templates),
            Err(error) => (false, Some(error.to_string()), Vec::new()),
        };
        let mut entries = BTreeMap::<String, AgentFlowTemplateLibraryEntry>::new();
        for (template_id, local_versions, current_release_version) in local {
            entries.insert(
                template_id.clone(),
                AgentFlowTemplateLibraryEntry {
                    template_id,
                    source_path: None,
                    remote_versions: Vec::new(),
                    local_versions,
                    current_release_version,
                },
            );
        }
        for template in remote_templates {
            let entry = entries
                .entry(template.template_id.clone())
                .or_insert_with(|| AgentFlowTemplateLibraryEntry {
                    template_id: template.template_id.clone(),
                    source_path: None,
                    remote_versions: Vec::new(),
                    local_versions: Vec::new(),
                    current_release_version: None,
                });
            entry.source_path = Some(template.source_path);
            entry.remote_versions = template.versions;
            entry
                .remote_versions
                .sort_by_key(|version| version.release_version);
        }
        Ok(AgentFlowTemplateLibraryCatalog {
            remote_available,
            remote_error,
            templates: entries.into_values().collect(),
        })
    }

    async fn sync(
        &self,
        template_id: &str,
        release_version: Option<u64>,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let version = self
            .selected_remote_version(template_id, release_version)
            .await?;
        self.install_remote_version(version, false).await
    }

    async fn resolve_artifact(
        &self,
        template_id: &str,
        release_version: Option<u64>,
    ) -> Result<Vec<u8>> {
        validate_template_id(template_id)?;
        let root = self.root.clone();
        let local_template_id = template_id.to_string();
        let local_versions =
            tokio::task::spawn_blocking(move || local_versions(&root, &local_template_id))
                .await
                .context("Agent Flow local history task failed")??;
        if local_versions.is_empty() {
            self.sync(template_id, release_version).await?;
        }
        let root = self.root.clone();
        let template_id = template_id.to_string();
        let (bytes, receipt) = tokio::task::spawn_blocking(move || {
            let selected = match release_version {
                Some(version) => version,
                None => read_current(&root, &template_id)?.ok_or_else(|| {
                    anyhow!("local Agent Flow template current release is missing")
                })?,
            };
            let receipt = read_receipt(&root, &template_id, selected)?
                .ok_or_else(|| anyhow!("local Agent Flow template receipt is missing"))?;
            let bytes = fs::read(template_path(&root, &template_id, selected))
                .context("failed to read local Agent Flow template artifact")?;
            Ok::<_, anyhow::Error>((bytes, receipt))
        })
        .await
        .context("Agent Flow artifact read task failed")??;
        plugin_framework::verify_trusted_ed25519_artifact(
            &bytes,
            &receipt.checksum,
            &receipt.algorithm,
            &receipt.key_id,
            &receipt.signature,
            &self.trusted_public_keys,
        )?;
        Ok(bytes)
    }

    async fn switch_current(
        &self,
        template_id: &str,
        release_version: u64,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let root = self.root.clone();
        let template_id = template_id.to_string();
        tokio::task::spawn_blocking(move || {
            let receipt = read_receipt(&root, &template_id, release_version)?
                .ok_or_else(|| anyhow!("local Agent Flow template release not found"))?;
            write_current(&root, &template_id, release_version)?;
            Ok(receipt)
        })
        .await
        .context("Agent Flow current switch task failed")?
    }

    async fn delete_local_version(&self, template_id: &str, release_version: u64) -> Result<()> {
        let root = self.root.clone();
        let template_id = template_id.to_string();
        tokio::task::spawn_blocking(move || {
            validate_template_id(&template_id)?;
            let release_dir = release_dir(&root, &template_id, release_version);
            if !release_dir.is_dir() {
                bail!("local Agent Flow template release not found");
            }
            fs::remove_dir_all(&release_dir)
                .context("failed to delete local Agent Flow template release")?;
            let remaining = local_versions(&root, &template_id)?;
            if let Some(highest) = remaining.iter().map(|item| item.release_version).max() {
                if read_current(&root, &template_id)? == Some(release_version) {
                    write_current(&root, &template_id, highest)?;
                }
            } else {
                let template_dir = template_dir(&root, &template_id);
                if template_dir.exists() {
                    fs::remove_dir_all(template_dir)
                        .context("failed to remove empty local Agent Flow template directory")?;
                }
            }
            Ok(())
        })
        .await
        .context("Agent Flow release deletion task failed")?
    }

    async fn repair(
        &self,
        template_id: &str,
        release_version: u64,
    ) -> Result<LocalAgentFlowTemplateReceipt> {
        let remote = self
            .selected_remote_version(template_id, Some(release_version))
            .await?;
        self.install_remote_version(remote, true).await
    }
}

fn validate_template_id(template_id: &str) -> Result<()> {
    if template_id.is_empty()
        || !template_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("invalid Agent Flow template id");
    }
    Ok(())
}

fn validate_artifact_identity(
    artifact: &serde_json::Value,
    version: &AgentFlowCatalogVersion,
) -> Result<()> {
    let entries = artifact
        .get("applications")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("official Agent Flow artifact is not an application archive"))?;
    if entries.len() != 1 {
        bail!("official Agent Flow artifact must contain exactly one application");
    }
    let entry = &entries[0];
    let artifact_template_id = entry.get("template_id").and_then(serde_json::Value::as_str);
    let artifact_release_version = entry
        .get("release_version")
        .and_then(serde_json::Value::as_u64);
    let artifact_name = entry
        .pointer("/application/name")
        .and_then(serde_json::Value::as_str);
    let artifact_description = entry
        .pointer("/application/description")
        .and_then(serde_json::Value::as_str);
    if artifact_template_id != Some(version.template_id.as_str())
        || artifact_release_version != Some(version.release_version)
        || artifact_name != Some(version.application.name.as_str())
        || artifact_description != Some(version.application.description.as_str())
    {
        bail!("official Agent Flow artifact identity does not match catalog release");
    }
    Ok(())
}

fn resolve_download_url(
    catalog_url: &str,
    source_path: &str,
    download_url: &str,
) -> Result<String> {
    if reqwest::Url::parse(download_url).is_ok() {
        return Ok(download_url.to_string());
    }
    let base = reqwest::Url::parse(catalog_url)?;
    let source = if source_path.trim().is_empty() {
        base
    } else {
        base.join(source_path)?
    };
    Ok(source.join(download_url)?.to_string())
}

fn template_dir(root: &Path, template_id: &str) -> PathBuf {
    root.join(template_id)
}

fn release_dir(root: &Path, template_id: &str, release_version: u64) -> PathBuf {
    template_dir(root, template_id)
        .join("releases")
        .join(release_version.to_string())
}

fn template_path(root: &Path, template_id: &str, release_version: u64) -> PathBuf {
    release_dir(root, template_id, release_version).join("template.json")
}

fn receipt_path(root: &Path, template_id: &str, release_version: u64) -> PathBuf {
    release_dir(root, template_id, release_version).join("receipt.json")
}

fn read_receipt(
    root: &Path,
    template_id: &str,
    release_version: u64,
) -> Result<Option<LocalAgentFlowTemplateReceipt>> {
    validate_template_id(template_id)?;
    let path = receipt_path(root, template_id, release_version);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&fs::read(&path)?)?))
}

fn local_versions(root: &Path, template_id: &str) -> Result<Vec<LocalAgentFlowTemplateReceipt>> {
    validate_template_id(template_id)?;
    let releases = template_dir(root, template_id).join("releases");
    if !releases.is_dir() {
        return Ok(Vec::new());
    }
    let mut versions = Vec::new();
    for entry in fs::read_dir(releases)? {
        let entry = entry?;
        let Ok(version) = entry.file_name().to_string_lossy().parse::<u64>() else {
            continue;
        };
        if template_path(root, template_id, version).is_file() {
            if let Some(receipt) = read_receipt(root, template_id, version)? {
                versions.push(receipt);
            }
        }
    }
    versions.sort_by_key(|receipt| receipt.release_version);
    Ok(versions)
}

fn scan_local_library(
    root: &Path,
) -> Result<Vec<(String, Vec<LocalAgentFlowTemplateReceipt>, Option<u64>)>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut templates = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let template_id = entry.file_name().to_string_lossy().to_string();
        if validate_template_id(&template_id).is_err() {
            continue;
        }
        let versions = local_versions(root, &template_id)?;
        if !versions.is_empty() {
            templates.push((
                template_id.clone(),
                versions,
                read_current(root, &template_id)?,
            ));
        }
    }
    templates.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(templates)
}

fn write_release(root: &Path, receipt: &LocalAgentFlowTemplateReceipt, bytes: &[u8]) -> Result<()> {
    let directory = release_dir(root, &receipt.template_id, receipt.release_version);
    fs::create_dir_all(&directory)?;
    atomic_write(&directory.join("template.json"), bytes)?;
    atomic_write(
        &directory.join("receipt.json"),
        &serde_json::to_vec_pretty(receipt)?,
    )
}

fn write_current(root: &Path, template_id: &str, release_version: u64) -> Result<()> {
    if read_receipt(root, template_id, release_version)?.is_none() {
        bail!("local Agent Flow template release not found");
    }
    let directory = template_dir(root, template_id);
    fs::create_dir_all(&directory)?;
    atomic_write(
        &directory.join("current"),
        release_version.to_string().as_bytes(),
    )
}

fn read_current(root: &Path, template_id: &str) -> Result<Option<u64>> {
    let path = template_dir(root, template_id).join("current");
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(fs::read_to_string(path)?.trim().parse().context(
        "invalid local Agent Flow template current pointer",
    )?))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Agent Flow template path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        Uuid::now_v7()
    ));
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
