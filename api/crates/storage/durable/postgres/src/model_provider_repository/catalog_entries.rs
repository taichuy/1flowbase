use super::*;

impl PgControlPlaneStore {
    pub(super) async fn create_catalog_source_impl(
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

    pub(super) async fn create_catalog_sync_run_impl(
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

    pub(super) async fn upsert_catalog_entry_impl(
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

    pub(super) async fn list_catalog_entries_impl(
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

    pub(super) async fn list_catalog_entries_for_provider_instance_impl(
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
}
