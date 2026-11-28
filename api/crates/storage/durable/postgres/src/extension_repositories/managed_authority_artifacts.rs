use crate::plugin_contribution_authority_repository::AuthorityLockMode;
use anyhow::Result;
use control_plane_contracts::{
    ports::ValidateInstalledPluginManifest, ControlPlaneContractError as Error,
};
use domain::{PluginArtifactInstanceRecord, PluginInstallationRecord};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(super) async fn lock_node_artifacts(
    transaction: &mut Transaction<'_, Postgres>,
    node_id: &str,
    installation_ids: &[Uuid],
    mode: AuthorityLockMode,
) -> Result<BTreeMap<Uuid, domain::PluginArtifactInstanceRecord>> {
    if installation_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    macro_rules! artifact_query {
        ($lock:literal) => {
            concat!(
                r#"
                select node_id, installation_id, local_version, local_checksum, local_path,
                       package_path, manifest_fingerprint, artifact_status, runtime_status,
                       availability_status, checked_at, last_error, is_current
                from extension_artifact_instances
                where node_id = $1 and installation_id = any($2)
                order by installation_id
            "#,
                $lock
            )
        };
    }
    let query = match mode {
        AuthorityLockMode::Exclusive => {
            for &id in installation_ids {
                crate::plugin_repository::lock_node_artifact_slot(transaction, node_id, id).await?;
            }
            artifact_query!("for update")
        }
        AuthorityLockMode::SharedRead => {
            for &id in installation_ids {
                crate::plugin_repository::lock_node_artifact_slot_shared(transaction, node_id, id)
                    .await?;
            }
            artifact_query!("for share")
        }
    };
    sqlx::query(query)
        .bind(node_id)
        .bind(installation_ids)
        .fetch_all(&mut **transaction)
        .await?
        .into_iter()
        .map(|row| {
            let artifact = crate::plugin_repository::map_artifact_instance(row)?;
            Ok((artifact.installation_id, artifact))
        })
        .collect()
}

pub(super) async fn verify_managed_artifact(
    installations: &BTreeMap<Uuid, PluginInstallationRecord>,
    artifacts: &BTreeMap<Uuid, PluginArtifactInstanceRecord>,
    id: Uuid,
    validate_manifest: ValidateInstalledPluginManifest,
) -> Result<()> {
    if let (Some(installation), Some(artifact)) = (installations.get(&id), artifacts.get(&id)) {
        if artifact.artifact_status.is_ready()
            && !domain::has_local_artifact_warning(artifact.last_error.as_deref())
            && crate::plugin_repository::verify_locked_node_artifact_on_disk(
                installation,
                artifact,
                validate_manifest,
            )
            .await
        {
            return Ok(());
        }
    }
    Err(Error::Conflict("managed_candidate_artifact_invalid").into())
}
