use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        CommitPluginFamilyUninstallInput, CommitPluginInstallationInput,
        CreatePluginAssignmentInput, CreatePluginTaskInput, PluginRepository,
        RecordPluginArtifactCleanupFailureInput, UpdatePluginDesiredStateInput,
        UpdatePluginTaskStatusInput, UpsertPluginArtifactInstanceInput,
        UpsertPluginInstallationInput, UpsertPluginPackageCatalogProjectionInput,
    },
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::plugin_mapper::{
        PgPluginMapper, StoredPluginArtifactCleanupRow, StoredPluginArtifactInstanceRow,
        StoredPluginAssignmentRow, StoredPluginInstallationRow,
        StoredPluginPackageCatalogProjectionRow, StoredPluginTaskRow,
    },
    repositories::PgControlPlaneStore,
};

pub(crate) fn map_installation(
    row: sqlx::postgres::PgRow,
) -> Result<domain::PluginInstallationRecord> {
    PgPluginMapper::to_installation_record(StoredPluginInstallationRow {
        id: row.get("id"),
        scope_id: row.get("scope_id"),
        category: row.get("category"),
        organization: row.get("organization"),
        provider_code: row.get("provider_code"),
        plugin_id: row.get("plugin_id"),
        plugin_version: row.get("plugin_version"),
        contract_version: row.get("contract_version"),
        protocol: row.get("protocol"),
        display_name: row.get("display_name"),
        source_kind: row.get("source_kind"),
        trust_level: row.get("trust_level"),
        verification_status: row.get("verification_status"),
        desired_state: row.get("desired_state"),
        expected_checksum: row.get("expected_checksum"),
        signature_status: row.get("signature_status"),
        signature_algorithm: row.get("signature_algorithm"),
        signing_key_id: row.get("signing_key_id"),
        legacy_manifest_compatibility: row.get("legacy_manifest_compatibility"),
        metadata_json: row.get("metadata_json"),
        is_system_reserved: row.get("is_system_reserved"),
        created_by: row.get("created_by"),
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_assignment(row: sqlx::postgres::PgRow) -> Result<domain::PluginAssignmentRecord> {
    PgPluginMapper::to_assignment_record(StoredPluginAssignmentRow {
        id: row.get("id"),
        installation_id: row.get("installation_id"),
        workspace_id: row.get("workspace_id"),
        provider_code: row.get("provider_code"),
        assigned_by: row.get("assigned_by"),
        created_at: row.get("created_at"),
    })
}

fn map_artifact_instance(
    row: sqlx::postgres::PgRow,
) -> Result<domain::PluginArtifactInstanceRecord> {
    PgPluginMapper::to_artifact_instance_record(StoredPluginArtifactInstanceRow {
        node_id: row.get("node_id"),
        installation_id: row.get("installation_id"),
        local_version: row.get("local_version"),
        local_checksum: row.get("local_checksum"),
        local_path: row.get("local_path"),
        package_path: row.get("package_path"),
        manifest_fingerprint: row.get("manifest_fingerprint"),
        artifact_status: row.get("artifact_status"),
        runtime_status: row.get("runtime_status"),
        availability_status: row.get("availability_status"),
        checked_at: row.get("checked_at"),
        last_error: row.get("last_error"),
        is_current: row.get("is_current"),
    })
}

fn map_artifact_cleanup(row: sqlx::postgres::PgRow) -> domain::PluginArtifactCleanupRecord {
    PgPluginMapper::to_artifact_cleanup_record(StoredPluginArtifactCleanupRow {
        id: row.get("id"),
        node_id: row.get("node_id"),
        provider_code: row.get("provider_code"),
        tombstone_path: row.get("tombstone_path"),
        created_at: row.get("created_at"),
        last_error: row.get("last_error"),
        last_attempt_at: row.get("last_attempt_at"),
    })
}

fn map_task(row: sqlx::postgres::PgRow) -> Result<domain::PluginTaskRecord> {
    PgPluginMapper::to_task_record(StoredPluginTaskRow {
        id: row.get("id"),
        installation_id: row.get("installation_id"),
        workspace_id: row.get("workspace_id"),
        provider_code: row.get("provider_code"),
        task_kind: row.get("task_kind"),
        status: row.get("status"),
        status_message: row.get("status_message"),
        detail_json: row.get("detail_json"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        finished_at: row.get("finished_at"),
    })
}

fn map_catalog_projection(
    row: sqlx::postgres::PgRow,
) -> Result<domain::PluginPackageCatalogProjectionRecord> {
    PgPluginMapper::to_package_catalog_projection_record(StoredPluginPackageCatalogProjectionRow {
        installation_id: row.get("installation_id"),
        package_code: row.get("package_code"),
        package_version: row.get("package_version"),
        catalog_snapshot_json: row.get("catalog_snapshot_json"),
        projection_status: row.get("projection_status"),
        last_error_message: row.get("last_error_message"),
        refreshed_at: row.get("refreshed_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait]
impl PluginRepository for PgControlPlaneStore {
    async fn commit_plugin_installation(
        &self,
        input: &CommitPluginInstallationInput,
    ) -> Result<domain::PluginInstallationRecord> {
        crate::plugin_installation_commit_repository::commit_plugin_installation(self, input).await
    }

    async fn upsert_installation(
        &self,
        input: &UpsertPluginInstallationInput,
    ) -> Result<domain::PluginInstallationRecord> {
        let row = sqlx::query(
            r#"
            insert into extension_installations (
                id,
                scope_id,
                category,
                organization,
                artifact_id,
                artifact_version,
                plugin_id,
                contract_version,
                protocol,
                display_name,
                source_kind,
                trust_level,
                verification_status,
                desired_state,
                expected_checksum,
                signature_status,
                signature_algorithm,
                signing_key_id,
                metadata_json,
                is_system_reserved,
                created_by,
                updated_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22
            )
            on conflict (plugin_id) where plugin_id is not null do update
            set
                scope_id = excluded.scope_id,
                category = excluded.category,
                organization = excluded.organization,
                artifact_id = excluded.artifact_id,
                artifact_version = excluded.artifact_version,
                contract_version = excluded.contract_version,
                protocol = excluded.protocol,
                display_name = excluded.display_name,
                source_kind = excluded.source_kind,
                trust_level = excluded.trust_level,
                verification_status = excluded.verification_status,
                -- Reinstalling a stable plugin identity must preserve its durable desired state.
                desired_state = extension_installations.desired_state,
                expected_checksum = excluded.expected_checksum,
                signature_status = excluded.signature_status,
                signature_algorithm = excluded.signature_algorithm,
                signing_key_id = excluded.signing_key_id,
                receipt = extension_installations.receipt - 'legacy_manifest_compatibility',
                metadata_json = excluded.metadata_json,
                is_system_reserved = excluded.is_system_reserved,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning
                id,
                scope_id,
                category,
                organization,
                artifact_id as provider_code,
                plugin_id,
                artifact_version as plugin_version,
                contract_version,
                protocol,
                display_name,
                source_kind,
                trust_level,
                verification_status,
                desired_state,
                expected_checksum,
                signature_status,
                signature_algorithm,
                signing_key_id,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility,
                metadata_json,
                is_system_reserved,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.installation_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(input.category.as_str())
        .bind(&input.organization)
        .bind(&input.provider_code)
        .bind(&input.plugin_version)
        .bind(&input.plugin_id)
        .bind(&input.contract_version)
        .bind(&input.protocol)
        .bind(&input.display_name)
        .bind(&input.source_kind)
        .bind(&input.trust_level)
        .bind(input.verification_status.as_str())
        .bind(input.desired_state.as_str())
        .bind(input.expected_checksum.as_deref())
        .bind(input.signature_status.as_str())
        .bind(input.signature_algorithm.as_deref())
        .bind(input.signing_key_id.as_deref())
        .bind(&input.metadata_json)
        .bind(input.is_system_reserved)
        .bind(input.actor_user_id)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_installation(row)
    }

    async fn get_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                scope_id,
                category,
                organization,
                artifact_id as provider_code,
                plugin_id,
                artifact_version as plugin_version,
                contract_version,
                protocol,
                display_name,
                source_kind,
                trust_level,
                verification_status,
                desired_state,
                expected_checksum,
                signature_status,
                signature_algorithm,
                signing_key_id,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility,
                metadata_json,
                is_system_reserved,
                created_by,
                updated_by,
                created_at,
                updated_at
            from extension_installations
            where id = $1
              and plugin_id is not null
            "#,
        )
        .bind(installation_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_installation).transpose()
    }

    async fn list_installations(&self) -> Result<Vec<domain::PluginInstallationRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                scope_id,
                category,
                organization,
                artifact_id as provider_code,
                plugin_id,
                artifact_version as plugin_version,
                contract_version,
                protocol,
                display_name,
                source_kind,
                trust_level,
                verification_status,
                desired_state,
                expected_checksum,
                signature_status,
                signature_algorithm,
                signing_key_id,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility,
                metadata_json,
                is_system_reserved,
                created_by,
                updated_by,
                created_at,
                updated_at
            from extension_installations
            where plugin_id is not null
            order by updated_at desc, id desc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_installation).collect()
    }

    async fn upsert_plugin_package_catalog_projection(
        &self,
        input: &UpsertPluginPackageCatalogProjectionInput,
    ) -> Result<domain::PluginPackageCatalogProjectionRecord> {
        let row = sqlx::query(
            r#"
            insert into plugin_package_catalog_projection (
                installation_id,
                package_code,
                package_version,
                catalog_snapshot_json,
                projection_status,
                last_error_message,
                refreshed_at
            ) values ($1, $2, $3, $4, $5, $6, $7)
            on conflict (installation_id) do update
            set package_code = excluded.package_code,
                package_version = excluded.package_version,
                catalog_snapshot_json = excluded.catalog_snapshot_json,
                projection_status = excluded.projection_status,
                last_error_message = excluded.last_error_message,
                refreshed_at = excluded.refreshed_at,
                updated_at = now()
            returning
                installation_id,
                package_code,
                package_version,
                catalog_snapshot_json,
                projection_status,
                last_error_message,
                refreshed_at,
                updated_at
            "#,
        )
        .bind(input.installation_id)
        .bind(&input.package_code)
        .bind(&input.package_version)
        .bind(&input.catalog_snapshot_json)
        .bind(input.projection_status.as_str())
        .bind(&input.last_error_message)
        .bind(input.refreshed_at)
        .fetch_one(self.pool())
        .await?;

        map_catalog_projection(row)
    }

    async fn get_plugin_package_catalog_projection(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginPackageCatalogProjectionRecord>> {
        let row = sqlx::query(
            r#"
            select
                installation_id,
                package_code,
                package_version,
                catalog_snapshot_json,
                projection_status,
                last_error_message,
                refreshed_at,
                updated_at
            from plugin_package_catalog_projection
            where installation_id = $1
            "#,
        )
        .bind(installation_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_catalog_projection).transpose()
    }

    async fn list_plugin_package_catalog_projections(
        &self,
    ) -> Result<Vec<domain::PluginPackageCatalogProjectionRecord>> {
        let rows = sqlx::query(
            r#"
            select
                installation_id,
                package_code,
                package_version,
                catalog_snapshot_json,
                projection_status,
                last_error_message,
                refreshed_at,
                updated_at
            from plugin_package_catalog_projection
            order by updated_at desc, installation_id desc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_catalog_projection).collect()
    }

    async fn delete_installation(&self, installation_id: Uuid) -> Result<()> {
        let deleted = sqlx::query_scalar::<_, Uuid>(
            r#"
            delete from extension_installations
            where id = $1 and plugin_id is not null
            returning id
            "#,
        )
        .bind(installation_id)
        .fetch_optional(self.pool())
        .await?;

        if deleted.is_some() {
            Ok(())
        } else {
            bail!(ControlPlaneError::NotFound("plugin_installation"));
        }
    }

    async fn list_pending_restart_host_extensions(
        &self,
    ) -> Result<Vec<domain::PluginInstallationRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                scope_id,
                category,
                organization,
                artifact_id as provider_code,
                plugin_id,
                artifact_version as plugin_version,
                contract_version,
                protocol,
                display_name,
                source_kind,
                trust_level,
                verification_status,
                desired_state,
                expected_checksum,
                signature_status,
                signature_algorithm,
                signing_key_id,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility,
                metadata_json,
                is_system_reserved,
                created_by,
                updated_by,
                created_at,
                updated_at
            from extension_installations
            where desired_state = 'pending_restart'
              and category = 'host-extensions'
            order by updated_at desc, id desc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_installation).collect()
    }

    async fn update_desired_state(
        &self,
        input: &UpdatePluginDesiredStateInput,
    ) -> Result<domain::PluginInstallationRecord> {
        let row = sqlx::query(
            r#"
            update extension_installations
            set
                desired_state = $2,
                updated_by = $3,
                updated_at = now()
            where id = $1 and plugin_id is not null
            returning
                id,
                scope_id,
                category,
                organization,
                artifact_id as provider_code,
                plugin_id,
                artifact_version as plugin_version,
                contract_version,
                protocol,
                display_name,
                source_kind,
                trust_level,
                verification_status,
                desired_state,
                expected_checksum,
                signature_status,
                signature_algorithm,
                signing_key_id,
                receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility,
                metadata_json,
                is_system_reserved,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.installation_id)
        .bind(input.desired_state.as_str())
        .bind(input.actor_user_id)
        .fetch_optional(self.pool())
        .await?;

        match row {
            Some(row) => map_installation(row),
            None => bail!(ControlPlaneError::NotFound("plugin_installation")),
        }
    }

    async fn upsert_artifact_instance(
        &self,
        input: &UpsertPluginArtifactInstanceInput,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        let row = sqlx::query(
            r#"
            insert into extension_artifact_instances (
                node_id,
                installation_id,
                local_version,
                local_checksum,
                local_path,
                package_path,
                manifest_fingerprint,
                artifact_status,
                runtime_status,
                availability_status,
                checked_at,
                last_error,
                is_current
            ) values (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13
            )
            on conflict (node_id, installation_id) do update
            set
                local_version = excluded.local_version,
                local_checksum = excluded.local_checksum,
                local_path = excluded.local_path,
                package_path = excluded.package_path,
                manifest_fingerprint = excluded.manifest_fingerprint,
                artifact_status = excluded.artifact_status,
                runtime_status = excluded.runtime_status,
                availability_status = excluded.availability_status,
                checked_at = excluded.checked_at,
                last_error = excluded.last_error,
                is_current = excluded.is_current
            returning
                node_id,
                installation_id,
                local_version,
                local_checksum,
                local_path,
                package_path,
                manifest_fingerprint,
                artifact_status,
                runtime_status,
                availability_status,
                checked_at,
                last_error,
                is_current
            "#,
        )
        .bind(&input.node_id)
        .bind(input.installation_id)
        .bind(input.local_version.as_deref())
        .bind(input.local_checksum.as_deref())
        .bind(input.local_path.as_deref())
        .bind(input.package_path.as_deref())
        .bind(input.manifest_fingerprint.as_deref())
        .bind(input.artifact_status.as_str())
        .bind(input.runtime_status.as_str())
        .bind(input.availability_status.as_str())
        .bind(input.checked_at)
        .bind(input.last_error.as_deref())
        .bind(input.is_current)
        .fetch_one(self.pool())
        .await?;

        map_artifact_instance(row)
    }

    async fn select_network_egress_current(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginArtifactInstanceRecord>> {
        let mut transaction = self.pool().begin().await?;
        let target = sqlx::query(
            r#"
            select installation.category, installation.artifact_id as provider_code
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id and artifact.node_id = $1
            where installation.id = $2
              and installation.contract_version = '1flowbase.network_egress_provider/v1'
              and installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
              and artifact.artifact_status = 'ready'
            for update of installation, artifact
            "#,
        )
        .bind(node_id)
        .bind(installation_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(target) = target else {
            transaction.rollback().await?;
            return Ok(None);
        };
        let category: String = target.get("category");
        let provider_code: String = target.get("provider_code");
        let family_lock = format!("network-egress-current:{category}:{provider_code}");
        sqlx::query("select pg_advisory_xact_lock(hashtext($1))")
            .bind(family_lock)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            r#"
            update extension_artifact_instances artifact
            set is_current = false, checked_at = now()
            from extension_installations installation
            where installation.id = artifact.installation_id
              and artifact.node_id = $1
              and installation.category = $2
              and installation.artifact_id = $3
              and installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
              and artifact.is_current
            "#,
        )
        .bind(node_id)
        .bind(&category)
        .bind(&provider_code)
        .execute(&mut *transaction)
        .await?;
        let artifact = sqlx::query(
            r#"
            update extension_artifact_instances
            set is_current = true, checked_at = now()
            where node_id = $1 and installation_id = $2
            returning node_id, installation_id, local_version, local_checksum, local_path,
                package_path, manifest_fingerprint, artifact_status, runtime_status,
                availability_status, checked_at, last_error, is_current
            "#,
        )
        .bind(node_id)
        .bind(installation_id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(map_artifact_instance(artifact)?))
    }

    async fn commit_plugin_family_uninstall(
        &self,
        input: &CommitPluginFamilyUninstallInput,
    ) -> Result<()> {
        let mut transaction = self.pool().begin().await?;
        for artifact in &input.artifact_instances {
            sqlx::query(
                r#"
                insert into extension_artifact_instances (
                    node_id,
                    installation_id,
                    local_version,
                    local_checksum,
                    local_path,
                    package_path,
                    manifest_fingerprint,
                    artifact_status,
                    runtime_status,
                    availability_status,
                    checked_at,
                    last_error,
                    is_current
                ) values (
                    $1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12, $13
                )
                on conflict (node_id, installation_id) do update
                set
                    local_version = excluded.local_version,
                    local_checksum = excluded.local_checksum,
                    local_path = excluded.local_path,
                    package_path = excluded.package_path,
                    manifest_fingerprint = excluded.manifest_fingerprint,
                    artifact_status = excluded.artifact_status,
                    runtime_status = excluded.runtime_status,
                    availability_status = excluded.availability_status,
                    checked_at = excluded.checked_at,
                    last_error = excluded.last_error,
                    is_current = excluded.is_current
                "#,
            )
            .bind(&artifact.node_id)
            .bind(artifact.installation_id)
            .bind(artifact.local_version.as_deref())
            .bind(artifact.local_checksum.as_deref())
            .bind(artifact.local_path.as_deref())
            .bind(artifact.package_path.as_deref())
            .bind(artifact.manifest_fingerprint.as_deref())
            .bind(artifact.artifact_status.as_str())
            .bind(artifact.runtime_status.as_str())
            .bind(artifact.availability_status.as_str())
            .bind(artifact.checked_at)
            .bind(artifact.last_error.as_deref())
            .bind(artifact.is_current)
            .execute(&mut *transaction)
            .await?;
        }
        for cleanup in &input.artifact_cleanups {
            sqlx::query(
                r#"
                insert into plugin_artifact_cleanup_jobs (
                    id,
                    node_id,
                    provider_code,
                    tombstone_path,
                    created_at
                ) values ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(cleanup.cleanup_id)
            .bind(&cleanup.node_id)
            .bind(&cleanup.provider_code)
            .bind(&cleanup.tombstone_path)
            .bind(cleanup.created_at)
            .execute(&mut *transaction)
            .await?;
        }
        let event = &input.audit_log;
        sqlx::query(
            r#"
            insert into audit_logs (
                id,
                workspace_id,
                scope_id,
                actor_user_id,
                target_type,
                target_id,
                event_code,
                payload,
                created_by,
                updated_by,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $4, $4, $9, $9)
            "#,
        )
        .bind(event.id)
        .bind(event.workspace_id)
        .bind(event.workspace_id.unwrap_or(domain::SYSTEM_SCOPE_ID))
        .bind(event.actor_user_id)
        .bind(&event.target_type)
        .bind(event.target_id)
        .bind(&event.event_code)
        .bind(&event.payload)
        .bind(event.created_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn list_plugin_artifact_cleanups(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::PluginArtifactCleanupRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                node_id,
                provider_code,
                tombstone_path,
                created_at,
                last_error,
                last_attempt_at
            from plugin_artifact_cleanup_jobs
            where node_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(node_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_artifact_cleanup).collect())
    }

    async fn complete_plugin_artifact_cleanup(&self, cleanup_id: Uuid) -> Result<()> {
        sqlx::query("delete from plugin_artifact_cleanup_jobs where id = $1")
            .bind(cleanup_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn record_plugin_artifact_cleanup_failure(
        &self,
        input: &RecordPluginArtifactCleanupFailureInput,
    ) -> Result<()> {
        sqlx::query(
            r#"
            update plugin_artifact_cleanup_jobs
            set last_error = $2, last_attempt_at = $3
            where id = $1
            "#,
        )
        .bind(input.cleanup_id)
        .bind(&input.last_error)
        .bind(input.attempted_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_artifact_instance(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginArtifactInstanceRecord>> {
        let row = sqlx::query(
            r#"
            select
                node_id,
                installation_id,
                local_version,
                local_checksum,
                local_path,
                package_path,
                manifest_fingerprint,
                artifact_status,
                runtime_status,
                availability_status,
                checked_at,
                last_error,
                is_current
            from extension_artifact_instances
            where node_id = $1 and installation_id = $2
            "#,
        )
        .bind(node_id)
        .bind(installation_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_artifact_instance).transpose()
    }

    async fn list_artifact_instances(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::PluginArtifactInstanceRecord>> {
        let rows = sqlx::query(
            r#"
            select
                node_id,
                installation_id,
                local_version,
                local_checksum,
                local_path,
                package_path,
                manifest_fingerprint,
                artifact_status,
                runtime_status,
                availability_status,
                checked_at,
                last_error,
                is_current
            from extension_artifact_instances
            join extension_installations
              on extension_installations.id = extension_artifact_instances.installation_id
            where node_id = $1
              and extension_installations.plugin_id is not null
            order by checked_at desc, installation_id desc
            "#,
        )
        .bind(node_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_artifact_instance).collect()
    }

    async fn create_assignment(
        &self,
        input: &CreatePluginAssignmentInput,
    ) -> Result<domain::PluginAssignmentRecord> {
        let row = sqlx::query(
            r#"
            insert into plugin_assignments (
                id,
                installation_id,
                workspace_id,
                provider_code,
                assigned_by
            ) values ($1, $2, $3, $4, $5)
            on conflict (workspace_id, provider_code) do update
            set
                installation_id = excluded.installation_id,
                assigned_by = excluded.assigned_by
            returning
                id,
                installation_id,
                workspace_id,
                provider_code,
                assigned_by,
                created_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.installation_id)
        .bind(input.workspace_id)
        .bind(&input.provider_code)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_assignment(row)
    }

    async fn list_assignments(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::PluginAssignmentRecord>> {
        let rows = sqlx::query(
            r#"
            select id, installation_id, workspace_id, provider_code, assigned_by, created_at
            from plugin_assignments
            where workspace_id = $1
            order by created_at desc, id desc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_assignment).collect()
    }

    async fn list_assigned_installation_ids(&self) -> Result<Vec<Uuid>> {
        let rows = sqlx::query_scalar(
            "select distinct installation_id from plugin_assignments order by installation_id",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    async fn create_task(&self, input: &CreatePluginTaskInput) -> Result<domain::PluginTaskRecord> {
        let row = sqlx::query(
            r#"
            insert into plugin_tasks (
                id,
                installation_id,
                workspace_id,
                scope_kind,
                scope_id,
                provider_code,
                task_kind,
                status,
                status_message,
                detail_json,
                created_by,
                updated_by
            ) values (
                $1,
                $2,
                $3,
                case when $5 in ('assign', 'unassign') then 'workspace' else 'system' end,
                case when $5 in ('assign', 'unassign') then $3 else $4 end,
                $6,
                $5,
                $7,
                $8,
                $9,
                $10,
                $10
            )
            returning
                id,
                installation_id,
                workspace_id,
                provider_code,
                task_kind,
                status,
                status_message,
                detail_json,
                created_by,
                created_at,
                updated_at,
                finished_at
            "#,
        )
        .bind(input.task_id)
        .bind(input.installation_id)
        .bind(input.workspace_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(input.task_kind.as_str())
        .bind(&input.provider_code)
        .bind(input.status.as_str())
        .bind(input.status_message.as_deref())
        .bind(&input.detail_json)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_task(row)
    }

    async fn update_task_status(
        &self,
        input: &UpdatePluginTaskStatusInput,
    ) -> Result<domain::PluginTaskRecord> {
        let row = sqlx::query(
            r#"
            update plugin_tasks
            set
                status = $2,
                status_message = $3,
                detail_json = $4,
                updated_at = now(),
                finished_at = case
                    when $2 in ('succeeded', 'failed', 'canceled', 'timed_out')
                        then coalesce(finished_at, now())
                    else null
                end
            where id = $1
            returning
                id,
                installation_id,
                workspace_id,
                provider_code,
                task_kind,
                status,
                status_message,
                detail_json,
                created_by,
                created_at,
                updated_at,
                finished_at
            "#,
        )
        .bind(input.task_id)
        .bind(input.status.as_str())
        .bind(input.status_message.as_deref())
        .bind(&input.detail_json)
        .fetch_optional(self.pool())
        .await?;

        match row {
            Some(row) => map_task(row),
            None => bail!(ControlPlaneError::NotFound("plugin_task")),
        }
    }

    async fn get_task(&self, task_id: Uuid) -> Result<Option<domain::PluginTaskRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                installation_id,
                workspace_id,
                provider_code,
                task_kind,
                status,
                status_message,
                detail_json,
                created_by,
                created_at,
                updated_at,
                finished_at
            from plugin_tasks
            where id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_task).transpose()
    }

    async fn list_tasks(&self) -> Result<Vec<domain::PluginTaskRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                installation_id,
                workspace_id,
                provider_code,
                task_kind,
                status,
                status_message,
                detail_json,
                created_by,
                created_at,
                updated_at,
                finished_at
            from plugin_tasks
            order by created_at desc, id desc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_task).collect()
    }
}
