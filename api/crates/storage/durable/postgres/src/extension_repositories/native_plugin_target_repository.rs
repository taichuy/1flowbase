//! Native target selection has one durable owner; node runtime observations never select it.
use std::{io::ErrorKind, path::Path};

use anyhow::{bail, Result};
use control_plane_contracts::{
    ports::{
        derive_contract_availability_status, plugin_artifact_status, NativePluginStartupOutcome,
        UpdatePluginDesiredStateInput, VerifiedNativePluginArtifact,
    },
    ControlPlaneContractError,
};
use domain::{
    NativePluginTarget, PluginDesiredState, PluginInstallationRecord, PluginRuntimeStatus,
    RuntimeArtifactMarkerIdentity,
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::{
    mappers::plugin_mapper::{parse_artifact_instance_status, parse_desired_state},
    plugin_repository::{lock_node_artifact_slot, map_artifact_instance, map_installation},
    PgControlPlaneStore,
};

fn target(row: sqlx::postgres::PgRow) -> NativePluginTarget {
    NativePluginTarget {
        scope_id: row.get("scope_id"),
        category: domain::ExtensionCategory::HostExtensions,
        organization: row.get("organization"),
        artifact_id: row.get("artifact_id"),
        installation_id: row.get("installation_id"),
        selection_revision: row.get("selection_revision"),
        enabled: row.get("enabled"),
        application_generation: row.get("application_generation"),
    }
}

async fn installation(connection: &mut PgConnection, id: Uuid) -> Result<PluginInstallationRecord> {
    let row = sqlx::query("select *, artifact_id as provider_code, artifact_version as plugin_version, receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility from extension_installations where id=$1 and category='host-extensions' and plugin_id is not null")
        .bind(id).fetch_optional(connection).await?
        .ok_or(ControlPlaneContractError::NotFound("native_plugin_installation"))?;
    map_installation(row)
}

async fn lock_family(connection: &mut PgConnection, i: &PluginInstallationRecord) -> Result<()> {
    // The lock covers an absent target's first insert. The exact row/FK/CAS below authorizes
    // writes; this advisory lock alone never grants a prepared attempt permission to write.
    let key = serde_json::to_string(&(
        i.scope_id,
        i.category.as_str(),
        &i.organization,
        &i.provider_code,
    ))?;
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1, 201405))")
        .bind(key)
        .execute(connection)
        .await?;
    Ok(())
}

async fn selected(
    connection: &mut PgConnection,
    i: &PluginInstallationRecord,
) -> Result<Option<NativePluginTarget>> {
    Ok(sqlx::query("select * from native_plugin_targets where scope_id=$1 and category=$2 and organization=$3 and artifact_id=$4 for update")
        .bind(i.scope_id).bind(i.category.as_str()).bind(&i.organization).bind(&i.provider_code)
        .fetch_optional(connection).await?.map(target))
}

async fn write_selection(
    connection: &mut PgConnection,
    i: &PluginInstallationRecord,
    previous: Option<&NativePluginTarget>,
    actor: Uuid,
) -> Result<NativePluginTarget> {
    let generation = previous.map_or(1, |p| {
        p.application_generation + i64::from(p.installation_id != i.id)
    });
    let revision = previous.map_or(1, |p| p.selection_revision + 1);
    if previous.is_none_or(|p| p.installation_id != i.id) {
        sqlx::query("insert into native_plugin_application_requests(scope_id,category,organization,artifact_id,application_generation,installation_id,created_by) values($1,$2,$3,$4,$5,$6,$7)")
            .bind(i.scope_id).bind(i.category.as_str()).bind(&i.organization).bind(&i.provider_code)
            .bind(generation).bind(i.id).bind(actor).execute(&mut *connection).await?;
    }
    let row = sqlx::query("insert into native_plugin_targets(scope_id,category,organization,artifact_id,installation_id,selection_revision,enabled,application_generation,created_by,updated_by) values($1,$2,$3,$4,$5,$6,true,$7,$8,$8) on conflict(scope_id,category,organization,artifact_id) do update set installation_id=excluded.installation_id,selection_revision=excluded.selection_revision,enabled=true,application_generation=excluded.application_generation,updated_by=excluded.updated_by,updated_at=now() returning *")
        .bind(i.scope_id).bind(i.category.as_str()).bind(&i.organization).bind(&i.provider_code)
        .bind(i.id).bind(revision).bind(generation).bind(actor).fetch_one(connection).await?;
    Ok(target(row))
}

pub(crate) async fn update_desired_state(
    store: &PgControlPlaneStore,
    input: &UpdatePluginDesiredStateInput,
) -> Result<PluginInstallationRecord> {
    let mut tx = store.pool().begin().await?;
    let i = installation(&mut tx, input.installation_id).await?;
    lock_family(&mut tx, &i).await?;
    let previous = selected(&mut tx, &i).await?;
    if input.desired_state == PluginDesiredState::Disabled {
        if let Some(p) = &previous {
            if p.installation_id != i.id {
                return Err(
                    ControlPlaneContractError::Conflict("native_plugin_target_changed").into(),
                );
            }
            sqlx::query("update native_plugin_targets set enabled=false,selection_revision=selection_revision+1,updated_by=$2,updated_at=now() where installation_id=$1")
                .bind(i.id).bind(input.actor_user_id).execute(&mut *tx).await?;
        }
    } else {
        write_selection(&mut tx, &i, previous.as_ref(), input.actor_user_id).await?;
        // Superseded desired facts are disabled, but their exact artifacts and runtime/history
        // remain intact. A running old process is not represented as unloaded.
        sqlx::query("update extension_installations set desired_state='disabled',updated_by=$5,updated_at=now() where scope_id=$1 and category=$2 and organization=$3 and artifact_id=$4 and id<>$6 and desired_state<>'disabled'")
            .bind(i.scope_id).bind(i.category.as_str()).bind(&i.organization).bind(&i.provider_code)
            .bind(input.actor_user_id).bind(i.id).execute(&mut *tx).await?;
    }
    let state = if input.desired_state == PluginDesiredState::Disabled {
        "disabled"
    } else {
        "pending_restart"
    };
    sqlx::query("update extension_installations set desired_state=$2,updated_by=$3,updated_at=now() where id=$1")
        .bind(i.id).bind(state).bind(input.actor_user_id).execute(&mut *tx).await?;
    let updated = installation(&mut tx, i.id).await?;
    tx.commit().await?;
    Ok(updated)
}

pub(crate) async fn list(store: &PgControlPlaneStore) -> Result<Vec<NativePluginTarget>> {
    Ok(sqlx::query(
        "select * from native_plugin_targets order by scope_id,category,organization,artifact_id",
    )
    .fetch_all(store.pool())
    .await?
    .into_iter()
    .map(target)
    .collect())
}

pub(crate) async fn reconcile_legacy(
    store: &PgControlPlaneStore,
    installation_id: Uuid,
) -> Result<Option<NativePluginTarget>> {
    let mut tx = store.pool().begin().await?;
    let i = installation(&mut tx, installation_id).await?;
    lock_family(&mut tx, &i).await?;
    if let Some(current) = selected(&mut tx, &i).await? {
        tx.commit().await?;
        return Ok(Some(current));
    }
    let candidates: Vec<Uuid> = sqlx::query_scalar("select id from extension_installations where scope_id=$1 and category=$2 and organization=$3 and artifact_id=$4 and plugin_id is not null and desired_state in ('pending_restart','active_requested') order by id")
        .bind(i.scope_id).bind(i.category.as_str()).bind(&i.organization).bind(&i.provider_code).fetch_all(&mut *tx).await?;
    match candidates.as_slice() {
        [] => {
            tx.commit().await?;
            Ok(None)
        }
        [id] => {
            let candidate = installation(&mut tx, *id).await?;
            let current = write_selection(&mut tx, &candidate, None, candidate.created_by).await?;
            tx.commit().await?;
            Ok(Some(current))
        }
        _ => Err(anyhow::Error::new(ControlPlaneContractError::Conflict(
            "native_plugin_selection_conflict",
        ))
        .context(format!(
            "scope={} organization={} artifact={} installation_ids={candidates:?}",
            i.scope_id, i.organization, i.provider_code
        ))),
    }
}

/// Available to template application within its own transaction in the following packet.
pub(crate) async fn lock_current_target(
    connection: &mut PgConnection,
    expected: &NativePluginTarget,
) -> Result<()> {
    let current = sqlx::query("select * from native_plugin_targets where scope_id=$1 and category=$2 and organization=$3 and artifact_id=$4 for update")
        .bind(expected.scope_id).bind(expected.category.as_str()).bind(&expected.organization).bind(&expected.artifact_id)
        .fetch_optional(connection).await?.map(target);
    if !expected.enabled || current.as_ref() != Some(expected) {
        return Err(ControlPlaneContractError::Conflict("native_plugin_target_changed").into());
    }
    Ok(())
}

pub(crate) async fn complete_startup(
    store: &PgControlPlaneStore,
    expected: &NativePluginTarget,
    node_id: &str,
    outcome: NativePluginStartupOutcome<'_>,
) -> Result<()> {
    let mut tx = store.pool().begin().await?;
    lock_current_target(&mut tx, expected).await?;
    let (status, artifact_status, availability, fingerprint, last_error) = match outcome {
        NativePluginStartupOutcome::Active(verified) => {
            // Take the installation row before the node slot, as other lifecycle writers do.
            let (plugin_id, source_kind, expected_checksum): (String, String, Option<String>) =
                sqlx::query_as("select plugin_id, source_kind, expected_checksum from extension_installations where id=$1 and plugin_id is not null for no key update")
                    .bind(expected.installation_id).fetch_one(&mut *tx).await?;
            lock_node_artifact_slot(&mut tx, node_id, expected.installation_id).await?;
            let existing = sqlx::query("select * from extension_artifact_instances where node_id=$1 and installation_id=$2 for update")
                .bind(node_id).bind(expected.installation_id).fetch_optional(&mut *tx).await?
                .map(map_artifact_instance).transpose()?;
            existing
                .filter(|existing| {
                    matches!(
                        existing.artifact_status,
                        domain::PluginArtifactInstanceStatus::Ready
                            | domain::PluginArtifactInstanceStatus::LoadFailed
                    ) && !domain::has_local_artifact_warning(existing.last_error.as_deref())
                        && existing.local_path.as_deref() == Some(verified.local_path.as_str())
                        && existing.local_version.as_deref()
                            == Some(verified.local_version.as_str())
                        && existing.local_checksum == verified.local_checksum
                        && existing
                            .manifest_fingerprint
                            .as_ref()
                            .is_none_or(|fp| fp == &verified.manifest_fingerprint)
                })
                .ok_or(ControlPlaneContractError::Conflict(
                    "native_plugin_artifact_changed",
                ))?;
            let marker_required = source_kind != "filesystem_dropin";
            if !verify_locked_native_artifact_on_disk(
                &plugin_id,
                expected_checksum.as_deref(),
                marker_required,
                verified,
            )
            .await
            {
                return Err(
                    ControlPlaneContractError::Conflict("native_plugin_artifact_changed").into(),
                );
            }
            sqlx::query("update extension_installations set desired_state='active_requested',updated_at=now() where id=$1")
                .bind(expected.installation_id).execute(&mut *tx).await?;
            (
                "active",
                "ready",
                "available",
                Some(verified.manifest_fingerprint.as_str()),
                None,
            )
        }
        NativePluginStartupOutcome::LoadFailed(error) => {
            lock_node_artifact_slot(&mut tx, node_id, expected.installation_id).await?;
            (
                "load_failed",
                "load_failed",
                "load_failed",
                None,
                Some(error),
            )
        }
    };
    let updated = sqlx::query("update extension_artifact_instances set runtime_status=$3,artifact_status=$4,availability_status=$5,manifest_fingerprint=coalesce($6,manifest_fingerprint),last_error=$7,checked_at=greatest(now(), checked_at + interval '1 microsecond') where node_id=$1 and installation_id=$2")
        .bind(node_id).bind(expected.installation_id).bind(status).bind(artifact_status).bind(availability)
        .bind(fingerprint).bind(last_error).execute(&mut *tx).await?;
    if updated.rows_affected() != 1 {
        bail!("native startup artifact is absent on this node");
    }
    tx.commit().await?;
    Ok(())
}

/// Revalidate the prepared package under the target, node slot and artifact row locks.
async fn verify_locked_native_artifact_on_disk(
    plugin_id: &str,
    expected_checksum: Option<&str>,
    marker_required: bool,
    verified: &VerifiedNativePluginArtifact,
) -> bool {
    let root = Path::new(&verified.local_path);
    let Ok(raw) = tokio::fs::read_to_string(root.join("manifest.yaml")).await else {
        return false;
    };
    let Some(entry) = (verified.verify_manifest)(&raw, plugin_id, verified) else {
        return false;
    };
    let Ok(contribution) = tokio::fs::read_to_string(root.join(entry)).await else {
        return false;
    };
    if !(verified.contribution_is_valid)(&contribution) {
        return false;
    }
    match tokio::fs::read(root.join(".1flowbase-artifact.json")).await {
        Ok(bytes) => {
            serde_json::from_slice::<RuntimeArtifactMarkerIdentity>(&bytes).is_ok_and(|marker| {
                marker.plugin_id == plugin_id
                    && marker.version == verified.local_version
                    && marker.checksum.as_deref() == expected_checksum
                    && marker.checksum == verified.local_checksum
                    && marker
                        .manifest_fingerprint
                        .as_deref()
                        .map_or(!marker_required, |fp| fp == verified.manifest_fingerprint)
            })
        }
        Err(error) => error.kind() == ErrorKind::NotFound && !marker_required,
    }
}

impl PgControlPlaneStore {
    /// Startup only: retire this node's active observations with no admitted boot attempt.
    pub async fn reset_unloaded_native_startup_artifacts(
        &self,
        node_id: &str,
        boot_targets: &[&NativePluginTarget],
    ) -> Result<()> {
        reset_unloaded_startup_artifacts(self, node_id, boot_targets).await
    }
}

async fn reset_unloaded_startup_artifacts(
    store: &PgControlPlaneStore,
    node_id: &str,
    boot_targets: &[&NativePluginTarget],
) -> Result<()> {
    let mut tx = store.pool().begin().await?;
    let active = sqlx::query(
        "select i.*, i.artifact_id as provider_code, i.artifact_version as plugin_version, i.receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility
         from extension_installations i join extension_artifact_instances a on a.installation_id=i.id
         where a.node_id=$1 and i.category='host-extensions' and i.plugin_id is not null and a.runtime_status='active'
         order by i.scope_id,i.organization,i.artifact_id,i.id",
    ).bind(node_id).fetch_all(&mut *tx).await?;
    for row in active {
        let i = map_installation(row)?;
        lock_family(&mut tx, &i).await?;
        // A prepared candidate is exempt only if its enabled target is still current.
        if selected(&mut tx, &i).await?.is_some_and(|live| {
            live.enabled && live.installation_id == i.id && boot_targets.contains(&&live)
        }) {
            continue;
        }
        let Some(desired) = sqlx::query_scalar::<_, String>(
            "select desired_state from extension_installations where id=$1 for share",
        )
        .bind(i.id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            continue;
        };
        lock_node_artifact_slot(&mut tx, node_id, i.id).await?;
        let Some(artifact) = sqlx::query_scalar::<_, String>(
            "select artifact_status from extension_artifact_instances
             where node_id=$1 and installation_id=$2 and runtime_status='active' for update",
        )
        .bind(node_id)
        .bind(i.id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            continue;
        };
        let availability = derive_contract_availability_status(
            parse_desired_state(&desired)?,
            plugin_artifact_status(parse_artifact_instance_status(&artifact)?),
            PluginRuntimeStatus::Inactive,
        );
        sqlx::query(
            "update extension_artifact_instances set runtime_status='inactive', availability_status=$3,
             checked_at=greatest(now(), checked_at + interval '1 microsecond')
             where node_id=$1 and installation_id=$2",
        ).bind(node_id).bind(i.id).bind(availability.as_str()).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
