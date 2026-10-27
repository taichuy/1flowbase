mod managed_operation_lifetime;
mod managed_service_mutation;
extern crate self as storage_durable_postgres;

pub mod application_public_api_repository;
pub mod application_repository;
pub mod auth_repository;
pub mod billing_repository;
mod connection;
pub mod data_source_repository;
#[path = "extension_repositories/extension_installation_repository.rs"]
pub mod extension_installation_repository;
pub mod file_management_repository;
pub mod flow_repository;
pub mod frontend_block_catalog_repository;
pub mod frontstage_block_repository;
pub mod frontstage_repository;
#[path = "extension_repositories/host_extension_migration_repository.rs"]
pub mod host_extension_migration_repository;
pub mod i18n_catalog_repository;
#[path = "extension_repositories/js_dependency_repository.rs"]
pub mod js_dependency_repository;
pub mod lifecycle_outbox_repository;
pub mod managed_schema_repository;
pub mod mappers;
#[path = "integration_repositories/mcp_management_repository.rs"]
pub mod mcp_management_repository;
#[path = "integration_repositories/mcp_result_receipt_repository.rs"]
pub mod mcp_result_receipt_repository;
#[path = "identity_repositories/member_repository.rs"]
pub mod member_repository;
pub mod model_definition_repository;
pub mod model_provider_repository;
#[path = "extension_repositories/native_plugin_target_repository.rs"]
mod native_plugin_target_repository;
pub mod native_sql;
#[path = "integration_repositories/network_egress_repository.rs"]
pub mod network_egress_repository;
#[path = "extension_repositories/node_contribution_repository.rs"]
pub mod node_contribution_repository;
pub mod orchestration_runtime_repository;
pub mod ordered_tree;
pub mod physical_schema_repository;
#[path = "extension_repositories/plugin_data_repository.rs"]
pub mod plugin_data_repository;
#[path = "extension_repositories/plugin_installation_commit_repository.rs"]
mod plugin_installation_commit_repository;
#[path = "extension_repositories/plugin_repository.rs"]
pub mod plugin_repository;
#[path = "extension_repositories/plugin_settings_template_repository.rs"]
mod plugin_settings_template_repository;
#[path = "extension_repositories/plugin_worker_repository.rs"]
pub mod plugin_worker_repository;
#[path = "integration_repositories/provider_protocol_capsule_store.rs"]
mod provider_protocol_capsule_store;
pub mod repositories;
#[path = "identity_repositories/role_repository.rs"]
pub mod role_repository;
mod runtime;
pub mod runtime_record_repository;
mod secret_crypto;
#[path = "identity_repositories/system_branding_repository.rs"]
mod system_branding_repository;
pub use secret_crypto::AuthenticationSecretCipher;
pub mod system_backup;
pub mod system_recovery;
pub mod ui_management_repository;
#[path = "identity_repositories/workspace_repository.rs"]
pub mod workspace_repository;

pub use connection::{
    connect, connect_with_max_connections, connect_with_pool_settings, PgAdvisoryLockKey,
    PgConnectionObservation, PgConnectionRegistry, PgConnectionSettings, PgCoordinationLane,
    PgCoordinationLease, PgInstanceConnectionBudget, PgLaneObservation, PgPoolObservation,
    PgPoolSettings, PgSessionStatus, PgTemporaryObservation, PgTemporaryPool,
};
pub use model_definition_repository::RuntimeTableNamePolicy;
pub use native_sql::execute_native_sql;
pub use orchestration_runtime_repository::RuntimeStorageCompactionReceipt;
pub use provider_protocol_capsule_store::PgProviderProtocolCapsuleStore;
pub use repositories::PgControlPlaneStore;
pub use runtime::{
    build_main_durable_postgres, build_main_durable_postgres_with_connection_settings,
    build_main_durable_postgres_with_max_connections,
    build_main_durable_postgres_with_pool_settings, MainDurableRuntime, MainDurableStore,
};
pub use system_backup::*;
pub use system_recovery::PostgreSqlRecoveryTarget;

use anyhow::Result;
use sqlx::PgPool;

pub fn crate_name() -> &'static str {
    "storage-durable-postgres"
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    // The embedded forward-only migration set is the only schema initialization path.
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod _tests;

#[path = "extension_repositories/plugin_contribution_authority_repository.rs"]
mod plugin_contribution_authority_repository;

mod portable_template_repository;

#[path = "integration_repositories/mcp_oauth_repository.rs"]
mod mcp_oauth_repository;

#[path = "identity_repositories/organization_repository.rs"]
pub mod organization_repository;

#[path = "extension_repositories/plugin_credential_repository.rs"]
mod plugin_credential_repository;
pub use plugin_credential_repository::PgPluginCredentialRepository;

#[path = "integration_repositories/notification_repository.rs"]
mod notification_repository;

#[path = "integration_repositories/notification_publication.rs"]
mod notification_publication;

#[path = "integration_repositories/communications/mod.rs"]
mod communications;
