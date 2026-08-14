use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use async_trait::async_trait;
use control_plane::{errors::ControlPlaneError, ports::FrontstageExecutableUpgradeRepository};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Executor, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

const UPGRADE_ADVISORY_LOCK: &str = "frontstage_executable_upgrade";

fn target_identity(target: &domain::FrontstageExecutableUpgradeTarget) -> Result<Value> {
    Ok(serde_json::to_value(target)?)
}

async fn advisory_lock(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    sqlx::query("select pg_advisory_xact_lock(hashtext($1))")
        .bind(UPGRADE_ADVISORY_LOCK)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn conflict(code: &'static str) -> anyhow::Error {
    ControlPlaneError::Conflict(code).into()
}

#[async_trait]
impl FrontstageExecutableUpgradeRepository for PgControlPlaneStore {
    async fn begin_frontstage_executable_upgrade(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
    ) -> Result<domain::FrontstageExecutableUpgradeStart> {
        let target_identity = target_identity(target)?;
        let mut tx = self.pool().begin().await?;
        advisory_lock(&mut tx).await?;
        let marker = sqlx::query(
            r#"
            select target_identity, status, current_run_id
            from frontstage_executable_upgrade_markers
            where marker = $1
            for update
            "#,
        )
        .bind(&target.marker)
        .fetch_optional(&mut *tx)
        .await?;

        let marker_exists = marker.is_some();
        if let Some(marker) = marker.as_ref() {
            if marker.get::<Value, _>("target_identity") != target_identity {
                return Err(conflict("frontstage_executable_upgrade_target"));
            }
            let status: String = marker.get("status");
            if status == "completed" {
                tx.commit().await?;
                return Ok(domain::FrontstageExecutableUpgradeStart::Completed);
            }
            if status == "running" {
                let run_id: Option<Uuid> = marker.get("current_run_id");
                let run_id = run_id.ok_or_else(|| conflict("frontstage_executable_upgrade_run"))?;
                let attempt: i32 = sqlx::query_scalar(
                    "select attempt from frontstage_executable_upgrade_runs where run_id = $1 and marker = $2 and status = 'running'",
                )
                .bind(run_id)
                .bind(&target.marker)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| conflict("frontstage_executable_upgrade_run"))?;
                tx.commit().await?;
                return Ok(domain::FrontstageExecutableUpgradeStart::Run {
                    run_id,
                    attempt: attempt as u32,
                });
            }
        }

        let attempt: i32 = sqlx::query_scalar(
            "select coalesce(max(attempt), 0) + 1 from frontstage_executable_upgrade_runs where marker = $1",
        )
        .bind(&target.marker)
        .fetch_one(&mut *tx)
        .await?;
        let run_id = Uuid::now_v7();
        if !marker_exists {
            sqlx::query(
                "insert into frontstage_executable_upgrade_markers (marker, target_identity, status) values ($1, $2, 'running')",
            )
            .bind(&target.marker)
            .bind(&target_identity)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"
            insert into frontstage_executable_upgrade_runs (
                run_id, marker, attempt, target_identity, status, compiler_identity
            ) values ($1, $2, $3, $4, 'running', $5)
            "#,
        )
        .bind(run_id)
        .bind(&target.marker)
        .bind(attempt)
        .bind(&target_identity)
        .bind(&target.compiler_identity)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            update frontstage_executable_upgrade_markers
            set status = 'running', current_run_id = $2, completed_at = null, updated_at = now()
            where marker = $1
            "#,
        )
        .bind(&target.marker)
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(domain::FrontstageExecutableUpgradeStart::Run {
            run_id,
            attempt: attempt as u32,
        })
    }

    async fn capture_frontstage_executable_upgrade_snapshot(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        run_id: Uuid,
    ) -> Result<domain::LegacyFrontstageExecutableSnapshot> {
        let mut tx = self.pool().begin().await?;
        advisory_lock(&mut tx).await?;
        let run = require_running_run(&mut tx, target, run_id).await?;
        match (run.source_snapshot, run.source_snapshot_sha256) {
            (Some(snapshot), Some(snapshot_sha256)) => {
                let rows: Vec<domain::LegacyFrontstageExecutableSnapshotRow> =
                    serde_json::from_value(snapshot)
                        .map_err(|_| conflict("frontstage_executable_upgrade_snapshot_evidence"))?;
                if snapshot_digest(&rows)? != snapshot_sha256 {
                    return Err(conflict("frontstage_executable_upgrade_snapshot_evidence"));
                }
                tx.commit().await?;
                return Ok(domain::LegacyFrontstageExecutableSnapshot {
                    run_id,
                    rows,
                    snapshot_sha256,
                });
            }
            (None, None) => {}
            _ => return Err(conflict("frontstage_executable_upgrade_snapshot_evidence")),
        }
        let rows = legacy_snapshot_rows(&mut *tx).await?;
        let snapshot_sha256 = snapshot_digest(&rows)?;
        let result = sqlx::query(
            r#"
            update frontstage_executable_upgrade_runs
            set source_snapshot = $2, source_snapshot_sha256 = $3, updated_at = now()
            where run_id = $1 and marker = $4 and status = 'running'
              and source_snapshot is null and source_snapshot_sha256 is null
            "#,
        )
        .bind(run_id)
        .bind(serde_json::to_value(&rows)?)
        .bind(&snapshot_sha256)
        .bind(&target.marker)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() != 1 {
            return Err(conflict("frontstage_executable_upgrade_run"));
        }
        tx.commit().await?;
        Ok(domain::LegacyFrontstageExecutableSnapshot {
            run_id,
            rows,
            snapshot_sha256,
        })
    }

    async fn commit_frontstage_executable_upgrade(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        snapshot: &domain::LegacyFrontstageExecutableSnapshot,
        compiled: &[domain::CompiledFrontstageExecutable],
    ) -> Result<()> {
        validate_compiled_set(target, snapshot, compiled)?;
        let mut tx = self.pool().begin().await?;
        advisory_lock(&mut tx).await?;
        let run = require_running_run(&mut tx, target, snapshot.run_id).await?;
        if run.source_snapshot_sha256.as_deref() != Some(&snapshot.snapshot_sha256)
            || run.source_snapshot.as_ref() != Some(&serde_json::to_value(&snapshot.rows)?)
        {
            return Err(conflict("frontstage_executable_upgrade_snapshot_evidence"));
        }

        sqlx::query(
            r#"
            select id from frontstage_block_codes
            where source_sha256 is null or dependency_lock is null
               or tailwind_toolchain_lock is null or generated_css is null
               or generated_css_sha256 is null or compiler_identity is null
            order by workspace_id, page_id, code_ref, id
            for update
            "#,
        )
        .fetch_all(&mut *tx)
        .await?;
        let current_rows = legacy_snapshot_rows(&mut *tx).await?;
        if current_rows != snapshot.rows
            || snapshot_digest(&current_rows)? != snapshot.snapshot_sha256
        {
            return Err(conflict("frontstage_executable_upgrade_snapshot_drift"));
        }

        let payloads = compiled
            .iter()
            .map(|payload| (payload.row_id, payload))
            .collect::<BTreeMap<_, _>>();
        let mut workspace_counts = BTreeMap::<Uuid, usize>::new();
        for source in &snapshot.rows {
            let payload = payloads
                .get(&source.row_id)
                .ok_or_else(|| conflict("frontstage_executable_upgrade_compiled_set"))?;
            let updated = sqlx::query(
                r#"
                update frontstage_block_codes
                set source_sha256 = $2, dependency_lock = $3,
                    tailwind_toolchain_lock = $4, generated_css = $5,
                    generated_css_sha256 = $6, compiler_identity = $7,
                    updated_by = null, updated_at = now()
                where id = $1 and workspace_id = $8 and page_id = $9 and code_ref = $10
                  and code = $11
                  and source_sha256 is null and dependency_lock is null
                  and tailwind_toolchain_lock is null and generated_css is null
                  and generated_css_sha256 is null and compiler_identity is null
                "#,
            )
            .bind(source.row_id)
            .bind(&payload.source_sha256)
            .bind(&payload.dependency_lock)
            .bind(&payload.toolchain_lock)
            .bind(&payload.generated_css)
            .bind(&payload.generated_css_sha256)
            .bind(&payload.compiler_identity)
            .bind(source.workspace_id)
            .bind(source.page_id)
            .bind(&source.code_ref)
            .bind(&source.source_code)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(conflict("frontstage_executable_upgrade_snapshot_drift"));
            }
            *workspace_counts.entry(source.workspace_id).or_default() += 1;
        }

        for (workspace_id, upgraded) in workspace_counts {
            sqlx::query(
                r#"
                insert into audit_logs (
                    id, workspace_id, scope_id, actor_user_id, target_type, target_id,
                    event_code, payload, created_by, updated_by, created_at, updated_at
                ) values ($1, $2, $2, null, 'frontstage_executable_upgrade', $2,
                    'frontstage.executable_system_upgraded', $3, null, null, now(), now())
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(workspace_id)
            .bind(json!({
                "actor": "system_upgrade",
                "marker": target.marker,
                "run_id": snapshot.run_id,
                "upgraded": upgraded,
            }))
            .execute(&mut *tx)
            .await?;
        }

        let legacy_count: i64 = legacy_count(&mut *tx).await?;
        if legacy_count != 0 {
            return Err(conflict("frontstage_executable_upgrade_legacy_remaining"));
        }
        let completed_run = sqlx::query(
            r#"
            update frontstage_executable_upgrade_runs
            set status = 'completed', completed_at = now(), updated_at = now()
            where run_id = $1 and marker = $2 and status = 'running'
            "#,
        )
        .bind(snapshot.run_id)
        .bind(&target.marker)
        .execute(&mut *tx)
        .await?;
        if completed_run.rows_affected() != 1 {
            return Err(conflict("frontstage_executable_upgrade_run"));
        }
        let completed_marker = sqlx::query(
            r#"
            update frontstage_executable_upgrade_markers
            set status = 'completed', completed_at = now(), updated_at = now()
            where marker = $1 and current_run_id = $2 and target_identity = $3
              and status = 'running'
            "#,
        )
        .bind(&target.marker)
        .bind(snapshot.run_id)
        .bind(target_identity(target)?)
        .execute(&mut *tx)
        .await?;
        if completed_marker.rows_affected() != 1 {
            return Err(conflict("frontstage_executable_upgrade_marker"));
        }
        tx.commit().await?;
        Ok(())
    }

    async fn record_frontstage_executable_upgrade_failure(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        failure: &domain::FrontstageExecutableUpgradeFailure,
    ) -> Result<()> {
        if failure.marker != target.marker {
            return Err(conflict("frontstage_executable_upgrade_target"));
        }
        let mut tx = self.pool().begin().await?;
        advisory_lock(&mut tx).await?;
        let marker = sqlx::query(
            "select target_identity, status, current_run_id from frontstage_executable_upgrade_markers where marker = $1 for update",
        )
        .bind(&target.marker)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| conflict("frontstage_executable_upgrade_marker"))?;
        if marker.get::<Value, _>("target_identity") != target_identity(target)?
            || marker.get::<Option<Uuid>, _>("current_run_id") != Some(failure.run_id)
        {
            return Err(conflict("frontstage_executable_upgrade_target"));
        }
        if marker.get::<String, _>("status") == "failed" {
            let existing = sqlx::query(
                "select error_code, failure_target_identity, compiler_identity from frontstage_executable_upgrade_runs where run_id = $1 and status = 'failed'",
            )
            .bind(failure.run_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| conflict("frontstage_executable_upgrade_run"))?;
            if existing.get::<Option<String>, _>("error_code").as_deref()
                != Some(&failure.error_code)
                || existing
                    .get::<Option<Value>, _>("failure_target_identity")
                    .as_ref()
                    != Some(&failure.target_identity)
                || existing.get::<Value, _>("compiler_identity") != failure.compiler_identity
            {
                return Err(conflict("frontstage_executable_upgrade_failure_evidence"));
            }
            tx.commit().await?;
            return Ok(());
        }
        let updated_run = sqlx::query(
            r#"
            update frontstage_executable_upgrade_runs
            set status = 'failed', error_code = $2, failure_target_identity = $3,
                failed_at = now(), updated_at = now()
            where run_id = $1 and marker = $4 and status = 'running'
              and compiler_identity = $5
            "#,
        )
        .bind(failure.run_id)
        .bind(&failure.error_code)
        .bind(&failure.target_identity)
        .bind(&target.marker)
        .bind(&failure.compiler_identity)
        .execute(&mut *tx)
        .await?;
        if updated_run.rows_affected() != 1 {
            return Err(conflict("frontstage_executable_upgrade_run"));
        }
        let updated_marker = sqlx::query(
            "update frontstage_executable_upgrade_markers set status = 'failed', updated_at = now() where marker = $1 and current_run_id = $2 and status = 'running'",
        )
        .bind(&target.marker)
        .bind(failure.run_id)
        .execute(&mut *tx)
        .await?;
        if updated_marker.rows_affected() != 1 {
            return Err(conflict("frontstage_executable_upgrade_marker"));
        }
        tx.commit().await?;
        Ok(())
    }

    async fn require_frontstage_executable_cutover(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        advisory_lock(&mut tx).await?;
        let marker = sqlx::query(
            "select target_identity, status from frontstage_executable_upgrade_markers where marker = $1",
        )
        .bind(&target.marker)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| conflict("frontstage_executable_upgrade_cutover"))?;
        if marker.get::<Value, _>("target_identity") != target_identity(target)?
            || marker.get::<String, _>("status") != "completed"
            || legacy_count(&mut *tx).await? != 0
        {
            return Err(conflict("frontstage_executable_upgrade_cutover"));
        }
        tx.commit().await?;
        Ok(())
    }
}

struct RunningRunEvidence {
    source_snapshot: Option<Value>,
    source_snapshot_sha256: Option<String>,
}

async fn require_running_run(
    tx: &mut Transaction<'_, Postgres>,
    target: &domain::FrontstageExecutableUpgradeTarget,
    run_id: Uuid,
) -> Result<RunningRunEvidence> {
    let row = sqlx::query(
        r#"
        select run.source_snapshot, run.source_snapshot_sha256
        from frontstage_executable_upgrade_markers marker
        join frontstage_executable_upgrade_runs run on run.run_id = marker.current_run_id
        where marker.marker = $1 and marker.target_identity = $2
          and marker.status = 'running' and run.status = 'running' and run.run_id = $3
        for update of marker, run
        "#,
    )
    .bind(&target.marker)
    .bind(target_identity(target)?)
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| conflict("frontstage_executable_upgrade_run"))?;
    Ok(RunningRunEvidence {
        source_snapshot: row.get("source_snapshot"),
        source_snapshot_sha256: row.get("source_snapshot_sha256"),
    })
}

async fn legacy_count<'e, E>(executor: E) -> Result<i64>
where
    E: Executor<'e, Database = Postgres>,
{
    Ok(sqlx::query_scalar(
        r#"
        select count(*) from frontstage_block_codes
        where source_sha256 is null or dependency_lock is null
           or tailwind_toolchain_lock is null or generated_css is null
           or generated_css_sha256 is null or compiler_identity is null
        "#,
    )
    .fetch_one(executor)
    .await?)
}

async fn legacy_snapshot_rows<'e, E>(
    executor: E,
) -> Result<Vec<domain::LegacyFrontstageExecutableSnapshotRow>>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query(
        r#"
        select code.id as row_id, code.workspace_id, code.page_id, code.code_ref,
               code.code as source_code,
               coalesce(node.runtime_descriptor, jsonb_build_object('codeRef', code.code_ref))
                 as runtime_descriptor,
               catalog.installation_id, catalog.provider_code, catalog.plugin_id,
               catalog.plugin_version, catalog.contribution_code, catalog.code_modules,
               count(catalog.id) over (partition by code.id) as catalog_matches
        from frontstage_block_codes code
        left join frontstage_block_nodes node
          on node.scope_id = code.workspace_id
         and node.tree_partition_id = code.page_id
         and node.code_ref = code.code_ref
        left join frontend_block_catalog catalog on (
          catalog.installation_id::text = node.runtime_descriptor #>> '{catalog,installationId}'
          and catalog.provider_code = node.runtime_descriptor #>> '{catalog,providerCode}'
          and catalog.plugin_id = node.runtime_descriptor #>> '{contribution,pluginId}'
          and catalog.plugin_version = node.runtime_descriptor #>> '{contribution,pluginVersion}'
          and catalog.contribution_code = node.runtime_descriptor #>> '{contribution,code}'
        ) or (
          (
            node.id is null
            or (
              not (node.runtime_descriptor ? 'catalog')
              and not (node.runtime_descriptor ? 'contribution')
            )
          )
          and catalog.id = (
            select (array_agg(candidate.id order by candidate.id))[1]
            from frontend_block_catalog candidate
            having count(*) = 1
          )
        )
        where code.source_sha256 is null or code.dependency_lock is null
           or code.tailwind_toolchain_lock is null or code.generated_css is null
           or code.generated_css_sha256 is null or code.compiler_identity is null
        order by code.workspace_id, code.page_id, code.code_ref collate "C", code.id
        "#,
    )
    .fetch_all(executor)
    .await?;
    rows.into_iter().map(snapshot_row).collect()
}

fn snapshot_row(
    row: sqlx::postgres::PgRow,
) -> Result<domain::LegacyFrontstageExecutableSnapshotRow> {
    let catalog_matches: i64 = row.get("catalog_matches");
    let installation_id: Option<Uuid> = row.get("installation_id");
    let runtime_descriptor: Option<Value> = row.get("runtime_descriptor");
    let (Some(installation_id), Some(runtime_descriptor)) = (installation_id, runtime_descriptor)
    else {
        return Err(conflict("frontstage_executable_upgrade_catalog_locator"));
    };
    if catalog_matches != 1 {
        return Err(conflict("frontstage_executable_upgrade_catalog_locator"));
    }
    let workspace_id: Uuid = row.get("workspace_id");
    let modules: Value = row.get("code_modules");
    let dependency_lock = dependency_lock(workspace_id, modules)?;
    let source_code: String = row.get("source_code");
    let source_sha256 = format!("{:x}", Sha256::digest(source_code.as_bytes()));
    Ok(domain::LegacyFrontstageExecutableSnapshotRow {
        row_id: row.get("row_id"),
        workspace_id,
        page_id: row.get("page_id"),
        code_ref: row.get("code_ref"),
        source_code,
        source_sha256,
        catalog_locator: domain::FrontstageExecutableCatalogLocator {
            installation_id,
            provider_code: row.get("provider_code"),
            plugin_id: row.get("plugin_id"),
            plugin_version: row.get("plugin_version"),
            contribution_code: row.get("contribution_code"),
        },
        runtime_descriptor,
        dependency_lock,
    })
}

fn dependency_lock(workspace_id: Uuid, modules: Value) -> Result<Value> {
    let modules: Vec<domain::FrontendBlockCodeModule> = serde_json::from_value(modules)
        .map_err(|_| conflict("frontstage_executable_upgrade_catalog_dependency_lock"))?;
    let mut seen = BTreeSet::new();
    let mut lock = Vec::with_capacity(modules.len());
    for module in modules {
        if !seen.insert(module.source.clone()) || module.exports.is_empty() {
            return Err(conflict(
                "frontstage_executable_upgrade_catalog_dependency_lock",
            ));
        }
        let assets = module
            .assets
            .into_iter()
            .map(|asset| {
                json!({
                    "role": asset.role,
                    "media_type": asset.media_type,
                    "sha256": asset.sha256,
                    "url": format!(
                        "/api/console/frontstage/{workspace_id}/component-module-assets/{}",
                        asset.sha256
                    ),
                })
            })
            .collect::<Vec<_>>();
        lock.push(json!({
            "module_source": module.source,
            "module_version": module.version,
            "binding": module.binding,
            "assets": assets,
            "exports": module.exports,
        }));
    }
    Ok(Value::Array(lock))
}

fn snapshot_digest(rows: &[domain::LegacyFrontstageExecutableSnapshotRow]) -> Result<String> {
    let value = canonical_value(serde_json::to_value(rows)?);
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
}

fn canonical_value(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonical_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, canonical_value(value)))
                .collect(),
        ),
        value => value,
    }
}

fn validate_compiled_set(
    target: &domain::FrontstageExecutableUpgradeTarget,
    snapshot: &domain::LegacyFrontstageExecutableSnapshot,
    compiled: &[domain::CompiledFrontstageExecutable],
) -> Result<()> {
    let payloads = compiled
        .iter()
        .map(|payload| (payload.row_id, payload))
        .collect::<BTreeMap<_, _>>();
    if payloads.len() != compiled.len() || payloads.len() != snapshot.rows.len() {
        return Err(conflict("frontstage_executable_upgrade_compiled_set"));
    }
    for source in &snapshot.rows {
        let payload = payloads
            .get(&source.row_id)
            .ok_or_else(|| conflict("frontstage_executable_upgrade_compiled_set"))?;
        let css_sha256 = format!("{:x}", Sha256::digest(payload.generated_css.as_bytes()));
        if payload.source_sha256 != source.source_sha256
            || payload.dependency_lock != source.dependency_lock
            || payload.generated_css_sha256 != css_sha256
            || payload.compiler_identity != target.compiler_identity
            || payload.toolchain_lock != target.toolchain_lock
            || payload.contract_identity != target.contract_identity
        {
            return Err(conflict("frontstage_executable_upgrade_compiled_payload"));
        }
    }
    Ok(())
}
