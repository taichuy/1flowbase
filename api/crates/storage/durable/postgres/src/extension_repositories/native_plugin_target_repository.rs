//! Native target selection has one durable owner; node runtime observations never select it.
use anyhow::{bail, Result};
use control_plane_contracts::{ports::UpdatePluginDesiredStateInput, ControlPlaneContractError};
use domain::{
    NativePluginTarget, PluginDesiredState, PluginInstallationRecord, PluginRuntimeStatus,
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::{plugin_repository::map_installation, PgControlPlaneStore};

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
    status: PluginRuntimeStatus,
    last_error: Option<&str>,
) -> Result<()> {
    if !matches!(
        status,
        PluginRuntimeStatus::Active | PluginRuntimeStatus::LoadFailed
    ) {
        bail!("invalid native startup completion status");
    }
    let mut tx = store.pool().begin().await?;
    lock_current_target(&mut tx, expected).await?;
    let active = status == PluginRuntimeStatus::Active;
    let updated = sqlx::query("update extension_artifact_instances set runtime_status=$3,artifact_status=case when $3='active' then 'ready' else 'load_failed' end,availability_status=$4,last_error=$5,checked_at=now() where node_id=$1 and installation_id=$2")
        .bind(node_id).bind(expected.installation_id).bind(status.as_str())
        .bind(if active { "available" } else { "load_failed" }).bind(last_error).execute(&mut *tx).await?;
    if updated.rows_affected() != 1 {
        bail!("native startup artifact is absent on this node");
    }
    if active {
        sqlx::query("update extension_installations set desired_state='active_requested',updated_at=now() where id=$1")
            .bind(expected.installation_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
