use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
};
use plugin_framework::{
    reconcile_provider_artifact, ArtifactReconcileInput, ArtifactReconcileOutcome,
};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{PluginRepository, UpsertPluginArtifactInstanceInput},
};

pub fn derive_availability_status(
    desired_state: PluginDesiredState,
    artifact_status: PluginArtifactStatus,
    runtime_status: PluginRuntimeStatus,
) -> PluginAvailabilityStatus {
    match desired_state {
        PluginDesiredState::Disabled => PluginAvailabilityStatus::Disabled,
        PluginDesiredState::PendingRestart => {
            if artifact_status == PluginArtifactStatus::Ready {
                PluginAvailabilityStatus::PendingRestart
            } else {
                PluginAvailabilityStatus::ArtifactMissing
            }
        }
        PluginDesiredState::ActiveRequested => match artifact_status {
            PluginArtifactStatus::Missing => PluginAvailabilityStatus::ArtifactMissing,
            PluginArtifactStatus::Staged
            | PluginArtifactStatus::InstallIncomplete
            | PluginArtifactStatus::Corrupted => PluginAvailabilityStatus::InstallIncomplete,
            PluginArtifactStatus::Ready => match runtime_status {
                PluginRuntimeStatus::Active => PluginAvailabilityStatus::Available,
                PluginRuntimeStatus::LoadFailed => PluginAvailabilityStatus::LoadFailed,
                PluginRuntimeStatus::Inactive => PluginAvailabilityStatus::InstallIncomplete,
            },
        },
    }
}

pub async fn reconcile_installation_snapshot<R>(
    repository: &R,
    node_id: &str,
    installation_id: Uuid,
) -> anyhow::Result<domain::LocalPluginInstallationRecord>
where
    R: PluginRepository + ?Sized,
{
    let local = repository
        .get_local_installation(node_id, installation_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    let reconcile = reconcile_provider_artifact(ArtifactReconcileInput {
        package_path: local
            .artifact
            .package_path
            .as_deref()
            .map(std::path::Path::new),
        installed_path: std::path::Path::new(
            local
                .artifact
                .local_path
                .as_deref()
                .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?,
        ),
        expected_artifact_sha256: local.expected_checksum.as_deref(),
        expected_manifest_fingerprint: local.artifact.manifest_fingerprint.as_deref(),
    })
    .await?;
    let (artifact_status, lifecycle_artifact_status) = match reconcile.outcome {
        ArtifactReconcileOutcome::Missing => (
            domain::PluginArtifactInstanceStatus::Missing,
            PluginArtifactStatus::Missing,
        ),
        ArtifactReconcileOutcome::InstallIncomplete => (
            domain::PluginArtifactInstanceStatus::Mismatched,
            PluginArtifactStatus::InstallIncomplete,
        ),
        ArtifactReconcileOutcome::Ready => (
            domain::PluginArtifactInstanceStatus::Ready,
            PluginArtifactStatus::Ready,
        ),
        ArtifactReconcileOutcome::Corrupted => (
            domain::PluginArtifactInstanceStatus::Corrupted,
            PluginArtifactStatus::Corrupted,
        ),
    };
    let availability_status = derive_availability_status(
        local.desired_state,
        lifecycle_artifact_status,
        local.artifact.runtime_status,
    );
    let manifest_fingerprint = reconcile
        .manifest_fingerprint
        .or_else(|| local.artifact.manifest_fingerprint.clone());
    if local.artifact.artifact_status == artifact_status
        && local.artifact.availability_status == availability_status
        && local.artifact.manifest_fingerprint == manifest_fingerprint
    {
        return Ok(local);
    }

    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: node_id.to_string(),
            installation_id,
            local_version: local.artifact.local_version.clone(),
            local_checksum: local.artifact.local_checksum.clone(),
            local_path: local.artifact.local_path.clone(),
            package_path: local.artifact.package_path.clone(),
            manifest_fingerprint,
            artifact_status,
            runtime_status: local.artifact.runtime_status,
            availability_status,
            checked_at: time::OffsetDateTime::now_utc(),
            last_error: None,
            is_current: local.artifact.is_current,
        })
        .await?;
    repository
        .get_local_installation(node_id, installation_id)
        .await?
        .ok_or_else(|| ControlPlaneError::NotFound("plugin_installation").into())
}
