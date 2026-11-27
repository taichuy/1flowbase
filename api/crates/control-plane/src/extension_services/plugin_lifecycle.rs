use domain::{
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
};
use plugin_framework::{
    reconcile_provider_artifact, ArtifactReconcileInput, ArtifactReconcileOutcome,
};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    plugin_management::{inspect_artifact_path, ARTIFACT_MARKER_FILE},
    ports::{PluginRepository, RefreshPluginArtifactInstanceInput},
};

pub(crate) const ARTIFACT_REFRESH_ATTEMPTS: usize = 8;

pub fn derive_availability_status(
    desired_state: PluginDesiredState,
    artifact_status: PluginArtifactStatus,
    runtime_status: PluginRuntimeStatus,
) -> PluginAvailabilityStatus {
    control_plane_contracts::ports::derive_contract_availability_status(
        desired_state,
        artifact_status,
        runtime_status,
    )
}

pub async fn reconcile_installation_snapshot<R>(
    repository: &R,
    node_id: &str,
    installation_id: Uuid,
) -> anyhow::Result<domain::LocalPluginInstallationRecord>
where
    R: PluginRepository + ?Sized,
{
    for _ in 0..ARTIFACT_REFRESH_ATTEMPTS {
        let local = repository
            .get_local_installation(node_id, installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let installed_path = std::path::Path::new(
            local
                .artifact
                .local_path
                .as_deref()
                .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?,
        );
        let reconcile = reconcile_provider_artifact(ArtifactReconcileInput {
            package_path: local
                .artifact
                .package_path
                .as_deref()
                .map(std::path::Path::new),
            installed_path,
            expected_artifact_sha256: local.expected_checksum.as_deref(),
            expected_manifest_fingerprint: local.artifact.manifest_fingerprint.as_deref(),
        })
        .await?;
        let mut artifact_status = match reconcile.outcome {
            ArtifactReconcileOutcome::Missing => domain::PluginArtifactInstanceStatus::Missing,
            ArtifactReconcileOutcome::InstallIncomplete => {
                domain::PluginArtifactInstanceStatus::Mismatched
            }
            ArtifactReconcileOutcome::Ready => domain::PluginArtifactInstanceStatus::Ready,
            ArtifactReconcileOutcome::Corrupted => domain::PluginArtifactInstanceStatus::Corrupted,
        };
        let had_warning = domain::has_local_artifact_warning(local.artifact.last_error.as_deref());
        let marker_exists = installed_path
            .join(ARTIFACT_MARKER_FILE)
            .try_exists()
            .unwrap_or(true);
        let mut last_error = reconcile.last_error;
        let (local_version, local_checksum, manifest_fingerprint) =
            if marker_exists || had_warning || local.expected_checksum.is_some() {
                let scanned = inspect_artifact_path(&local.installation, installed_path).await;
                if artifact_status.is_ready() {
                    artifact_status = scanned.artifact_status;
                    last_error = if !marker_exists && had_warning {
                        local.artifact.last_error.clone()
                    } else {
                        scanned.last_error.or(last_error)
                    };
                }
                (
                    scanned.local_version,
                    scanned.local_checksum,
                    scanned.manifest_fingerprint,
                )
            } else {
                // Lightweight markerless fixtures have no installation identity to rescan.
                (
                    local.artifact.local_version.clone(),
                    local.artifact.local_checksum.clone(),
                    reconcile.manifest_fingerprint,
                )
            };
        if repository
            .refresh_artifact_instance(&RefreshPluginArtifactInstanceInput {
                node_id: node_id.to_string(),
                installation_id,
                local_version,
                local_checksum,
                local_path: local.artifact.local_path.clone(),
                observed_artifact: Some(local.artifact),
                touch_checked_at: false,
                manifest_fingerprint,
                artifact_status,
                last_error,
                reset_runtime_status: false,
                checked_at: time::OffsetDateTime::now_utc(),
            })
            .await?
            .is_some()
        {
            return repository
                .get_local_installation(node_id, installation_id)
                .await?
                .ok_or_else(|| ControlPlaneError::NotFound("plugin_installation").into());
        }
    }
    Err(ControlPlaneError::Conflict("plugin_artifact_state_changed").into())
}
