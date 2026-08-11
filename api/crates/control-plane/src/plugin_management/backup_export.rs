use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
};

const ARTIFACT_STREAM_BUFFER_BYTES: usize = 256 * 1024;

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
    pub identity: String,
    pub kind: BackupArtifactKind,
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
) -> Result<Vec<BackupArtifactEntry>> {
    let mut instances = BTreeMap::new();
    for instance in plugin_instances
        .into_iter()
        .filter(|instance| instance.node_id == node_id && instance.is_current)
    {
        if instances
            .insert(instance.installation_id, instance)
            .is_some()
        {
            bail!("duplicate current plugin artifact instance");
        }
    }

    let mut inventory = BTreeMap::<String, BackupArtifactEntry>::new();
    for installation in plugin_installations {
        let Some(instance) = instances.remove(&installation.id) else {
            bail!("plugin artifact instance is missing");
        };
        let identity = format!(
            "plugin:{}/{}/{}@{}",
            installation.category.as_str(),
            installation.organization,
            installation.plugin_id,
            installation.plugin_version
        );
        let disposition = classify_source(&installation.source_kind);
        let artifact_path = match disposition {
            BackupArtifactDisposition::RebuildableIdentity => None,
            BackupArtifactDisposition::Embedded => instance
                .package_path
                .as_deref()
                .or(instance.local_path.as_deref())
                .map(PathBuf::from),
        };
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
                identity,
                kind,
                source_kind: installation.source_kind,
                version: installation.plugin_version,
                expected_checksum: instance.local_checksum.or(installation.expected_checksum),
                disposition,
                artifact_path,
            },
        )?;
    }
    if !instances.is_empty() {
        bail!("orphan current plugin artifact instance");
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
                identity,
                kind,
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

    for entry in inventory.values() {
        if entry.disposition == BackupArtifactDisposition::Embedded && entry.artifact_path.is_none()
        {
            bail!("non-rebuildable artifact path is missing");
        }
    }
    Ok(inventory.into_values().collect())
}

fn classify_source(source_kind: &str) -> BackupArtifactDisposition {
    match source_kind {
        "builtin"
        | "official"
        | "official_registry"
        | "official_repository"
        | "mirror_registry" => BackupArtifactDisposition::RebuildableIdentity,
        _ => BackupArtifactDisposition::Embedded,
    }
}

fn insert_unique(
    inventory: &mut BTreeMap<String, BackupArtifactEntry>,
    entry: BackupArtifactEntry,
) -> Result<()> {
    if inventory.insert(entry.identity.clone(), entry).is_some() {
        bail!("duplicate backup artifact identity");
    }
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
            identity: "extension:mcp/acme/example@1.0.0".to_string(),
            kind: BackupArtifactKind::Mcp,
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
}
