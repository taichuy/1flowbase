use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        CreateModelCatalogSyncRunInput, CreateModelFailoverQueueItemInput,
        CreateModelFailoverQueueSnapshotInput, CreateModelFailoverQueueTemplateInput,
        CreateModelProviderCatalogSourceInput, CreateModelProviderInstanceInput,
        CreateModelProviderPreviewSessionInput, ModelProviderRepository,
        ReassignModelProviderInstancesInput, UpdateModelProviderInstanceInput,
        UpsertModelProviderCatalogCacheInput, UpsertModelProviderCatalogEntryInput,
        UpsertModelProviderMainInstanceInput, UpsertModelProviderSecretInput,
    },
};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::model_provider_mapper::{
        PgModelProviderMapper, StoredModelProviderCatalogCacheRow, StoredModelProviderInstanceRow,
        StoredModelProviderMainInstanceRow, StoredModelProviderMainModelRoutingPolicyRow,
        StoredModelProviderPreviewSessionRow, StoredModelProviderSecretRow,
    },
    repositories::PgControlPlaneStore,
};

mod row_mappers;

use crate::secret_crypto::{decrypt_secret_json, encrypt_secret_json};
use row_mappers::{
    map_catalog_cache, map_catalog_entry, map_catalog_source, map_catalog_sync_run,
    map_failover_queue_item, map_failover_queue_snapshot, map_failover_queue_template,
    map_instance, map_main_instance, map_main_model_routing_policy, map_preview_session,
    map_secret,
};

async fn list_main_model_routing_policies(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    provider_code: &str,
) -> Result<Vec<domain::ModelProviderMainModelRoutingPolicyRecord>> {
    let rows = sqlx::query(
        r#"
        select
            workspace_id,
            provider_code,
            model_id,
            distribution_rule,
            provider_instance_ids,
            excluded_provider_instance_ids,
            created_by,
            updated_by,
            created_at,
            updated_at
        from model_provider_main_model_distribution_rules
        where workspace_id = $1
          and provider_code = $2
        order by model_id asc
        "#,
    )
    .bind(workspace_id)
    .bind(provider_code)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(map_main_model_routing_policy)
        .collect()
}

#[async_trait]
impl ModelProviderRepository for PgControlPlaneStore {
    async fn create_instance(
        &self,
        input: &CreateModelProviderInstanceInput,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        let row = sqlx::query(
            r#"
            insert into model_provider_instances (
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                coalesce(
                    $11,
                    (
                        select auto_include_new_instances
                        from model_provider_main_instances
                        where workspace_id = $2
                          and provider_code = $4
                    ),
                    $12
                ),
                $13,
                $13
            )
            returning
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.instance_id)
        .bind(input.workspace_id)
        .bind(input.installation_id)
        .bind(&input.provider_code)
        .bind(&input.protocol)
        .bind(&input.display_name)
        .bind(input.status.as_str())
        .bind(&input.config_json)
        .bind(serde_json::to_value(&input.configured_models)?)
        .bind(&input.enabled_model_ids)
        .bind(input.included_in_main)
        .bind(domain::DEFAULT_AUTO_INCLUDE_NEW_PROVIDER_INSTANCES)
        .bind(input.created_by)
        .fetch_one(self.pool())
        .await?;

        map_instance(row)
    }

    async fn update_instance(
        &self,
        input: &UpdateModelProviderInstanceInput,
    ) -> Result<domain::ModelProviderInstanceRecord> {
        let row = sqlx::query(
            r#"
            update model_provider_instances
            set
                display_name = $3,
                status = $4,
                config_json = $5,
                configured_models_json = $6,
                enabled_model_ids = $7,
                included_in_main = $8,
                updated_by = $9,
                updated_at = now()
            where workspace_id = $1
              and id = $2
            returning
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.instance_id)
        .bind(&input.display_name)
        .bind(input.status.as_str())
        .bind(&input.config_json)
        .bind(serde_json::to_value(&input.configured_models)?)
        .bind(&input.enabled_model_ids)
        .bind(input.included_in_main)
        .bind(input.updated_by)
        .fetch_optional(self.pool())
        .await?;

        match row {
            Some(row) => map_instance(row),
            None => bail!(ControlPlaneError::NotFound("model_provider_instance")),
        }
    }

    async fn get_instance(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderInstanceRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by,
                created_at,
                updated_at
            from model_provider_instances
            where workspace_id = $1
              and id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_instance).transpose()
    }

    async fn list_instances(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by,
                created_at,
                updated_at
            from model_provider_instances
            where workspace_id = $1
            order by updated_at desc, id desc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_instance).collect()
    }

    async fn list_instances_by_provider_code(
        &self,
        provider_code: &str,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by,
                created_at,
                updated_at
            from model_provider_instances
            where provider_code = $1
            order by updated_at desc, id desc
            "#,
        )
        .bind(provider_code)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_instance).collect()
    }

    async fn reassign_instances_to_installation(
        &self,
        input: &ReassignModelProviderInstancesInput,
    ) -> Result<Vec<domain::ModelProviderInstanceRecord>> {
        let rows = sqlx::query(
            r#"
            update model_provider_instances
            set
                installation_id = $3,
                protocol = $4,
                updated_by = $5,
                updated_at = now()
            where workspace_id = $1
              and provider_code = $2
            returning
                id,
                workspace_id,
                installation_id,
                provider_code,
                protocol,
                display_name,
                status,
                config_json,
                configured_models_json,
                enabled_model_ids,
                included_in_main,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.workspace_id)
        .bind(&input.provider_code)
        .bind(input.target_installation_id)
        .bind(&input.target_protocol)
        .bind(input.updated_by)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_instance).collect()
    }

    async fn upsert_catalog_cache(
        &self,
        input: &UpsertModelProviderCatalogCacheInput,
    ) -> Result<domain::ModelProviderCatalogCacheRecord> {
        let row = sqlx::query(
            r#"
            insert into provider_instance_model_catalog_cache (
                id,
                provider_instance_id,
                scope_id,
                model_discovery_mode,
                refresh_status,
                source,
                models_json,
                last_error_message,
                refreshed_at,
                created_by,
                updated_by
            )
            select
                $1,
                instances.id,
                instances.scope_id,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                instances.updated_by,
                instances.updated_by
            from model_provider_instances instances
            where instances.id = $8
            on conflict (provider_instance_id) do update
            set scope_id = excluded.scope_id,
                model_discovery_mode = excluded.model_discovery_mode,
                refresh_status = excluded.refresh_status,
                source = excluded.source,
                models_json = excluded.models_json,
                last_error_message = excluded.last_error_message,
                refreshed_at = excluded.refreshed_at,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning
                provider_instance_id,
                model_discovery_mode,
                refresh_status,
                source,
                models_json,
                last_error_message,
                refreshed_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.model_discovery_mode.as_str())
        .bind(input.refresh_status.as_str())
        .bind(input.source.as_str())
        .bind(&input.models_json)
        .bind(input.last_error_message.as_deref())
        .bind(input.refreshed_at)
        .bind(input.provider_instance_id)
        .fetch_one(self.pool())
        .await?;

        map_catalog_cache(row)
    }

    async fn get_catalog_cache(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderCatalogCacheRecord>> {
        let row = sqlx::query(
            r#"
            select
                provider_instance_id,
                model_discovery_mode,
                refresh_status,
                source,
                models_json,
                last_error_message,
                refreshed_at,
                updated_at
            from provider_instance_model_catalog_cache
            where provider_instance_id = $1
            "#,
        )
        .bind(provider_instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_catalog_cache).transpose()
    }

    async fn upsert_secret(
        &self,
        input: &UpsertModelProviderSecretInput,
    ) -> Result<domain::ModelProviderSecretRecord> {
        let encrypted_secret_json =
            encrypt_secret_json(&input.plaintext_secret_json, &input.master_key)?;
        let row = sqlx::query(
            r#"
            insert into model_provider_instance_secrets (
                id,
                provider_instance_id,
                scope_id,
                encrypted_secret_json,
                secret_version,
                created_by,
                updated_by
            )
            select
                $1,
                instances.id,
                instances.scope_id,
                $2,
                $3,
                instances.updated_by,
                instances.updated_by
            from model_provider_instances instances
            where instances.id = $4
            on conflict (provider_instance_id) do update
            set scope_id = excluded.scope_id,
                encrypted_secret_json = excluded.encrypted_secret_json,
                secret_version = excluded.secret_version,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning provider_instance_id, encrypted_secret_json, secret_version, updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(&encrypted_secret_json)
        .bind(input.secret_version)
        .bind(input.provider_instance_id)
        .fetch_one(self.pool())
        .await?;

        map_secret(row)
    }

    async fn upsert_main_instance(
        &self,
        input: &UpsertModelProviderMainInstanceInput,
    ) -> Result<domain::ModelProviderMainInstanceRecord> {
        let mut tx = self.pool().begin().await?;
        let main_instance = sqlx::query(
            r#"
            insert into model_provider_main_instances (
                id,
                workspace_id,
                provider_code,
                auto_include_new_instances,
                revision,
                created_by,
                updated_by
            )
            select $1, $2, $3, $4, 1, $5, $5
            where $6 = 0
               or exists (
                    select 1
                    from model_provider_main_instances existing
                    where existing.workspace_id = $2
                      and existing.provider_code = $3
               )
            on conflict (workspace_id, provider_code) do update
            set
                auto_include_new_instances = excluded.auto_include_new_instances,
                revision = model_provider_main_instances.revision + 1,
                updated_by = excluded.updated_by,
                updated_at = now()
            where model_provider_main_instances.revision = $6
            returning
                workspace_id,
                provider_code,
                auto_include_new_instances,
                revision,
                created_by,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(&input.provider_code)
        .bind(input.auto_include_new_instances)
        .bind(input.updated_by)
        .bind(input.expected_revision)
        .fetch_optional(&mut *tx)
        .await?;
        if main_instance.is_none() {
            return Err(
                ControlPlaneError::Conflict("model_provider_main_instance_revision").into(),
            );
        }

        if let Some(model_routing_policies) = input.model_routing_policies.as_ref() {
            sqlx::query(
                r#"
                delete from model_provider_main_model_distribution_rules
                where workspace_id = $1
                  and provider_code = $2
                "#,
            )
            .bind(input.workspace_id)
            .bind(&input.provider_code)
            .execute(&mut *tx)
            .await?;

            for policy in model_routing_policies {
                sqlx::query(
                    r#"
                    insert into model_provider_main_model_distribution_rules (
                        id,
                        workspace_id,
                        provider_code,
                        model_id,
                        distribution_rule,
                        provider_instance_ids,
                        excluded_provider_instance_ids,
                        created_by,
                        updated_by
                    ) values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
                    "#,
                )
                .bind(Uuid::now_v7())
                .bind(input.workspace_id)
                .bind(&input.provider_code)
                .bind(&policy.model_id)
                .bind(policy.distribution_rule.as_str())
                .bind(&policy.provider_instance_ids)
                .bind(&policy.excluded_provider_instance_ids)
                .bind(input.updated_by)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        self.get_main_instance(input.workspace_id, &input.provider_code)
            .await?
            .ok_or_else(|| anyhow!("model provider main instance was not returned after upsert"))
    }

    async fn get_main_instance(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
    ) -> Result<Option<domain::ModelProviderMainInstanceRecord>> {
        let row = sqlx::query(
            r#"
            select
                workspace_id,
                provider_code,
                auto_include_new_instances,
                revision,
                created_by,
                updated_by,
                created_at,
                updated_at
            from model_provider_main_instances
            where workspace_id = $1
              and provider_code = $2
            "#,
        )
        .bind(workspace_id)
        .bind(provider_code)
        .fetch_optional(self.pool())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let mut record = map_main_instance(row)?;
        record.model_routing_policies =
            list_main_model_routing_policies(self.pool(), workspace_id, provider_code).await?;
        Ok(Some(record))
    }

    async fn create_preview_session(
        &self,
        input: &CreateModelProviderPreviewSessionInput,
    ) -> Result<domain::ModelProviderPreviewSessionRecord> {
        let row = sqlx::query(
            r#"
            insert into model_provider_preview_sessions (
                id,
                workspace_id,
                actor_user_id,
                installation_id,
                instance_id,
                config_fingerprint,
                models_json,
                expires_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8)
            returning
                id,
                workspace_id,
                actor_user_id,
                installation_id,
                instance_id,
                config_fingerprint,
                models_json,
                expires_at,
                created_at
            "#,
        )
        .bind(input.session_id)
        .bind(input.workspace_id)
        .bind(input.actor_user_id)
        .bind(input.installation_id)
        .bind(input.instance_id)
        .bind(&input.config_fingerprint)
        .bind(&input.models_json)
        .bind(input.expires_at)
        .fetch_one(self.pool())
        .await?;

        map_preview_session(row)
    }

    async fn get_preview_session(
        &self,
        workspace_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<domain::ModelProviderPreviewSessionRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                actor_user_id,
                installation_id,
                instance_id,
                config_fingerprint,
                models_json,
                expires_at,
                created_at
            from model_provider_preview_sessions
            where workspace_id = $1
              and id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(session_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_preview_session).transpose()
    }

    async fn delete_preview_session(&self, workspace_id: Uuid, session_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            delete from model_provider_preview_sessions
            where workspace_id = $1
              and id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(session_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_secret_json(
        &self,
        provider_instance_id: Uuid,
        master_key: &str,
    ) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query(
            r#"
            select provider_instance_id, encrypted_secret_json, secret_version, updated_at
            from model_provider_instance_secrets
            where provider_instance_id = $1
            "#,
        )
        .bind(provider_instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(|row| -> Result<Value> {
            let record = map_secret(row)?;
            decrypt_secret_json(&record.encrypted_secret_json, master_key)
        })
        .transpose()
    }

    async fn get_secret_record(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Option<domain::ModelProviderSecretRecord>> {
        let row = sqlx::query(
            r#"
            select provider_instance_id, encrypted_secret_json, secret_version, updated_at
            from model_provider_instance_secrets
            where provider_instance_id = $1
            "#,
        )
        .bind(provider_instance_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_secret).transpose()
    }

    async fn delete_instance(&self, workspace_id: Uuid, instance_id: Uuid) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let deleted = sqlx::query_scalar::<_, String>(
            r#"
            delete from model_provider_instances
            where workspace_id = $1
              and id = $2
            returning provider_code
            "#,
        )
        .bind(workspace_id)
        .bind(instance_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(provider_code) = deleted else {
            bail!(ControlPlaneError::NotFound("model_provider_instance"));
        };

        let removed = sqlx::query(
            r#"
            update model_provider_main_model_distribution_rules
            set provider_instance_ids = array_remove(provider_instance_ids, $3),
                excluded_provider_instance_ids = array_remove(excluded_provider_instance_ids, $3),
                updated_at = now()
            where scope_id = $1
              and provider_code = $2
              and ($3 = any(provider_instance_ids)
                   or $3 = any(excluded_provider_instance_ids))
            "#,
        )
        .bind(workspace_id)
        .bind(&provider_code)
        .bind(instance_id)
        .execute(&mut *tx)
        .await?;

        if removed.rows_affected() > 0 {
            sqlx::query(
                r#"
                update model_provider_main_instances
                set revision = revision + 1,
                    updated_at = now()
                where workspace_id = $1
                  and provider_code = $2
                "#,
            )
            .bind(workspace_id)
            .bind(&provider_code)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn count_instance_references(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
    ) -> Result<u64> {
        let pattern = format!("%{instance_id}%");
        let count: i64 = sqlx::query_scalar(
            r#"
            select count(*)::bigint
            from (
                select 1
                from flow_drafts fd
                join flows f on f.id = fd.flow_id
                join applications a on a.id = f.application_id
                where a.workspace_id = $1
                  and fd.document::text like $2
                union all
                select 1
                from flow_versions fv
                join flows f on f.id = fv.flow_id
                join applications a on a.id = f.application_id
                where a.workspace_id = $1
                  and fv.document::text like $2
            ) refs
            "#,
        )
        .bind(workspace_id)
        .bind(pattern)
        .fetch_one(self.pool())
        .await?;

        Ok(count as u64)
    }

    async fn create_catalog_source(
        &self,
        input: &CreateModelProviderCatalogSourceInput,
    ) -> Result<domain::ModelProviderCatalogSourceRecord> {
        let row = sqlx::query(
            r#"
            insert into model_provider_catalog_sources (
                id,
                workspace_id,
                source_kind,
                plugin_id,
                provider_code,
                display_name,
                base_url_ref,
                auth_secret_ref,
                protocol,
                status
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            returning
                id,
                workspace_id,
                source_kind,
                plugin_id,
                provider_code,
                display_name,
                base_url_ref,
                auth_secret_ref,
                protocol,
                status,
                last_sync_run_id,
                created_at,
                updated_at
            "#,
        )
        .bind(input.source_id)
        .bind(input.workspace_id)
        .bind(&input.source_kind)
        .bind(&input.plugin_id)
        .bind(&input.provider_code)
        .bind(&input.display_name)
        .bind(input.base_url_ref.as_deref())
        .bind(input.auth_secret_ref.as_deref())
        .bind(&input.protocol)
        .bind(&input.status)
        .fetch_one(self.pool())
        .await?;

        map_catalog_source(row)
    }

    async fn create_catalog_sync_run(
        &self,
        input: &CreateModelCatalogSyncRunInput,
    ) -> Result<domain::ModelCatalogSyncRunRecord> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            insert into model_catalog_sync_runs (
                id,
                catalog_source_id,
                status,
                error_message_ref,
                discovered_count,
                imported_count,
                disabled_count,
                started_at,
                finished_at
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            returning
                id,
                catalog_source_id,
                status,
                error_message_ref,
                discovered_count,
                imported_count,
                disabled_count,
                started_at,
                finished_at
            "#,
        )
        .bind(input.sync_run_id)
        .bind(input.catalog_source_id)
        .bind(&input.status)
        .bind(input.error_message_ref.as_deref())
        .bind(input.discovered_count)
        .bind(input.imported_count)
        .bind(input.disabled_count)
        .bind(input.started_at)
        .bind(input.finished_at)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            update model_provider_catalog_sources
            set last_sync_run_id = $1,
                updated_at = now()
            where id = $2
            "#,
        )
        .bind(input.sync_run_id)
        .bind(input.catalog_source_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        map_catalog_sync_run(row)
    }

    async fn upsert_catalog_entry(
        &self,
        input: &UpsertModelProviderCatalogEntryInput,
    ) -> Result<domain::ModelProviderCatalogEntryRecord> {
        let row = sqlx::query(
            r#"
            insert into model_provider_catalog_entries (
                id,
                provider_instance_id,
                catalog_source_id,
                upstream_model_id,
                display_label,
                protocol,
                capability_snapshot,
                parameter_schema_ref,
                context_window,
                max_output_tokens,
                pricing_ref,
                status
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            on conflict (catalog_source_id, upstream_model_id, protocol) do update
            set
                provider_instance_id = excluded.provider_instance_id,
                display_label = excluded.display_label,
                capability_snapshot = excluded.capability_snapshot,
                parameter_schema_ref = excluded.parameter_schema_ref,
                context_window = excluded.context_window,
                max_output_tokens = excluded.max_output_tokens,
                pricing_ref = excluded.pricing_ref,
                fetched_at = now(),
                status = excluded.status
            returning
                id,
                provider_instance_id,
                catalog_source_id,
                upstream_model_id,
                display_label,
                protocol,
                capability_snapshot,
                parameter_schema_ref,
                context_window,
                max_output_tokens,
                pricing_ref,
                fetched_at,
                status
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.provider_instance_id)
        .bind(input.catalog_source_id)
        .bind(&input.upstream_model_id)
        .bind(&input.display_label)
        .bind(&input.protocol)
        .bind(&input.capability_snapshot)
        .bind(input.parameter_schema_ref.as_deref())
        .bind(input.context_window)
        .bind(input.max_output_tokens)
        .bind(input.pricing_ref.as_deref())
        .bind(&input.status)
        .fetch_one(self.pool())
        .await?;

        map_catalog_entry(row)
    }

    async fn list_catalog_entries(
        &self,
        catalog_source_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                provider_instance_id,
                catalog_source_id,
                upstream_model_id,
                display_label,
                protocol,
                capability_snapshot,
                parameter_schema_ref,
                context_window,
                max_output_tokens,
                pricing_ref,
                fetched_at,
                status
            from model_provider_catalog_entries
            where catalog_source_id = $1
            order by upstream_model_id asc, protocol asc
            "#,
        )
        .bind(catalog_source_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_catalog_entry).collect()
    }

    async fn list_catalog_entries_for_provider_instance(
        &self,
        provider_instance_id: Uuid,
    ) -> Result<Vec<domain::ModelProviderCatalogEntryRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                provider_instance_id,
                catalog_source_id,
                upstream_model_id,
                display_label,
                protocol,
                capability_snapshot,
                parameter_schema_ref,
                context_window,
                max_output_tokens,
                pricing_ref,
                fetched_at,
                status
            from model_provider_catalog_entries
            where provider_instance_id = $1
            order by upstream_model_id asc, protocol asc
            "#,
        )
        .bind(provider_instance_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_catalog_entry).collect()
    }

    async fn create_failover_queue_template(
        &self,
        input: &CreateModelFailoverQueueTemplateInput,
    ) -> Result<domain::ModelFailoverQueueTemplateRecord> {
        let row = sqlx::query(
            r#"
            insert into model_failover_queue_templates (
                id,
                workspace_id,
                name,
                version,
                status,
                created_by
            ) values ($1, $2, $3, $4, $5, $6)
            returning
                id,
                workspace_id,
                name,
                version,
                status,
                created_by,
                created_at,
                updated_at
            "#,
        )
        .bind(input.queue_template_id)
        .bind(input.workspace_id)
        .bind(&input.name)
        .bind(input.version)
        .bind(&input.status)
        .bind(input.created_by)
        .fetch_one(self.pool())
        .await?;

        map_failover_queue_template(row)
    }

    async fn get_failover_queue_template(
        &self,
        queue_template_id: Uuid,
    ) -> Result<Option<domain::ModelFailoverQueueTemplateRecord>> {
        let row = sqlx::query(
            r#"
            select
                id,
                workspace_id,
                name,
                version,
                status,
                created_by,
                created_at,
                updated_at
            from model_failover_queue_templates
            where id = $1
            "#,
        )
        .bind(queue_template_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_failover_queue_template).transpose()
    }

    async fn create_failover_queue_item(
        &self,
        input: &CreateModelFailoverQueueItemInput,
    ) -> Result<domain::ModelFailoverQueueItemRecord> {
        let row = sqlx::query(
            r#"
            with inserted as (
                insert into model_failover_queue_items (
                    id,
                    queue_template_id,
                    sort_index,
                    provider_instance_id,
                    provider_code,
                    upstream_model_id,
                    protocol,
                    enabled
                ) values ($1, $2, $3, $4, $5, $6, $7, $8)
                returning
                    id,
                    queue_template_id,
                    sort_index,
                    provider_instance_id,
                    provider_code,
                    upstream_model_id,
                    protocol,
                    enabled
            )
            select
                inserted.id,
                inserted.queue_template_id,
                inserted.sort_index,
                inserted.provider_instance_id,
                instances.display_name as provider_instance_display_name,
                inserted.provider_code,
                inserted.upstream_model_id,
                inserted.protocol,
                inserted.enabled
            from inserted
            join model_provider_instances instances
              on instances.id = inserted.provider_instance_id
            "#,
        )
        .bind(input.queue_item_id)
        .bind(input.queue_template_id)
        .bind(input.sort_index)
        .bind(input.provider_instance_id)
        .bind(&input.provider_code)
        .bind(&input.upstream_model_id)
        .bind(&input.protocol)
        .bind(input.enabled)
        .fetch_one(self.pool())
        .await?;

        map_failover_queue_item(row)
    }

    async fn list_failover_queue_items(
        &self,
        queue_template_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverQueueItemRecord>> {
        let rows = sqlx::query(
            r#"
            select
                items.id,
                items.queue_template_id,
                items.sort_index,
                items.provider_instance_id,
                instances.display_name as provider_instance_display_name,
                items.provider_code,
                items.upstream_model_id,
                items.protocol,
                items.enabled
            from model_failover_queue_items items
            join model_provider_instances instances
              on instances.id = items.provider_instance_id
            where items.queue_template_id = $1
            order by items.sort_index asc, items.id asc
            "#,
        )
        .bind(queue_template_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_failover_queue_item).collect()
    }

    async fn create_failover_queue_snapshot(
        &self,
        input: &CreateModelFailoverQueueSnapshotInput,
    ) -> Result<domain::ModelFailoverQueueSnapshotRecord> {
        let row = sqlx::query(
            r#"
            insert into model_failover_queue_snapshots (
                id,
                queue_template_id,
                scope_id,
                version,
                items,
                created_by,
                updated_by
            ) values (
                $1,
                $2,
                (select scope_id from model_failover_queue_templates where id = $2),
                $3,
                $4,
                (select created_by from model_failover_queue_templates where id = $2),
                (select created_by from model_failover_queue_templates where id = $2)
            )
            returning
                id,
                queue_template_id,
                version,
                items,
                created_at
            "#,
        )
        .bind(input.snapshot_id)
        .bind(input.queue_template_id)
        .bind(input.version)
        .bind(&input.items)
        .fetch_one(self.pool())
        .await?;

        map_failover_queue_snapshot(row)
    }

    async fn list_failover_queue_snapshots(
        &self,
        queue_template_id: Uuid,
    ) -> Result<Vec<domain::ModelFailoverQueueSnapshotRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                queue_template_id,
                version,
                items,
                created_at
            from model_failover_queue_snapshots
            where queue_template_id = $1
            order by created_at desc, id desc
            "#,
        )
        .bind(queue_template_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_failover_queue_snapshot).collect()
    }
}
