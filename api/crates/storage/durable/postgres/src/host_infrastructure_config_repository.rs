use anyhow::{bail, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{HostInfrastructureConfigRepository, UpsertHostInfrastructureProviderConfigInput},
};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

fn parse_status(raw: &str) -> Result<domain::HostInfrastructureConfigStatus> {
    match raw {
        "disabled" => Ok(domain::HostInfrastructureConfigStatus::Disabled),
        "pending_restart" => Ok(domain::HostInfrastructureConfigStatus::PendingRestart),
        "active" => Ok(domain::HostInfrastructureConfigStatus::Active),
        _ => bail!(ControlPlaneError::InvalidInput(
            "host_infrastructure_config_status"
        )),
    }
}

fn map_record(
    row: sqlx::postgres::PgRow,
) -> Result<domain::HostInfrastructureProviderConfigRecord> {
    let status: String = row.get("status");
    Ok(domain::HostInfrastructureProviderConfigRecord {
        id: row.get("id"),
        installation_id: row.get("installation_id"),
        extension_id: row.get("extension_id"),
        provider_code: row.get("provider_code"),
        config_ref: row.get("config_ref"),
        enabled_contracts: row.get("enabled_contracts"),
        config_json: row.get("config_json"),
        status: parse_status(&status)?,
        updated_by: row.get("updated_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[async_trait]
impl HostInfrastructureConfigRepository for PgControlPlaneStore {
    async fn upsert_host_infrastructure_provider_config(
        &self,
        input: &UpsertHostInfrastructureProviderConfigInput,
    ) -> Result<domain::HostInfrastructureProviderConfigRecord> {
        let row = sqlx::query(
            r#"
            insert into host_infrastructure_provider_configs (
                id,
                scope_id,
                installation_id,
                extension_id,
                provider_code,
                config_ref,
                enabled_contracts,
                config_json,
                status,
                created_by,
                updated_by
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            on conflict (installation_id, provider_code) do update
            set
                extension_id = excluded.extension_id,
                config_ref = excluded.config_ref,
                enabled_contracts = excluded.enabled_contracts,
                config_json = excluded.config_json,
                status = excluded.status,
                updated_by = excluded.updated_by,
                updated_at = now()
            returning
                id,
                installation_id,
                extension_id,
                provider_code,
                config_ref,
                enabled_contracts,
                config_json,
                status,
                updated_by,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(input.installation_id)
        .bind(&input.extension_id)
        .bind(&input.provider_code)
        .bind(&input.config_ref)
        .bind(&input.enabled_contracts)
        .bind(&input.config_json)
        .bind(input.status.as_str())
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_record(row)
    }

    async fn list_host_infrastructure_provider_configs(
        &self,
    ) -> Result<Vec<domain::HostInfrastructureProviderConfigRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                installation_id,
                extension_id,
                provider_code,
                config_ref,
                enabled_contracts,
                config_json,
                status,
                updated_by,
                created_at,
                updated_at
            from host_infrastructure_provider_configs
            order by updated_at desc, id desc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_record).collect()
    }
}
