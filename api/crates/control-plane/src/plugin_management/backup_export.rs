use std::{collections::BTreeMap, io::ErrorKind, path::PathBuf, sync::Arc};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
};

use crate::{
    ports::{BackupComponentWriter, ExtensionInstallationRepository, PluginRepository},
    system_backup::{BackupComponentDescriptor, BackupComponentSource, BackupSourceError},
};

const ARTIFACT_STREAM_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupArtifactInventoryReason {
    RebuildableIdentityInvalid,
    RetainedArtifactMissing,
    RetainedArtifactNotFile,
    DuplicateCurrentArtifact,
    OrphanCurrentArtifact,
    DuplicateArtifactIdentity,
}

impl BackupArtifactInventoryReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RebuildableIdentityInvalid => "rebuildable_identity_invalid",
            Self::RetainedArtifactMissing => "retained_artifact_missing",
            Self::RetainedArtifactNotFile => "retained_artifact_not_file",
            Self::DuplicateCurrentArtifact => "duplicate_current_artifact",
            Self::OrphanCurrentArtifact => "orphan_current_artifact",
            Self::DuplicateArtifactIdentity => "duplicate_artifact_identity",
        }
    }
}

#[derive(Debug, Error)]
#[error("system backup artifact inventory is invalid: {reason:?}")]
pub struct BackupArtifactInventoryError {
    pub reason: BackupArtifactInventoryReason,
    pub installation_id: uuid::Uuid,
    pub artifact_identity: String,
}

#[derive(Debug, Error)]
pub enum BackupArtifactSourceLoadError {
    #[error("failed to {operation}")]
    Infrastructure {
        operation: &'static str,
        #[source]
        source: anyhow::Error,
    },
    #[error(transparent)]
    Inventory(#[from] BackupArtifactInventoryError),
}

impl BackupArtifactSourceLoadError {
    pub fn into_inventory_error(self) -> std::result::Result<BackupArtifactInventoryError, Self> {
        match self {
            Self::Inventory(error) => Ok(error),
            error => Err(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackupArtifactKind {
    Extension,
    HostExtension,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupArtifactDisposition {
    RebuildableIdentity,
    Embedded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupArtifactEntry {
    pub installation_id: uuid::Uuid,
    pub identity: String,
    pub kind: BackupArtifactKind,
    pub category: String,
    pub organization: String,
    pub artifact_id: String,
    pub source_kind: String,
    pub version: String,
    pub expected_checksum: Option<String>,
    pub disposition: BackupArtifactDisposition,
    pub artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedArtifact {
    pub size_bytes: u64,
    pub sha256: String,
    pub content_type: &'static str,
}

/// Builds the finite artifact inventory from durable installation records.
///
/// Official, mirror and builtin sources are reproducible from their immutable identity and are
/// represented without copying local caches. Uploaded and local/drop-in sources are never assumed
/// reproducible and therefore must retain a readable package or artifact file.
pub fn build_backup_artifact_inventory(
    node_id: &str,
    plugin_installations: Vec<domain::PluginInstallationRecord>,
    plugin_instances: Vec<domain::PluginArtifactInstanceRecord>,
    extension_installations: Vec<domain::ExtensionInstallationRecord>,
) -> std::result::Result<Vec<BackupArtifactEntry>, BackupArtifactInventoryError> {
    let mut instances = BTreeMap::new();
    for instance in plugin_instances.into_iter().filter(|instance| {
        instance.node_id == node_id
            && instance.is_current
            && instance.artifact_status == domain::PluginArtifactInstanceStatus::Ready
    }) {
        if instances.contains_key(&instance.installation_id) {
            return Err(inventory_error(
                BackupArtifactInventoryReason::DuplicateCurrentArtifact,
                instance.installation_id,
                "plugin:unknown",
            ));
        }
        instances.insert(instance.installation_id, instance);
    }

    let mut inventory = BTreeMap::<String, BackupArtifactEntry>::new();
    for installation in plugin_installations {
        let instance = instances.remove(&installation.id);
        if !requires_recovery_artifact(&installation) {
            continue;
        }
        let identity = plugin_artifact_identity(&installation);
        let disposition = classify_source(&installation.source_kind);
        if disposition == BackupArtifactDisposition::RebuildableIdentity {
            validate_rebuildable_identity(&installation, &identity)?;
        }
        let instance = match disposition {
            BackupArtifactDisposition::RebuildableIdentity => instance.as_ref(),
            BackupArtifactDisposition::Embedded => Some(instance.as_ref().ok_or_else(|| {
                inventory_error(
                    BackupArtifactInventoryReason::RetainedArtifactMissing,
                    installation.id,
                    &identity,
                )
            })?),
        };
        let artifact_path = match disposition {
            BackupArtifactDisposition::RebuildableIdentity => None,
            BackupArtifactDisposition::Embedded => instance
                .and_then(|instance| {
                    instance
                        .package_path
                        .as_deref()
                        .or(instance.local_path.as_deref())
                })
                .map(PathBuf::from),
        };
        if disposition == BackupArtifactDisposition::Embedded && artifact_path.is_none() {
            return Err(inventory_error(
                BackupArtifactInventoryReason::RetainedArtifactMissing,
                installation.id,
                &identity,
            ));
        }
        let kind = if installation.category == domain::ExtensionCategory::Mcp {
            BackupArtifactKind::Mcp
        } else if installation.category == domain::ExtensionCategory::HostExtensions {
            BackupArtifactKind::HostExtension
        } else {
            BackupArtifactKind::Extension
        };
        insert_unique(
            &mut inventory,
            BackupArtifactEntry {
                installation_id: installation.id,
                identity,
                kind,
                category: installation.category.as_str().to_string(),
                organization: installation.organization,
                artifact_id: installation.plugin_id,
                source_kind: installation.source_kind,
                version: installation.plugin_version,
                expected_checksum: instance
                    .and_then(|instance| instance.local_checksum.clone())
                    .or(installation.expected_checksum),
                disposition,
                artifact_path,
            },
        )?;
    }
    if let Some((installation_id, _)) = instances.into_iter().next() {
        return Err(inventory_error(
            BackupArtifactInventoryReason::OrphanCurrentArtifact,
            installation_id,
            "plugin:unknown",
        ));
    }

    for installation in extension_installations.into_iter().filter(|record| {
        record.node_id == node_id
            && record.is_current
            && record.status == domain::ExtensionInstallationStatus::Installed
    }) {
        let identity = format!(
            "extension:{}/{}/{}@{}",
            installation.identity.category.as_str(),
            installation.identity.organization,
            installation.identity.artifact_id,
            installation.identity.version
        );
        let disposition = classify_source(&installation.source_kind);
        let kind = match installation.identity.category {
            domain::ExtensionCategory::Mcp => BackupArtifactKind::Mcp,
            domain::ExtensionCategory::HostExtensions => BackupArtifactKind::HostExtension,
            _ => BackupArtifactKind::Extension,
        };
        insert_unique(
            &mut inventory,
            BackupArtifactEntry {
                installation_id: installation.id,
                identity,
                kind,
                category: installation.identity.category.as_str().to_string(),
                organization: installation.identity.organization,
                artifact_id: installation.identity.artifact_id,
                source_kind: installation.source_kind,
                version: installation.identity.version,
                expected_checksum: installation.local_checksum,
                disposition,
                artifact_path: (disposition == BackupArtifactDisposition::Embedded)
                    .then(|| installation.local_path.map(PathBuf::from))
                    .flatten(),
            },
        )?;
    }

    Ok(inventory.into_values().collect())
}

pub async fn load_backup_artifact_sources<R>(
    repository: &R,
    node_id: &str,
) -> std::result::Result<Vec<Arc<dyn BackupComponentSource>>, BackupArtifactSourceLoadError>
where
    R: PluginRepository + ExtensionInstallationRepository,
{
    let installations = repository.list_installations().await.map_err(|source| {
        BackupArtifactSourceLoadError::Infrastructure {
            operation: "list plugin backup inventory",
            source,
        }
    })?;
    let instances = repository
        .list_artifact_instances(node_id)
        .await
        .map_err(|source| BackupArtifactSourceLoadError::Infrastructure {
            operation: "list plugin artifact backup inventory",
            source,
        })?;
    let extensions = repository
        .list_extension_installations_for_node(node_id)
        .await
        .map_err(|source| BackupArtifactSourceLoadError::Infrastructure {
            operation: "list extension backup inventory",
            source,
        })?;
    let entries = build_backup_artifact_inventory(node_id, installations, instances, extensions)?;
    for entry in &entries {
        validate_retained_artifact(entry).await?;
    }
    Ok(entries
        .into_iter()
        .map(|entry| Arc::new(entry) as Arc<dyn BackupComponentSource>)
        .collect())
}

fn requires_recovery_artifact(installation: &domain::PluginInstallationRecord) -> bool {
    matches!(
        installation.desired_state,
        domain::PluginDesiredState::ActiveRequested | domain::PluginDesiredState::PendingRestart
    )
}

fn plugin_artifact_identity(installation: &domain::PluginInstallationRecord) -> String {
    format!(
        "plugin:{}/{}/{}@{}",
        installation.category.as_str(),
        installation.organization,
        installation.plugin_id,
        installation.plugin_version
    )
}

fn inventory_error(
    reason: BackupArtifactInventoryReason,
    installation_id: uuid::Uuid,
    artifact_identity: impl Into<String>,
) -> BackupArtifactInventoryError {
    BackupArtifactInventoryError {
        reason,
        installation_id,
        artifact_identity: artifact_identity.into(),
    }
}

fn classify_source(source_kind: &str) -> BackupArtifactDisposition {
    match source_kind {
        "builtin"
        | "official"
        | "official_registry"
        | "official_repository"
        // A configured proxy only changes the transport route to an official catalog. The
        // verified package identity remains independently rebuildable after restore.
        | "configured_proxy"
        | "mirror_registry" => BackupArtifactDisposition::RebuildableIdentity,
        _ => BackupArtifactDisposition::Embedded,
    }
}

fn validate_rebuildable_identity(
    installation: &domain::PluginInstallationRecord,
    artifact_identity: &str,
) -> std::result::Result<(), BackupArtifactInventoryError> {
    if installation.organization.trim().is_empty()
        || installation.plugin_id.trim().is_empty()
        || installation.plugin_version.trim().is_empty()
        || installation.verification_status != domain::PluginVerificationStatus::Valid
    {
        return Err(inventory_error(
            BackupArtifactInventoryReason::RebuildableIdentityInvalid,
            installation.id,
            artifact_identity,
        ));
    }
    Ok(())
}

fn insert_unique(
    inventory: &mut BTreeMap<String, BackupArtifactEntry>,
    entry: BackupArtifactEntry,
) -> std::result::Result<(), BackupArtifactInventoryError> {
    if let Some(existing) = inventory.insert(entry.identity.clone(), entry) {
        return Err(inventory_error(
            BackupArtifactInventoryReason::DuplicateArtifactIdentity,
            existing.installation_id,
            existing.identity,
        ));
    }
    Ok(())
}

async fn validate_retained_artifact(
    entry: &BackupArtifactEntry,
) -> std::result::Result<(), BackupArtifactSourceLoadError> {
    if entry.disposition != BackupArtifactDisposition::Embedded {
        return Ok(());
    }
    let path = entry
        .artifact_path
        .as_deref()
        .expect("embedded entries have an artifact path");
    let metadata = tokio::fs::metadata(path).await.map_err(|source| {
        if source.kind() == ErrorKind::NotFound {
            BackupArtifactSourceLoadError::Inventory(inventory_error(
                BackupArtifactInventoryReason::RetainedArtifactMissing,
                entry.installation_id,
                &entry.identity,
            ))
        } else {
            BackupArtifactSourceLoadError::Infrastructure {
                operation: "inspect retained backup artifact",
                source: source.into(),
            }
        }
    })?;
    if !metadata.is_file() {
        return Err(BackupArtifactSourceLoadError::Inventory(inventory_error(
            BackupArtifactInventoryReason::RetainedArtifactNotFile,
            entry.installation_id,
            &entry.identity,
        )));
    }
    tokio::fs::File::open(path).await.map_err(|source| {
        if source.kind() == ErrorKind::NotFound {
            BackupArtifactSourceLoadError::Inventory(inventory_error(
                BackupArtifactInventoryReason::RetainedArtifactMissing,
                entry.installation_id,
                &entry.identity,
            ))
        } else {
            BackupArtifactSourceLoadError::Infrastructure {
                operation: "open retained backup artifact",
                source: source.into(),
            }
        }
    })?;
    Ok(())
}

/// Streams one non-rebuildable artifact into the backup envelope and proves that the source did
/// not change while it was read. Directories are rejected: uploaded plugin packages must use the
/// retained package archive; silently archiving a mutable installation tree would not be safe.
pub async fn export_backup_artifact<W>(
    entry: &BackupArtifactEntry,
    writer: &mut W,
) -> Result<ExportedArtifact>
where
    W: AsyncWrite + Unpin + Send,
{
    if entry.disposition != BackupArtifactDisposition::Embedded {
        bail!("identity-only artifact has no payload");
    }
    let path = entry
        .artifact_path
        .as_deref()
        .context("non-rebuildable artifact path is missing")?;
    let initial = fs::metadata(path)
        .await
        .context("backup artifact is missing or unreadable")?;
    if !initial.is_file() {
        bail!("non-rebuildable artifact must be a retained package file");
    }
    let mut file = fs::File::open(path)
        .await
        .context("failed to open backup artifact")?;
    let mut buffer = vec![0_u8; ARTIFACT_STREAM_BUFFER_BYTES];
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .context("failed to read backup artifact")?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .await
            .context("failed to write backup artifact")?;
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes
            .checked_add(read as u64)
            .context("backup artifact size overflow")?;
    }
    writer
        .flush()
        .await
        .context("failed to flush backup artifact")?;
    let final_metadata = fs::metadata(path)
        .await
        .context("backup artifact disappeared while reading")?;
    if initial.len() != final_metadata.len()
        || initial.modified().ok() != final_metadata.modified().ok()
        || size_bytes != initial.len()
    {
        bail!("backup artifact changed while reading");
    }
    let sha256 = format!("sha256:{:x}", hasher.finalize());
    if let Some(expected) = entry.expected_checksum.as_deref() {
        if normalize_checksum(expected) != sha256 {
            bail!("backup artifact checksum mismatch");
        }
    }
    Ok(ExportedArtifact {
        size_bytes,
        sha256,
        content_type: "application/vnd.1flowbase.extension-artifact",
    })
}

fn normalize_checksum(value: &str) -> String {
    if value.starts_with("sha256:") {
        value.to_ascii_lowercase()
    } else {
        format!("sha256:{}", value.to_ascii_lowercase())
    }
}

pub fn backup_artifact_component_id(entry: &BackupArtifactEntry) -> String {
    let mut hasher = Sha256::new();
    hasher.update(entry.identity.as_bytes());
    format!("artifact-{:x}", hasher.finalize())
}

#[async_trait]
impl BackupComponentSource for BackupArtifactEntry {
    fn descriptor(&self) -> BackupComponentDescriptor {
        let kind = match self.kind {
            BackupArtifactKind::Mcp => domain::BackupComponentKind::McpArtifact,
            BackupArtifactKind::Extension | BackupArtifactKind::HostExtension => {
                domain::BackupComponentKind::ExtensionArtifact
            }
        };
        let (disposition, rebuildability) = match self.disposition {
            BackupArtifactDisposition::RebuildableIdentity => (
                domain::BackupComponentDisposition::IdentityOnly,
                domain::ArtifactRebuildability::Rebuildable,
            ),
            BackupArtifactDisposition::Embedded => (
                domain::BackupComponentDisposition::Embedded,
                domain::ArtifactRebuildability::NonRebuildable,
            ),
        };
        BackupComponentDescriptor {
            component_id: domain::BackupComponentId::try_from(backup_artifact_component_id(self))
                .expect("sha256-derived backup component id"),
            kind,
            source_identity: domain::BackupSourceIdentity::try_from(self.identity.clone())
                .expect("validated artifact backup identity"),
            content_type: "application/vnd.1flowbase.extension-artifact".to_string(),
            disposition,
            rebuildability,
            restore_target: domain::BackupComponentRestoreTarget::Artifact {
                category: self.category.clone(),
                organization: self.organization.clone(),
                artifact_id: self.artifact_id.clone(),
                version: self.version.clone(),
            },
        }
    }

    async fn write_to(
        &self,
        mut destination: BackupComponentWriter,
    ) -> Result<(), BackupSourceError> {
        export_backup_artifact(self, &mut destination)
            .await
            .map(|_| ())
            .map_err(|_| BackupSourceError::Changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_matrix_only_marks_proven_registry_sources_rebuildable() {
        for source in [
            "builtin",
            "official",
            "official_registry",
            "official_repository",
            "configured_proxy",
            "mirror_registry",
        ] {
            assert_eq!(
                classify_source(source),
                BackupArtifactDisposition::RebuildableIdentity
            );
        }
        for source in ["uploaded", "dropin", "local", "unknown"] {
            assert_eq!(classify_source(source), BackupArtifactDisposition::Embedded);
        }
    }

    #[test]
    fn duplicate_identity_is_rejected() {
        let entry = BackupArtifactEntry {
            installation_id: uuid::Uuid::from_u128(1),
            identity: "extension:mcp/acme/example@1.0.0".to_string(),
            kind: BackupArtifactKind::Mcp,
            category: "mcp".to_string(),
            organization: "acme".to_string(),
            artifact_id: "example".to_string(),
            source_kind: "uploaded".to_string(),
            version: "1.0.0".to_string(),
            expected_checksum: None,
            disposition: BackupArtifactDisposition::Embedded,
            artifact_path: Some(PathBuf::from("/tmp/example.1flowbasepkg")),
        };
        let mut inventory = BTreeMap::new();
        insert_unique(&mut inventory, entry.clone()).unwrap();
        assert!(insert_unique(&mut inventory, entry).is_err());
    }

    #[tokio::test]
    async fn missing_retained_package_is_an_inventory_conflict_before_backup_streaming() {
        let entry = BackupArtifactEntry {
            installation_id: uuid::Uuid::from_u128(9),
            identity: "plugin:runtime-extensions/acme/missing@1.0.0".to_owned(),
            kind: BackupArtifactKind::Extension,
            category: "runtime-extensions".to_owned(),
            organization: "acme".to_owned(),
            artifact_id: "missing".to_owned(),
            source_kind: "uploaded".to_owned(),
            version: "1.0.0".to_owned(),
            expected_checksum: None,
            disposition: BackupArtifactDisposition::Embedded,
            artifact_path: Some(PathBuf::from(
                "/tmp/1flowbase-backup-artifact-does-not-exist",
            )),
        };

        let error = validate_retained_artifact(&entry)
            .await
            .unwrap_err()
            .into_inventory_error()
            .expect("missing retained package is a backup inventory conflict");

        assert_eq!(
            error.reason,
            BackupArtifactInventoryReason::RetainedArtifactMissing
        );
        assert_eq!(error.installation_id, entry.installation_id);
        assert_eq!(error.artifact_identity, entry.identity);
    }
}
