use anyhow::Result;
use async_trait::async_trait;
use control_plane::ports::{NodeContributionRepository, ReplaceInstallationNodeContributionsInput};
use domain::NodeContributionDependencyStatus;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    mappers::node_contribution_mapper::{
        PgNodeContributionMapper, StoredNodeContributionRegistryRow,
    },
    repositories::PgControlPlaneStore,
};

fn map_registry_row(
    row: sqlx::postgres::PgRow,
    dependency_status: NodeContributionDependencyStatus,
) -> Result<domain::NodeContributionRegistryEntry> {
    PgNodeContributionMapper::to_registry_entry(StoredNodeContributionRegistryRow {
        installation_id: row.get("installation_id"),
        provider_code: row.get("provider_code"),
        plugin_unique_identifier: row.get("plugin_unique_identifier"),
        package_id: row.get("package_id"),
        plugin_id: row.get("plugin_id"),
        plugin_version: row.get("plugin_version"),
        contribution_code: row.get("contribution_code"),
        node_shell: row.get("node_shell"),
        category: row.get("category"),
        title: row.get("title"),
        description: row.get("description"),
        icon: row.get("icon"),
        schema_ui: row.get("schema_ui"),
        schema_version: row.get("schema_version"),
        output_schema: row.get("output_schema"),
        contribution_checksum: row.get("contribution_checksum"),
        compiled_contribution_hash: row.get("compiled_contribution_hash"),
        output_schema_snapshot: row.get("output_schema_snapshot"),
        side_effect_policy: row.get("side_effect_policy"),
        infra_contracts: row.get("infra_contracts"),
        required_auth: row.get("required_auth"),
        visibility: row.get("visibility"),
        experimental: row.get("experimental"),
        dependency_installation_kind: row.get("dependency_installation_kind"),
        dependency_plugin_version_range: row.get("dependency_plugin_version_range"),
        dependency_status: dependency_status.as_str().to_string(),
    })
}

#[async_trait]
impl NodeContributionRepository for PgControlPlaneStore {
    async fn replace_installation_node_contributions(
        &self,
        input: &ReplaceInstallationNodeContributionsInput,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"
            delete from node_contribution_registry
            where installation_id = $1
            "#,
        )
        .bind(input.installation_id)
        .execute(&mut *tx)
        .await?;

        for entry in &input.entries {
            sqlx::query(
                r#"
                insert into node_contribution_registry (
                    id,
                    scope_id,
                    installation_id,
                    provider_code,
                    plugin_unique_identifier,
                    package_id,
                    plugin_id,
                    plugin_version,
                    contribution_code,
                    node_shell,
                    category,
                    title,
                    description,
                    icon,
                    schema_ui,
                    schema_version,
                    output_schema,
                    contribution_checksum,
                    compiled_contribution_hash,
                    output_schema_snapshot,
                    side_effect_policy,
                    infra_contracts,
                    required_auth,
                    visibility,
                    experimental,
                    dependency_installation_kind,
                    dependency_plugin_version_range
                ) values (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                    $18, $19, $20, $21, $22, $23, $24, $25, $26, $27
                )
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(domain::SYSTEM_SCOPE_ID)
            .bind(input.installation_id)
            .bind(&input.provider_code)
            .bind(&entry.plugin_unique_identifier)
            .bind(&entry.package_id)
            .bind(&input.plugin_id)
            .bind(&input.plugin_version)
            .bind(&entry.contribution_code)
            .bind(&entry.node_shell)
            .bind(&entry.category)
            .bind(&entry.title)
            .bind(&entry.description)
            .bind(&entry.icon)
            .bind(&entry.schema_ui)
            .bind(&entry.schema_version)
            .bind(&entry.output_schema)
            .bind(&entry.contribution_checksum)
            .bind(&entry.compiled_contribution_hash)
            .bind(&entry.output_schema_snapshot)
            .bind(&entry.side_effect_policy)
            .bind(serde_json::to_value(&entry.infra_contracts)?)
            .bind(serde_json::to_value(&entry.required_auth)?)
            .bind(&entry.visibility)
            .bind(entry.experimental)
            .bind(&entry.dependency_installation_kind)
            .bind(&entry.dependency_plugin_version_range)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_node_contributions(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::NodeContributionRegistryEntry>> {
        let rows = sqlx::query(
            r#"
            select
                reg.installation_id,
                reg.provider_code,
                reg.plugin_unique_identifier,
                reg.package_id,
                reg.plugin_id,
                reg.plugin_version,
                reg.contribution_code,
                reg.node_shell,
                reg.category,
                reg.title,
                reg.description,
                reg.icon,
                reg.schema_ui,
                reg.schema_version,
                reg.output_schema,
                reg.contribution_checksum,
                reg.compiled_contribution_hash,
                reg.output_schema_snapshot,
                reg.side_effect_policy,
                reg.infra_contracts,
                reg.required_auth,
                reg.visibility,
                reg.experimental,
                reg.dependency_installation_kind,
                reg.dependency_plugin_version_range,
                assigned.id as assigned_installation_id,
                installed.desired_state as installed_desired_state
            from node_contribution_registry reg
            left join plugin_assignments pa
                on pa.workspace_id = $1
               and pa.installation_id = reg.installation_id
            left join extension_installations assigned
                on assigned.id = pa.installation_id
            left join extension_installations installed
               on installed.id = reg.installation_id
               and installed.plugin_id = reg.package_id
               and installed.artifact_version = reg.plugin_version
               and installed.contract_version = '1flowbase.capability/v1'
            where reg.schema_version = '1flowbase.node-contribution/v2'
            order by reg.category asc, reg.title asc, reg.contribution_code asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                let assigned_installation_id: Option<Uuid> = row.get("assigned_installation_id");
                let installed_desired_state: Option<String> = row.get("installed_desired_state");
                let dependency_status =
                    if assigned_installation_id.is_none() || installed_desired_state.is_none() {
                        NodeContributionDependencyStatus::MissingPlugin
                    } else if installed_desired_state.as_deref() == Some("disabled") {
                        NodeContributionDependencyStatus::DisabledPlugin
                    } else {
                        NodeContributionDependencyStatus::Ready
                    };
                map_registry_row(row, dependency_status)
            })
            .collect()
    }
}
