extern crate self as storage_postgres;

pub mod application_public_api_repository;
pub mod application_repository;
pub mod auth_repository;
mod connection;
pub mod data_source_repository;
pub mod extension_installation_repository;
pub mod file_management_repository;
pub mod flow_repository;
pub mod frontend_block_catalog_repository;
pub mod frontstage_repository;
pub mod host_extension_migration_repository;
pub mod host_infrastructure_config_repository;
pub mod i18n_catalog_repository;
pub mod js_dependency_repository;
pub mod mappers;
pub mod mcp_management_repository;
pub mod mcp_result_receipt_repository;
pub mod member_repository;
pub mod model_definition_repository;
pub mod model_provider_repository;
pub mod native_sql;
pub mod node_contribution_repository;
pub mod orchestration_runtime_repository;
pub mod physical_schema_repository;
mod plugin_installation_commit_repository;
pub mod plugin_repository;
pub mod plugin_worker_repository;
pub mod repositories;
pub mod role_repository;
pub mod runtime_record_repository;
mod secret_crypto;
pub mod workspace_repository;

pub use connection::{
    connect, connect_with_max_connections, connect_with_pool_settings, PgPoolSettings,
};
pub use native_sql::execute_native_sql;
pub use repositories::PgControlPlaneStore;

use anyhow::Result;
use sqlx::PgPool;

pub fn crate_name() -> &'static str {
    "storage-postgres"
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod _tests;
