pub(crate) mod plugin_components;

use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::{
    FrontendBlockCatalogDefaultTemplate, FrontendBlockCatalogRepository,
    FrontendBlockCatalogTemplateRevision, ReplaceInstallationFrontendBlocksInput,
    RetainedFrontendModuleAsset,
};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::frontend_block_catalog_mapper::{
        PgFrontendBlockCatalogMapper, StoredFrontendBlockCatalogRow,
    },
    repositories::PgControlPlaneStore,
};

fn map_catalog_row(row: sqlx::postgres::PgRow) -> Result<domain::FrontendBlockCatalogEntry> {
    PgFrontendBlockCatalogMapper::to_catalog_entry(StoredFrontendBlockCatalogRow {
        installation_id: row.get("installation_id"),
        provider_code: row.get("provider_code"),
        plugin_id: row.get("plugin_id"),
        plugin_version: row.get("plugin_version"),
        contribution_code: row.get("contribution_code"),
        title: row.get("title"),
        runtime: row.get("runtime"),
        entry: row.get("entry"),
        ui_registration: row.get("ui_registration"),
        code_template: row.get("code_template"),
        code_template_version: row.get("code_template_version"),
        code_template_language: row.get("code_template_language"),
        code_modules: row.get("code_modules"),
        context_contract: row.get("context_contract"),
        permission_network: row.get("permission_network"),
        permission_storage: row.get("permission_storage"),
        permission_secrets: row.get("permission_secrets"),
        ui_capabilities: row.get("ui_capabilities"),
    })
}

#[async_trait]
impl FrontendBlockCatalogRepository for PgControlPlaneStore {
    async fn replace_installation_frontend_blocks(
        &self,
        input: &ReplaceInstallationFrontendBlocksInput,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"
            delete from frontend_block_catalog
            where installation_id = $1
            "#,
        )
        .bind(input.installation_id)
        .execute(&mut *tx)
        .await?;

        for entry in &input.entries {
            let context_contract = serde_json::json!({
                "primitives": entry.context_contract.primitives,
                "input_schema": entry.context_contract.input_schema,
            });
            sqlx::query(
                r#"
                insert into frontend_block_catalog (
                    id,
                    scope_id,
                    installation_id,
                    provider_code,
                    plugin_id,
                    plugin_version,
                    contribution_code,
                    title,
                    runtime,
                    entry,
                    code_template,
                    code_template_version,
                    code_template_language,
                    code_modules,
                    context_contract,
                    permission_network,
                    permission_storage,
                    permission_secrets,
                    ui_capabilities, ui_registration
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
                )
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(domain::SYSTEM_SCOPE_ID)
            .bind(input.installation_id)
            .bind(&input.provider_code)
            .bind(&input.plugin_id)
            .bind(&input.plugin_version)
            .bind(&entry.contribution_code)
            .bind(&entry.title)
            .bind(&entry.runtime)
            .bind(&entry.entry)
            .bind(&entry.code_template)
            .bind(&entry.code_template_version)
            .bind(&entry.code_template_language)
            .bind(serde_json::to_value(&entry.code_modules)?)
            .bind(context_contract)
            .bind(&entry.permissions.network)
            .bind(&entry.permissions.storage)
            .bind(&entry.permissions.secrets)
            .bind(serde_json::to_value(&entry.ui_capabilities)?)
            .bind(&entry.ui_registration)
            .execute(&mut *tx)
            .await?;
        }

        let actor = sqlx::query_scalar(
            "select coalesce(updated_by, created_by) from extension_installations where id=$1",
        )
        .bind(input.installation_id)
        .fetch_one(&mut *tx)
        .await?;
        plugin_components::sync_plugin_components(&mut tx, input.installation_id, input, actor)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn list_workspace_frontend_blocks(
        &self,
        node_id: &str,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
        let rows = sqlx::query(
            r#"
            select
                reg.installation_id,
                reg.provider_code,
                reg.plugin_id,
                reg.plugin_version,
                reg.contribution_code,
                reg.title,
                reg.runtime,
                reg.entry,
                reg.code_template,
                reg.code_template_version,
                reg.code_template_language,
                reg.code_modules,
                reg.context_contract,
                reg.permission_network,
                reg.permission_storage,
                reg.permission_secrets,
                reg.ui_capabilities, reg.ui_registration
            from frontend_block_catalog reg
            inner join extension_installations installation
                on installation.id = reg.installation_id
            inner join extension_artifact_instances artifact
                on artifact.installation_id = installation.id
               and artifact.node_id = $1
            left join plugin_assignments pa
                on pa.workspace_id = $2
               and pa.installation_id = reg.installation_id
            where (installation.source_kind = 'builtin'
               or pa.installation_id is not null)
              and installation.verification_status = 'valid'
              and artifact.artifact_status = 'ready'
              and artifact.availability_status = 'available'
            order by reg.title asc, reg.contribution_code asc
            "#,
        )
        .bind(node_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_catalog_row).collect()
    }

    async fn list_system_frontend_blocks(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
        let rows = sqlx::query(
            r#"
            select reg.installation_id, reg.provider_code, reg.plugin_id, reg.plugin_version,
                reg.contribution_code, reg.title, reg.runtime, reg.entry, reg.code_template,
                reg.code_template_version, reg.code_template_language, reg.code_modules, reg.ui_registration,
                reg.context_contract, reg.permission_network, reg.permission_storage,
                reg.permission_secrets, reg.ui_capabilities
            from frontend_block_catalog reg
            inner join extension_installations installation on installation.id = reg.installation_id
            inner join extension_artifact_instances artifact
                on artifact.installation_id = installation.id and artifact.node_id = $1
            where installation.verification_status = 'valid'
              and artifact.artifact_status = 'ready'
              and artifact.availability_status = 'available'
              and artifact.is_current = true
            order by reg.title asc, reg.contribution_code asc
            "#,
        )
        .bind(node_id)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(map_catalog_row).collect()
    }

    async fn get_retained_frontend_module_asset(
        &self,
        workspace_id: Uuid,
        sha256: &str,
    ) -> Result<Option<RetainedFrontendModuleAsset>> {
        let row = sqlx::query(
            r#"
            select retained.sha256, retained.media_type, retained.bytes
            from retained_frontend_module_assets retained
            inner join extension_installations installation
                on installation.id = retained.installation_id
            where retained.sha256 = $2
              and installation.verification_status = 'valid'
              and (
                  installation.source_kind = 'builtin'
                  or exists (
                      select 1
                      from plugin_assignments assignment
                      where assignment.installation_id = retained.installation_id
                        and assignment.workspace_id = $1
                  )
              )
            limit 1
            "#,
        )
        .bind(workspace_id)
        .bind(sha256)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| RetainedFrontendModuleAsset {
            sha256: row.get("sha256"),
            media_type: row.get("media_type"),
            bytes: row.get("bytes"),
        }))
    }

    async fn list_active_ui_code_templates_for_catalog(
        &self,
    ) -> Result<Vec<FrontendBlockCatalogDefaultTemplate>> {
        // Fix the narrow eligible set before PostgreSQL can project published bodies at a join input.
        sqlx::query(
            r#"with catalog_templates as materialized (
                select t.id, t.name, t.provider_code, t.contribution_code
                from ui_code_templates t
                join ui_code_template_defaults defaults on defaults.template_id = t.id
                join ui_code_template_revisions latest on latest.template_id = t.id and latest.is_latest
            )
            select t.provider_code, t.contribution_code, published.revision,
                published.source, published.language
            from catalog_templates t
            left join ui_code_template_revisions published on published.template_id = t.id and published.is_published
                and published.template_id = any(array(select id from catalog_templates))
            order by t.name, t.id"#,
        )
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(|row| {
            let published_revision = row.get::<Option<i32>, _>("revision").map(|revision| -> Result<_> {
                Ok(FrontendBlockCatalogTemplateRevision {
                    revision,
                    source: row.get("source"),
                    language: domain::UiCodeTemplateLanguage::try_from(row.get::<&str, _>("language"))?,
                })
            }).transpose()?;
            Ok(FrontendBlockCatalogDefaultTemplate {
                provider_code: row.get("provider_code"),
                contribution_code: row.get("contribution_code"),
                published_revision,
            })
        })
        .collect()
    }
}
