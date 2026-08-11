use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use domain::{
    ArtifactRebuildability, BackupComponent, BackupComponentDisposition, BackupComponentKind,
    BackupComponentRestoreTarget, BackupSourceIdentity, ContentDigest, ExtensionCategory,
};
use sha2::{Digest, Sha256};
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::{
    ports::BackupComponentReader,
    system_recovery::{RecoveryStepContext, RecoveryStepTarget, RecoveryStepTargetError},
};

const ARTIFACT_RESTORE_BUFFER_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArtifactRecoveryCoordinate {
    pub category: String,
    pub organization: String,
    pub artifact_id: String,
    pub version: String,
    pub source_identity: BackupSourceIdentity,
}

#[async_trait]
pub trait ArtifactRecoveryResolver: Send + Sync {
    /// Resolves the operator-owned local destination for an embedded artifact.
    async fn embedded_target(
        &self,
        coordinate: &ArtifactRecoveryCoordinate,
    ) -> Result<PathBuf, RecoveryStepTargetError>;

    /// Proves a rebuildable official/builtin identity already exists in the local inventory.
    /// Implementations must not fetch or download during offline recovery.
    async fn verify_rebuildable(
        &self,
        coordinate: &ArtifactRecoveryCoordinate,
    ) -> Result<(), RecoveryStepTargetError>;
}

pub struct FilesystemArtifactRecoveryTarget {
    resolver: Arc<dyn ArtifactRecoveryResolver>,
}

impl FilesystemArtifactRecoveryTarget {
    pub fn new(resolver: Arc<dyn ArtifactRecoveryResolver>) -> Self {
        Self { resolver }
    }

    async fn stage_embedded(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        mut plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        let coordinate = artifact_coordinate(component)?;
        let target = self.resolver.embedded_target(&coordinate).await?;
        validate_target_path(&target)?;
        let paths = artifact_paths(context, component, &target)?;
        let parent = target
            .parent()
            .ok_or(RecoveryStepTargetError::InvalidTarget)?;
        fs::create_dir_all(parent)
            .await
            .map_err(|_| RecoveryStepTargetError::Unavailable)?;
        remove_file_if_exists(&paths.staging).await?;
        let mut staging = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&paths.staging)
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?;
        let mut remaining = component.size_bytes;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; ARTIFACT_RESTORE_BUFFER_BYTES];
        while remaining > 0 {
            let limit = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| RecoveryStepTargetError::Integrity)?;
            let read = plaintext
                .read(&mut buffer[..limit])
                .await
                .map_err(|_| RecoveryStepTargetError::Staging)?;
            if read == 0 {
                return Err(RecoveryStepTargetError::Integrity);
            }
            staging
                .write_all(&buffer[..read])
                .await
                .map_err(|_| RecoveryStepTargetError::Staging)?;
            hasher.update(&buffer[..read]);
            remaining -= read as u64;
        }
        let mut extra = [0_u8; 1];
        if plaintext
            .read(&mut extra)
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?
            != 0
        {
            return Err(RecoveryStepTargetError::Integrity);
        }
        staging
            .flush()
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?;
        staging
            .sync_all()
            .await
            .map_err(|_| RecoveryStepTargetError::Staging)?;
        drop(staging);
        let digest = digest(hasher.finalize())?;
        if digest != component.content_digest {
            let _ = remove_file_if_exists(&paths.staging).await;
            return Err(RecoveryStepTargetError::Integrity);
        }
        Ok(())
    }

    async fn prepare_rollback(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        let target = self.embedded_target(component).await?;
        let paths = artifact_paths(context, component, &target)?;
        let rollback_exists = regular_file_exists(&paths.rollback).await?;
        let absent_exists = regular_file_exists(&paths.absent_marker).await?;
        if rollback_exists && absent_exists {
            return Err(RecoveryStepTargetError::Integrity);
        }
        if rollback_exists || absent_exists {
            return Ok(());
        }
        if regular_file_exists(&target).await? {
            fs::rename(&target, &paths.rollback)
                .await
                .map_err(|_| RecoveryStepTargetError::Promotion)?;
        } else {
            let marker = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&paths.absent_marker)
                .await
                .map_err(|_| RecoveryStepTargetError::Promotion)?;
            marker
                .sync_all()
                .await
                .map_err(|_| RecoveryStepTargetError::Promotion)?;
        }
        Ok(())
    }

    async fn promote_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        let target = self.embedded_target(component).await?;
        let paths = artifact_paths(context, component, &target)?;
        remove_file_if_exists(&target).await?;
        fs::rename(&paths.staging, &target)
            .await
            .map_err(|_| RecoveryStepTargetError::Promotion)?;
        let receipt = file_receipt(&target).await?;
        if receipt.size_bytes != component.size_bytes
            || receipt.content_digest != component.content_digest
        {
            return Err(RecoveryStepTargetError::Integrity);
        }
        Ok(())
    }

    async fn rollback_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        if component.disposition == BackupComponentDisposition::IdentityOnly {
            return Ok(());
        }
        let target = self.embedded_target(component).await?;
        let paths = artifact_paths(context, component, &target)?;
        let rollback_exists = regular_file_exists(&paths.rollback).await?;
        let absent_exists = regular_file_exists(&paths.absent_marker).await?;
        if rollback_exists && absent_exists {
            return Err(RecoveryStepTargetError::Compensation);
        }
        if rollback_exists {
            let expected = file_receipt(&paths.rollback).await?;
            remove_file_if_exists(&target).await?;
            fs::rename(&paths.rollback, &target)
                .await
                .map_err(|_| RecoveryStepTargetError::Compensation)?;
            if file_receipt(&target).await? != expected {
                return Err(RecoveryStepTargetError::Compensation);
            }
        } else if absent_exists {
            remove_file_if_exists(&target).await?;
            remove_file_if_exists(&paths.absent_marker).await?;
        }
        remove_file_if_exists(&paths.staging).await?;
        Ok(())
    }

    async fn finalize_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        if component.disposition == BackupComponentDisposition::IdentityOnly {
            return Ok(());
        }
        let target = self.embedded_target(component).await?;
        let paths = artifact_paths(context, component, &target)?;
        for path in [paths.staging, paths.rollback, paths.absent_marker] {
            remove_file_if_exists(&path).await?;
        }
        Ok(())
    }

    async fn embedded_target(
        &self,
        component: &BackupComponent,
    ) -> Result<PathBuf, RecoveryStepTargetError> {
        let coordinate = artifact_coordinate(component)?;
        let target = self.resolver.embedded_target(&coordinate).await?;
        validate_target_path(&target)?;
        Ok(target)
    }
}

#[async_trait]
impl RecoveryStepTarget for FilesystemArtifactRecoveryTarget {
    async fn begin(
        &self,
        _context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_artifact_components(components)?;
        let mut targets = BTreeSet::new();
        for component in components
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
        {
            if !targets.insert(self.embedded_target(component).await?) {
                return Err(RecoveryStepTargetError::InvalidTarget);
            }
        }
        Ok(())
    }

    async fn stage_component(
        &self,
        context: &RecoveryStepContext,
        component: &BackupComponent,
        plaintext: BackupComponentReader,
    ) -> Result<(), RecoveryStepTargetError> {
        validate_artifact_components(std::slice::from_ref(component))?;
        if component.disposition != BackupComponentDisposition::Embedded {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
        self.stage_embedded(context, component, plaintext).await
    }

    async fn stage_identity(
        &self,
        _context: &RecoveryStepContext,
        component: &BackupComponent,
    ) -> Result<(), RecoveryStepTargetError> {
        validate_artifact_components(std::slice::from_ref(component))?;
        if component.disposition != BackupComponentDisposition::IdentityOnly
            || component.rebuildability != ArtifactRebuildability::Rebuildable
            || digest(Sha256::digest(
                component.source_identity.as_str().as_bytes(),
            ))? != component.content_digest
        {
            return Err(RecoveryStepTargetError::Integrity);
        }
        let coordinate = artifact_coordinate(component)?;
        self.resolver.verify_rebuildable(&coordinate).await
    }

    async fn promote(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        validate_artifact_components(components)?;
        let embedded = components
            .iter()
            .filter(|component| component.disposition == BackupComponentDisposition::Embedded)
            .collect::<Vec<_>>();
        for component in &embedded {
            self.prepare_rollback(context, component).await?;
        }
        for component in embedded {
            self.promote_component(context, component).await?;
        }
        Ok(())
    }

    async fn rollback(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        let mut failed = false;
        for component in components.iter().rev() {
            if self.rollback_component(context, component).await.is_err() {
                failed = true;
            }
        }
        if failed {
            Err(RecoveryStepTargetError::Compensation)
        } else {
            Ok(())
        }
    }

    async fn finalize(
        &self,
        context: &RecoveryStepContext,
        components: &[BackupComponent],
    ) -> Result<(), RecoveryStepTargetError> {
        let mut failed = false;
        for component in components {
            if self.finalize_component(context, component).await.is_err() {
                failed = true;
            }
        }
        if failed {
            Err(RecoveryStepTargetError::Unavailable)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArtifactFileReceipt {
    size_bytes: u64,
    content_digest: ContentDigest,
}

struct ArtifactRecoveryPaths {
    staging: PathBuf,
    rollback: PathBuf,
    absent_marker: PathBuf,
}

fn validate_artifact_components(
    components: &[BackupComponent],
) -> Result<(), RecoveryStepTargetError> {
    let mut coordinates = BTreeSet::new();
    for component in components {
        if !matches!(
            component.kind,
            BackupComponentKind::ExtensionArtifact | BackupComponentKind::McpArtifact
        ) {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
        let coordinate = artifact_coordinate(component)?;
        let category = ExtensionCategory::parse(&coordinate.category)
            .ok_or(RecoveryStepTargetError::InvalidTarget)?;
        if (component.kind == BackupComponentKind::McpArtifact
            && category != ExtensionCategory::Mcp)
            || (component.kind == BackupComponentKind::ExtensionArtifact
                && category == ExtensionCategory::Mcp)
            || !coordinates.insert(coordinate)
        {
            return Err(RecoveryStepTargetError::InvalidTarget);
        }
    }
    Ok(())
}

fn artifact_coordinate(
    component: &BackupComponent,
) -> Result<ArtifactRecoveryCoordinate, RecoveryStepTargetError> {
    let BackupComponentRestoreTarget::Artifact {
        category,
        organization,
        artifact_id,
        version,
    } = &component.restore_target
    else {
        return Err(RecoveryStepTargetError::InvalidTarget);
    };
    if [organization, artifact_id, version]
        .into_iter()
        .any(|segment| !valid_path_segment(segment))
    {
        return Err(RecoveryStepTargetError::InvalidTarget);
    }
    Ok(ArtifactRecoveryCoordinate {
        category: category.clone(),
        organization: organization.clone(),
        artifact_id: artifact_id.clone(),
        version: version.clone(),
        source_identity: component.source_identity.clone(),
    })
}

fn valid_path_segment(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && !matches!(value, "." | "..")
        && !value.contains([':', '/', '\\', '\0'])
}

fn validate_target_path(target: &Path) -> Result<(), RecoveryStepTargetError> {
    if !target.is_absolute() || target.file_name().is_none() {
        return Err(RecoveryStepTargetError::InvalidTarget);
    }
    Ok(())
}

fn artifact_paths(
    context: &RecoveryStepContext,
    component: &BackupComponent,
    target: &Path,
) -> Result<ArtifactRecoveryPaths, RecoveryStepTargetError> {
    let parent = target
        .parent()
        .ok_or(RecoveryStepTargetError::InvalidTarget)?;
    let mut hasher = Sha256::new();
    hasher.update(component.component_id.as_str().as_bytes());
    hasher.update(target.as_os_str().as_encoded_bytes());
    let identity = format!("{:x}", hasher.finalize());
    let prefix = format!(
        ".1flowbase-recovery-{}-{identity}",
        context.recovery_job_id.as_uuid()
    );
    Ok(ArtifactRecoveryPaths {
        staging: parent.join(format!("{prefix}.staging")),
        rollback: parent.join(format!("{prefix}.rollback")),
        absent_marker: parent.join(format!("{prefix}.absent")),
    })
}

async fn file_receipt(path: &Path) -> Result<ArtifactFileReceipt, RecoveryStepTargetError> {
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(|_| RecoveryStepTargetError::Unavailable)?;
    if !metadata.file_type().is_file() {
        return Err(RecoveryStepTargetError::Integrity);
    }
    let mut file = fs::File::open(path)
        .await
        .map_err(|_| RecoveryStepTargetError::Unavailable)?;
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = vec![0_u8; ARTIFACT_RESTORE_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|_| RecoveryStepTargetError::Unavailable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes = size_bytes
            .checked_add(read as u64)
            .ok_or(RecoveryStepTargetError::Integrity)?;
    }
    if size_bytes != metadata.len() {
        return Err(RecoveryStepTargetError::Integrity);
    }
    Ok(ArtifactFileReceipt {
        size_bytes,
        content_digest: digest(hasher.finalize())?,
    })
}

async fn regular_file_exists(path: &Path) -> Result<bool, RecoveryStepTargetError> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err(RecoveryStepTargetError::Integrity),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(RecoveryStepTargetError::Unavailable),
    }
}

async fn remove_file_if_exists(path: &Path) -> Result<(), RecoveryStepTargetError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(RecoveryStepTargetError::Unavailable),
    }
}

fn digest(value: sha2::digest::Output<Sha256>) -> Result<ContentDigest, RecoveryStepTargetError> {
    ContentDigest::try_from(format!("{value:x}")).map_err(|_| RecoveryStepTargetError::Integrity)
}
