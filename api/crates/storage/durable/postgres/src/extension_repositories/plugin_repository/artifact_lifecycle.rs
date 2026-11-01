use super::*;

impl PgControlPlaneStore {
    pub(super) async fn upsert_artifact_instance_impl(
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

    pub(super) async fn select_network_egress_current_impl(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginArtifactInstanceRecord>> {
        let mut transaction = self.pool().begin().await?;
        let target = sqlx::query(
            r#"
            select installation.category, installation.organization,
                   installation.artifact_id as provider_code
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
        let organization: String = target.get("organization");
        let provider_code: String = target.get("provider_code");
        let family_lock =
            format!("network-egress-current:{category}:{organization}:{provider_code}");
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
              and installation.organization = $3
              and installation.artifact_id = $4
              and installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
              and artifact.is_current
            "#,
        )
        .bind(node_id)
        .bind(&category)
        .bind(&organization)
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

    pub(super) async fn commit_plugin_family_uninstall_impl(
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
        // Family uninstall retains installation/audit history but revokes reusable projections.
        let installation_ids: Vec<_> = input
            .artifact_instances
            .iter()
            .map(|artifact| artifact.installation_id)
            .collect();
        sqlx::query("delete from ui_component_records where origin='plugin' and owner_installation_id=any($1)")
            .bind(&installation_ids).execute(&mut *transaction).await?;
        sqlx::query("delete from frontend_block_catalog where installation_id=any($1)")
            .bind(&installation_ids)
            .execute(&mut *transaction)
            .await?;
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

    pub(super) async fn list_plugin_artifact_cleanups_impl(
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

    pub(super) async fn complete_plugin_artifact_cleanup_impl(
        &self,
        cleanup_id: Uuid,
    ) -> Result<()> {
        sqlx::query("delete from plugin_artifact_cleanup_jobs where id = $1")
            .bind(cleanup_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub(super) async fn record_plugin_artifact_cleanup_failure_impl(
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
}
