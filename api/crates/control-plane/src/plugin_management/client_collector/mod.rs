use super::ExtensionInstallationService;
use crate::{errors::ControlPlaneError, ports::ExtensionInstallationRepository};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
};
use tokio::io::DuplexStream;

pub fn inspect_client_collector_archive<R: Read>(
    reader: R,
) -> Result<domain::ClientCollectorManifest> {
    let decoder = flate2::read::MultiGzDecoder::new(reader);
    let mut archive = tar::Archive::new(decoder);
    archive.set_ignore_zeros(true);
    let mut members = BTreeMap::new();
    let mut manifest = None;
    let mut release = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let name = path
            .to_str()
            .context("collector asset path is not UTF-8")?
            .to_owned();
        if !domain::client_collector_flat_name(&name)
            || !entry.header().entry_type().is_file()
            || members.contains_key(&name)
        {
            bail!("collector archive contains an invalid or duplicate member");
        }
        let declared_size = entry.size();
        let mut hash = Sha256::new();
        let mut size = 0u64;
        let mut manifest_bytes = Vec::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hash.update(&buffer[..read]);
            size = size
                .checked_add(read as u64)
                .context("collector asset size overflow")?;
            if name == "collector-manifest.json" || name == "release.json" {
                manifest_bytes.extend_from_slice(&buffer[..read]);
            }
        }
        if size != declared_size {
            bail!("collector asset size mismatch");
        }
        if name == "collector-manifest.json" {
            manifest = Some(serde_json::from_slice::<domain::ClientCollectorManifest>(
                &manifest_bytes,
            )?);
        }
        if name == "release.json" {
            release = Some(serde_json::from_slice::<
                domain::ClientCollectorReleaseManifest,
            >(&manifest_bytes)?);
        }
        members.insert(name, (format!("{:x}", hash.finalize()), size));
    }
    // Consume the gzip trailer as well, so a truncated/corrupt stream cannot validate.
    std::io::copy(&mut archive.into_inner(), &mut std::io::sink())?;
    let manifest = manifest.context("collector manifest is missing")?;
    validate_client_collector_manifest(&manifest)?;
    members.remove("collector-manifest.json");
    for asset in &manifest.assets {
        let actual = members
            .remove(&asset.name)
            .context("collector asset is missing or duplicated")?;
        if actual != (asset.sha256.clone(), asset.size) {
            bail!("collector asset digest or size mismatch");
        }
    }
    if !members.is_empty() {
        bail!("collector archive contains undeclared members");
    }
    validate_client_collector_release(
        &manifest,
        &release.context("collector release manifest missing")?,
    )?;
    Ok(manifest)
}

fn validate_client_collector_manifest(manifest: &domain::ClientCollectorManifest) -> Result<()> {
    if manifest.schema_version != "1flowbase.client-collector/v1"
        || manifest.execution_target != "client"
        || manifest.protocol_version != "1flowbase.agent-logs/v1"
        || !domain::client_collector_flat_name(&manifest.organization)
        || !domain::client_collector_flat_name(&manifest.artifact_id)
        || !domain::client_collector_flat_name(&manifest.version)
        || !domain::client_collector_flat_name(&manifest.entry)
        || manifest.collector_code.is_empty()
        || manifest.source_client.is_empty()
        || manifest.display_name.is_empty()
        || !manifest.description.contains_key("en_US")
        || !manifest.description.contains_key("zh_Hans")
        || semver::Version::parse(&manifest.version).is_err()
        || semver::Version::parse(&manifest.minimum_host_version).is_err()
    {
        bail!("invalid client collector manifest");
    }
    let mut names = std::collections::BTreeSet::new();
    for asset in &manifest.assets {
        if !domain::client_collector_flat_name(&asset.name)
            || asset.name == "collector-manifest.json"
            || !names.insert(asset.name.clone())
            || asset.sha256.len() != 64
            || !asset
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("invalid client collector asset manifest");
        }
    }
    for required in [
        "install.sh",
        "install.ps1",
        "README.md",
        "README.en.md",
        "release.json",
        "checksums.txt",
        "checksums.txt.sig",
        "signing-public-key.pem",
    ] {
        if !names.contains(&required.to_string()) {
            bail!("required collector asset missing: {required}");
        }
    }
    Ok(())
}

fn validate_client_collector_release(
    manifest: &domain::ClientCollectorManifest,
    release: &domain::ClientCollectorReleaseManifest,
) -> Result<()> {
    if release.schema_version != "1flowbase.client-collector-release/v1"
        || release.collector_code != manifest.collector_code
        || release.version != manifest.version
        || release.source_sha.is_empty()
        || release.signature_algorithm != "ed25519"
        || release.signing_key_id.is_empty()
        || release.artifacts.is_empty()
    {
        bail!("invalid collector release identity or metadata");
    }
    let mut names = std::collections::BTreeSet::new();
    let mut platforms = std::collections::BTreeSet::new();
    for artifact in &release.artifacts {
        if !domain::client_collector_flat_name(&artifact.os)
            || !domain::client_collector_flat_name(&artifact.arch)
            || !domain::client_collector_flat_name(&artifact.rust_target)
            || !names.insert(&artifact.name)
            || !platforms.insert((&artifact.os, &artifact.arch))
            || !(artifact.name.ends_with(".tar.gz") || artifact.name.ends_with(".zip"))
        {
            bail!("invalid or duplicate native collector release artifact");
        }
        let asset = manifest
            .assets
            .iter()
            .find(|asset| asset.name == artifact.name)
            .context("native collector release asset missing from package manifest")?;
        if asset.sha256 != artifact.sha256 || asset.size != artifact.size {
            bail!("native collector release asset integrity differs from package manifest");
        }
    }
    Ok(())
}

pub fn ensure_client_collector_identity(
    manifest: &domain::ClientCollectorManifest,
    identity: &domain::ExtensionInstallationIdentity,
) -> Result<()> {
    if identity.category != domain::ExtensionCategory::RuntimeExtensions
        || manifest.organization != identity.organization
        || manifest.artifact_id != identity.artifact_id
        || manifest.version != identity.version
    {
        return Err(ControlPlaneError::InvalidInput("client_collector_identity").into());
    }
    Ok(())
}

pub struct ClientCollectorDownload {
    pub reader: DuplexStream,
    pub size: u64,
    pub name: String,
    pub completion: tokio::sync::oneshot::Receiver<std::io::Result<()>>,
}

impl<R: ExtensionInstallationRepository> ExtensionInstallationService<R> {
    /// Validates the current-node retained archive and its installation integrity, without remote repair.
    pub async fn client_collector_manifest(
        &self,
        record: &domain::ExtensionInstallationRecord,
    ) -> Result<domain::ClientCollectorManifest> {
        let record = record.clone();
        let install_root = self.install_root.clone();
        tokio::task::spawn_blocking(move || {
            let (_, manifest) = open_validated_collector(&record, &install_root)?;
            Ok(manifest)
        })
        .await?
    }
    pub async fn download_client_collector(
        &self,
        node_id: &str,
        identity: &domain::ExtensionInstallationIdentity,
        asset: &str,
    ) -> Result<ClientCollectorDownload> {
        if !domain::client_collector_flat_name(asset) {
            return Err(ControlPlaneError::NotFound("client_collector_asset").into());
        }
        let record = self
            .repository
            .find_extension_installation(node_id, identity)
            .await?
            .filter(|record| {
                record.node_id == node_id
                    && record.status == domain::ExtensionInstallationStatus::Installed
            })
            .ok_or(ControlPlaneError::NotFound("client_collector_installation"))?;
        let asset = asset.to_owned();
        let install_root = self.install_root.clone();
        let (file, manifest) =
            tokio::task::spawn_blocking(move || open_validated_collector(&record, &install_root))
                .await?
                .map_err(|_| {
                    ControlPlaneError::Conflict("client_collector_artifact_unavailable")
                })?;
        let size = manifest
            .assets
            .iter()
            .find(|member| member.name == asset)
            .ok_or(ControlPlaneError::NotFound("client_collector_asset"))?
            .size;
        let (reader, mut writer) = tokio::io::duplex(64 * 1024);
        let name = asset.clone();
        let worker = tokio::task::spawn_blocking(move || -> Result<()> {
            let mut archive = tar::Archive::new(flate2::read::MultiGzDecoder::new(file));
            for member in archive.entries()? {
                let mut member = member?;
                if member.path()?.to_str() == Some(asset.as_str()) {
                    let handle = tokio::runtime::Handle::current();
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        let read = member.read(&mut buffer)?;
                        if read == 0 {
                            break;
                        }
                        handle.block_on(tokio::io::AsyncWriteExt::write_all(
                            &mut writer,
                            &buffer[..read],
                        ))?;
                    }
                    return Ok(());
                }
            }
            bail!("validated collector member disappeared")
        });
        let (completed, completion) = tokio::sync::oneshot::channel();
        // This owner always observes the blocking worker, including disconnected HTTP readers.
        tokio::spawn(async move {
            let result = match worker.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(std::io::Error::other(error.to_string())),
                Err(error) => Err(std::io::Error::other(error.to_string())),
            };
            let _ = completed.send(result);
        });
        Ok(ClientCollectorDownload {
            reader,
            size,
            name,
            completion,
        })
    }
}
fn open_validated_collector(
    record: &domain::ExtensionInstallationRecord,
    install_root: &std::path::Path,
) -> Result<(File, domain::ClientCollectorManifest)> {
    if record.status != domain::ExtensionInstallationStatus::Installed
        || !domain::is_client_collector_receipt(&record.receipt)
    {
        bail!("collector is not installed");
    }
    let path = record
        .local_path
        .as_deref()
        .context("collector local path missing")?;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        bail!("collector archive is a symlink");
    }
    let retained = std::fs::canonicalize(path)?;
    let root = std::fs::canonicalize(install_root)?;
    if !retained.starts_with(&root) || retained == root {
        bail!("collector archive escaped retained install root");
    }
    let mut file = File::open(retained)?;
    if !file.metadata()?.is_file() {
        bail!("collector archive is not a regular file");
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let checksum = format!("sha256:{:x}", hash.finalize());
    if record.local_checksum.as_deref() != Some(checksum.as_str())
        || record
            .expected_checksum
            .as_deref()
            .is_some_and(|expected| expected != checksum)
    {
        bail!("collector local integrity mismatch");
    }
    file.seek(SeekFrom::Start(0))?;
    let manifest = inspect_client_collector_archive(&mut file)?;
    ensure_client_collector_identity(&manifest, &record.identity)?;
    file.seek(SeekFrom::Start(0))?;
    Ok((file, manifest))
}

#[cfg(test)]
pub(crate) mod _tests;
