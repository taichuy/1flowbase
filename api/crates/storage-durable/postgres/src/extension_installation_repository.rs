use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use control_plane::errors::ControlPlaneError;
use control_plane::ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput};
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;

use crate::{
    mappers::extension_installation::{
        to_extension_installation_record, StoredExtensionInstallationRow,
    },
    repositories::PgControlPlaneStore,
};

const JOINED_COLUMNS: &str = r#"
    installation.id,
    installation.category,
    installation.organization,
    installation.artifact_id,
    installation.artifact_version,
    installation.source_kind,
    installation.trust_level,
    installation.expected_checksum,
    installation.signature_status,
    installation.signature_algorithm,
    installation.signing_key_id,
    installation.warnings,
    installation.receipt,
    installation.application_action,
    installation.is_system_reserved,
    installation.created_by,
    installation.created_at,
    greatest(installation.updated_at, artifact.checked_at) as updated_at,
    artifact.node_id,
    artifact.local_path,
    artifact.local_checksum,
    case when artifact.artifact_status = 'ready' then 'installed' else 'missing' end as status,
    artifact.is_current
"#;

const DELETION_DECISION_QUERY: &str = r#"
    select
        installation.is_system_reserved,
        artifact.is_current,
        exists(select 1 from plugin_assignments where installation_id = installation.id)
            as has_assignment,
        exists(
            select 1 from plugin_tasks
            where installation_id = installation.id and status in ('queued', 'running')
        ) as has_active_task,
        exists(
            select 1 from plugin_worker_leases
            where installation_id = installation.id and status in ('starting', 'idle', 'busy')
        ) as has_active_worker,
        exists(select 1 from model_provider_instances where installation_id = installation.id)
            as has_model_provider_instance,
        exists(select 1 from model_provider_preview_sessions where installation_id = installation.id)
            as has_model_provider_preview,
        exists(select 1 from data_source_instances where installation_id = installation.id)
            as has_data_source_instance,
        exists(select 1 from host_infrastructure_provider_configs where installation_id = installation.id)
            as has_host_infrastructure_config,
        exists(select 1 from application_js_dependency_selections where installation_id = installation.id)
            as has_application_js_selection,
        exists(select 1 from application_extension_sources where extension_installation_id = installation.id)
            as has_application_source,
        exists(select 1 from mcp_extension_bundle_imports where extension_installation_id = installation.id)
            as has_mcp_import
    from extension_installations installation
    join extension_artifact_instances artifact
      on artifact.installation_id = installation.id and artifact.node_id = $1
    where installation.id = $2
"#;

#[async_trait]
impl ExtensionInstallationRepository for PgControlPlaneStore {
    async fn upsert_extension_installation(
        &self,
        input: &UpsertExtensionInstallationInput,
    ) -> Result<domain::ExtensionInstallationRecord> {
        if matches!(
            input.identity.category,
            domain::ExtensionCategory::CapabilityPlugins
                | domain::ExtensionCategory::HostExtensions
                | domain::ExtensionCategory::RuntimeExtensions
        ) {
            bail!("node plugin installation must use the unified runtime installation command");
        }

        let mut transaction = self.pool().begin().await?;
        let verification_status = match input.signature_status {
            domain::ExtensionSignatureStatus::Verified => "valid",
            domain::ExtensionSignatureStatus::Invalid => "invalid",
            domain::ExtensionSignatureStatus::Missing
            | domain::ExtensionSignatureStatus::UnknownKey => "pending",
        };
        let installation_id: Uuid = sqlx::query_scalar(
            r#"
            insert into extension_installations (
                id, category, organization, artifact_id, artifact_version,
                display_name, source_kind, trust_level, verification_status,
                expected_checksum, signature_status, signature_algorithm,
                signing_key_id, warnings, receipt, application_action,
                metadata_json, is_system_reserved, created_by, updated_by
            ) values (
                $1, $2, $3, $4, $5,
                $4, $6, $7, $8,
                $9, $10, $11,
                $12, $13, $14, $15,
                '{}'::jsonb, $16 = 'builtin', $17, $17
            )
            on conflict (category, organization, artifact_id, artifact_version)
            do update set
                source_kind = excluded.source_kind,
                trust_level = excluded.trust_level,
                verification_status = excluded.verification_status,
                expected_checksum = excluded.expected_checksum,
                signature_status = excluded.signature_status,
                signature_algorithm = excluded.signature_algorithm,
                signing_key_id = excluded.signing_key_id,
                warnings = excluded.warnings,
                receipt = excluded.receipt,
                application_action = excluded.application_action,
                is_system_reserved = excluded.is_system_reserved,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning id
            "#,
        )
        .bind(input.installation_id)
        .bind(input.identity.category.as_str())
        .bind(&input.identity.organization)
        .bind(&input.identity.artifact_id)
        .bind(&input.identity.version)
        .bind(&input.source_kind)
        .bind(&input.trust_level)
        .bind(verification_status)
        .bind(input.expected_checksum.as_deref())
        .bind(input.signature_status.as_str())
        .bind(&input.signature_algorithm)
        .bind(&input.signing_key_id)
        .bind(serde_json::to_value(&input.warnings)?)
        .bind(&input.receipt)
        .bind(input.application_action.as_str())
        .bind(&input.source_kind)
        .bind(input.created_by)
        .fetch_one(&mut *transaction)
        .await?;

        if input.is_current {
            demote_current_family_artifact(&mut transaction, &input.node_id, &input.identity)
                .await?;
        }
        sqlx::query(
            r#"
            insert into extension_artifact_instances (
                node_id, installation_id, local_version, local_checksum, local_path,
                artifact_status, runtime_status, availability_status,
                checked_at, last_error, is_current
            ) values (
                $1, $2, $3, $4, $5,
                $6, 'inactive', $7,
                now(), null, $8
            )
            on conflict (node_id, installation_id) do update set
                local_version = excluded.local_version,
                local_checksum = excluded.local_checksum,
                local_path = excluded.local_path,
                artifact_status = excluded.artifact_status,
                availability_status = excluded.availability_status,
                checked_at = now(),
                last_error = null,
                is_current = excluded.is_current
            "#,
        )
        .bind(&input.node_id)
        .bind(installation_id)
        .bind(&input.identity.version)
        .bind(&input.local_checksum)
        .bind(&input.local_path)
        .bind(match input.status {
            domain::ExtensionInstallationStatus::Installed => "ready",
            domain::ExtensionInstallationStatus::Missing => "missing",
        })
        .bind(match input.status {
            domain::ExtensionInstallationStatus::Installed => "available",
            domain::ExtensionInstallationStatus::Missing => "artifact_missing",
        })
        .bind(input.is_current)
        .execute(&mut *transaction)
        .await?;

        let record = find_joined_by_id(&mut transaction, &input.node_id, installation_id)
            .await?
            .ok_or_else(|| anyhow!("upserted extension installation artifact is missing"))?;
        transaction.commit().await?;
        Ok(record)
    }

    async fn find_extension_installation_by_id(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let query = format!(
            r#"
            select {JOINED_COLUMNS}
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id
            where artifact.node_id = $1 and installation.id = $2
            "#
        );
        sqlx::query(&query)
            .bind(node_id)
            .bind(installation_id)
            .fetch_optional(self.pool())
            .await?
            .map(map_row)
            .transpose()
    }

    async fn find_extension_installation(
        &self,
        node_id: &str,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let query = format!(
            r#"
            select {JOINED_COLUMNS}
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id
            where artifact.node_id = $1
              and installation.category = $2
              and installation.organization = $3
              and installation.artifact_id = $4
              and installation.artifact_version = $5
            "#
        );
        sqlx::query(&query)
            .bind(node_id)
            .bind(identity.category.as_str())
            .bind(&identity.organization)
            .bind(&identity.artifact_id)
            .bind(&identity.version)
            .fetch_optional(self.pool())
            .await?
            .map(map_row)
            .transpose()
    }

    async fn list_extension_installations_for_node(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::ExtensionInstallationRecord>> {
        let query = format!(
            r#"
            select {JOINED_COLUMNS}
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id
            where artifact.node_id = $1
            order by installation.updated_at desc, installation.id desc
            "#
        );
        sqlx::query(&query)
            .bind(node_id)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_row)
            .collect()
    }

    async fn set_extension_installation_status(
        &self,
        node_id: &str,
        installation_id: Uuid,
        status: domain::ExtensionInstallationStatus,
    ) -> Result<()> {
        let mut transaction = self.pool().begin().await?;
        let target = find_joined_by_id(&mut transaction, node_id, installation_id).await?;
        let Some(target) = target else {
            transaction.rollback().await?;
            return Ok(());
        };
        let becomes_missing = status == domain::ExtensionInstallationStatus::Missing;
        sqlx::query(
            r#"
            update extension_artifact_instances
            set artifact_status = $2,
                availability_status = $3,
                is_current = case when $2 = 'missing' then false else is_current end,
                checked_at = now()
            where installation_id = $1
              and node_id = $4
            "#,
        )
        .bind(installation_id)
        .bind(if becomes_missing { "missing" } else { "ready" })
        .bind(if becomes_missing {
            "artifact_missing"
        } else {
            "available"
        })
        .bind(node_id)
        .execute(&mut *transaction)
        .await?;
        if becomes_missing && target.is_current {
            select_newest_remaining_current(&mut transaction, &target, installation_id).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn select_current_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let mut transaction = self.pool().begin().await?;
        let Some(target) = find_joined_by_id(&mut transaction, node_id, installation_id).await?
        else {
            transaction.rollback().await?;
            return Ok(None);
        };
        if target.status != domain::ExtensionInstallationStatus::Installed {
            transaction.rollback().await?;
            return Ok(None);
        }
        demote_current_family_artifact(&mut transaction, node_id, &target.identity).await?;
        sqlx::query(
            r#"
            update extension_artifact_instances
            set is_current = true, checked_at = now()
            where node_id = $1 and installation_id = $2
            "#,
        )
        .bind(node_id)
        .bind(installation_id)
        .execute(&mut *transaction)
        .await?;
        let selected = find_joined_by_id(&mut transaction, node_id, installation_id).await?;
        transaction.commit().await?;
        Ok(selected)
    }

    async fn extension_deletion_decision(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionDeletionDecision>> {
        let row = sqlx::query(DELETION_DECISION_QUERY)
            .bind(node_id)
            .bind(installation_id)
            .fetch_optional(self.pool())
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(deletion_decision_from_row(&row)?))
    }

    async fn remove_extension_installation(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let mut transaction = self.pool().begin().await?;
        let locked_query =
            format!("{DELETION_DECISION_QUERY} for update of installation, artifact");
        let decision_row = sqlx::query(&locked_query)
            .bind(node_id)
            .bind(installation_id)
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(decision_row) = decision_row else {
            transaction.rollback().await?;
            return Ok(None);
        };
        let decision = deletion_decision_from_row(&decision_row)?;
        if !decision.deletable {
            transaction.rollback().await?;
            return Err(
                ControlPlaneError::Conflict(deletion_conflict_code(&decision.reasons)).into(),
            );
        }
        let Some(target) = find_joined_by_id(&mut transaction, node_id, installation_id).await?
        else {
            transaction.rollback().await?;
            return Ok(None);
        };
        sqlx::query(
            "delete from extension_artifact_instances where node_id = $1 and installation_id = $2",
        )
        .bind(node_id)
        .bind(installation_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(target))
    }
}

fn deletion_decision_from_row(row: &PgRow) -> Result<domain::ExtensionDeletionDecision> {
    let checks = [
        (
            "system_reserved",
            row.try_get::<bool, _>("is_system_reserved")?,
        ),
        ("current_version", row.try_get::<bool, _>("is_current")?),
        (
            "workspace_assignment",
            row.try_get::<bool, _>("has_assignment")?,
        ),
        ("active_task", row.try_get::<bool, _>("has_active_task")?),
        (
            "active_worker",
            row.try_get::<bool, _>("has_active_worker")?,
        ),
        (
            "model_provider_instance",
            row.try_get::<bool, _>("has_model_provider_instance")?,
        ),
        (
            "model_provider_preview",
            row.try_get::<bool, _>("has_model_provider_preview")?,
        ),
        (
            "data_source_instance",
            row.try_get::<bool, _>("has_data_source_instance")?,
        ),
        (
            "host_infrastructure_config",
            row.try_get::<bool, _>("has_host_infrastructure_config")?,
        ),
        (
            "application_js_selection",
            row.try_get::<bool, _>("has_application_js_selection")?,
        ),
        (
            "application_source",
            row.try_get::<bool, _>("has_application_source")?,
        ),
        ("mcp_import", row.try_get::<bool, _>("has_mcp_import")?),
    ];
    let reasons = checks
        .into_iter()
        .filter_map(|(reason, blocked)| blocked.then(|| reason.to_string()))
        .collect::<Vec<_>>();
    Ok(domain::ExtensionDeletionDecision {
        deletable: reasons.is_empty(),
        reasons,
    })
}

fn deletion_conflict_code(reasons: &[String]) -> &'static str {
    match reasons.first().map(String::as_str) {
        Some("system_reserved") => "extension_system_reserved",
        Some("current_version") => "extension_current_version",
        Some("workspace_assignment") => "extension_workspace_assignment",
        Some("active_task") => "extension_active_task",
        Some("active_worker") => "extension_active_worker",
        Some("model_provider_instance") => "extension_model_provider_instance",
        Some("model_provider_preview") => "extension_model_provider_preview",
        Some("data_source_instance") => "extension_data_source_instance",
        Some("host_infrastructure_config") => "extension_host_infrastructure_config",
        Some("application_js_selection") => "extension_application_js_selection",
        Some("application_source") => "extension_application_source",
        Some("mcp_import") => "extension_mcp_import",
        _ => "extension_installation_delete_blocked",
    }
}

async fn demote_current_family_artifact(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node_id: &str,
    identity: &domain::ExtensionInstallationIdentity,
) -> Result<()> {
    sqlx::query(
        r#"
        update extension_artifact_instances artifact
        set is_current = false, checked_at = now()
        from extension_installations installation
        where installation.id = artifact.installation_id
          and artifact.node_id = $1
          and installation.category = $2
          and installation.organization = $3
          and installation.artifact_id = $4
          and artifact.is_current
        "#,
    )
    .bind(node_id)
    .bind(identity.category.as_str())
    .bind(&identity.organization)
    .bind(&identity.artifact_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn select_newest_remaining_current(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target: &domain::ExtensionInstallationRecord,
    excluded_installation_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"
        update extension_artifact_instances
        set is_current = true, checked_at = now()
        where node_id = $1 and installation_id = (
            select installation.id
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id
             and artifact.node_id = $1
            where installation.category = $2
              and installation.organization = $3
              and installation.artifact_id = $4
              and installation.id <> $5
              and artifact.artifact_status = 'ready'
            order by installation.updated_at desc, installation.id desc
            limit 1
        )
        "#,
    )
    .bind(&target.node_id)
    .bind(target.identity.category.as_str())
    .bind(&target.identity.organization)
    .bind(&target.identity.artifact_id)
    .bind(excluded_installation_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn find_joined_by_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node_id: &str,
    installation_id: Uuid,
) -> Result<Option<domain::ExtensionInstallationRecord>> {
    let query = format!(
        r#"
        select {JOINED_COLUMNS}
        from extension_installations installation
        join extension_artifact_instances artifact
          on artifact.installation_id = installation.id
        where artifact.node_id = $1 and installation.id = $2
        "#
    );
    sqlx::query(&query)
        .bind(node_id)
        .bind(installation_id)
        .fetch_optional(&mut **transaction)
        .await?
        .map(map_row)
        .transpose()
}

fn map_row(row: PgRow) -> Result<domain::ExtensionInstallationRecord> {
    to_extension_installation_record(StoredExtensionInstallationRow {
        id: row.try_get("id")?,
        category: row.try_get("category")?,
        organization: row.try_get("organization")?,
        artifact_id: row.try_get("artifact_id")?,
        artifact_version: row.try_get("artifact_version")?,
        source_kind: row.try_get("source_kind")?,
        trust_level: row.try_get("trust_level")?,
        expected_checksum: row.try_get("expected_checksum")?,
        signature_status: row.try_get("signature_status")?,
        signature_algorithm: row.try_get("signature_algorithm")?,
        signing_key_id: row.try_get("signing_key_id")?,
        warnings: row.try_get("warnings")?,
        receipt: row.try_get("receipt")?,
        application_action: row.try_get("application_action")?,
        is_system_reserved: row.try_get("is_system_reserved")?,
        created_by: row.try_get("created_by")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        node_id: row.try_get("node_id")?,
        local_path: row.try_get("local_path")?,
        local_checksum: row.try_get("local_checksum")?,
        status: row.try_get("status")?,
        is_current: row.try_get("is_current")?,
    })
}
