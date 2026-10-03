//! Versioned application-template state. Resource owners still perform the actual writes.
use super::{PortableTemplatePackage, PortableTemplateRelease};
use crate::errors::ControlPlaneError;
use anyhow::{ensure, Result};
use control_plane_contracts::ports::{
    ApplicationTemplateReleaseRecord, PortableTemplateIdentityRepository,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn application_template_checksum(package: &PortableTemplatePackage) -> Result<String> {
    // Typed serialization includes release metadata; whitespace of the source file is irrelevant.
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(package)?)
    ))
}

pub fn validate_application_template_release(release: &PortableTemplateRelease) -> Result<()> {
    ensure!(
        !release.template_id.trim().is_empty(),
        "application_template_id"
    );
    ensure!(
        release.release_version > 0 && release.release_version <= i64::MAX as u64,
        "application_template_release_version"
    );
    ensure!(!release.name.trim().is_empty(), "application_template_name");
    ensure!(
        !release.exported_from_system_version.trim().is_empty()
            && !release.exported_at.trim().is_empty(),
        "application_template_export_metadata"
    );
    Ok(())
}

pub fn application_template_needs_install(
    release: &PortableTemplateRelease,
    checksum: &str,
    records: &[ApplicationTemplateReleaseRecord],
) -> Result<bool> {
    validate_application_template_release(release)?;
    if let Some(existing) = records
        .iter()
        .find(|r| r.release_version == release.release_version)
    {
        if existing.checksum != checksum {
            return Err(
                ControlPlaneError::Conflict("application_template_immutable_release").into(),
            );
        }
        if existing.successful {
            return Ok(false);
        }
    }
    if records
        .iter()
        .any(|r| r.successful && r.release_version > release.release_version)
    {
        return Err(ControlPlaneError::Conflict("application_template_release_downgrade").into());
    }
    Ok(true)
}

pub async fn prepare_application_template_release<R: PortableTemplateIdentityRepository>(
    repository: &R,
    workspace_id: Uuid,
    package: &PortableTemplatePackage,
) -> Result<bool> {
    let Some(release) = &package.release else {
        return Ok(true);
    };
    let checksum = application_template_checksum(package)?;
    let records = repository
        .load_application_template_releases(workspace_id, &release.template_id)
        .await?;
    if !application_template_needs_install(release, &checksum, &records)? {
        return Ok(false);
    }
    repository
        .record_application_template_release(
            workspace_id,
            &release.template_id,
            release.release_version,
            &checksum,
            false,
        )
        .await?;
    Ok(true)
}
