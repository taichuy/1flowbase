use std::path::Path;

use anyhow::Result;
use control_plane_contracts::{ports::ValidateInstalledPluginManifest, ControlPlaneContractError};
use domain::{
    PluginArtifactInstanceRecord, PluginInstallationRecord, RuntimeArtifactMarkerIdentity,
};
use sqlx::PgConnection;
use uuid::Uuid;

pub(super) async fn lock_artifact_installation(
    tx: &mut PgConnection,
    id: Uuid,
) -> Result<PluginInstallationRecord> {
    let row = sqlx::query(
        "select *, artifact_id as provider_code, artifact_version as plugin_version,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility
         from extension_installations where id=$1 and plugin_id is not null for share",
    )
    .bind(id)
    .fetch_optional(tx)
    .await?
    .ok_or(ControlPlaneContractError::NotFound("plugin_installation"))?;
    super::map_installation(row)
}

/// The caller has matched its expected identity against this row and holds the node slot
/// and artifact row locks until commit. Re-read disk instead of trusting that prior observation.
pub(crate) async fn verify_locked_node_artifact_on_disk(
    installation: &PluginInstallationRecord,
    artifact: &PluginArtifactInstanceRecord,
    validate_manifest: ValidateInstalledPluginManifest,
) -> bool {
    let Some(path) = artifact.local_path.as_deref().map(Path::new) else {
        return false;
    };
    let Ok(bytes) = tokio::fs::read(path.join(".1flowbase-artifact.json")).await else {
        return false;
    };
    let Ok(marker) = serde_json::from_slice::<RuntimeArtifactMarkerIdentity>(&bytes) else {
        return false;
    };
    if marker.plugin_id != installation.plugin_id
        || marker.version != installation.plugin_version
        || artifact.local_version.as_deref() != Some(marker.version.as_str())
        || artifact.local_checksum != marker.checksum
        || installation
            .expected_checksum
            .as_ref()
            .is_some_and(|expected| marker.checksum.as_ref() != Some(expected))
    {
        return false;
    }
    let Ok(raw) = tokio::fs::read_to_string(path.join("manifest.yaml")).await else {
        return false;
    };
    let Some(actual_fp) = validate_manifest(installation, &raw) else {
        return false;
    };
    artifact.manifest_fingerprint.as_deref() == Some(actual_fp.as_str())
        && marker.manifest_fingerprint.as_deref() == Some(actual_fp.as_str())
}
