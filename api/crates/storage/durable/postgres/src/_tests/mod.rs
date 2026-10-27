mod application;
mod billing_repository_tests;
mod connection;
mod crate_smoke_tests;
#[path = "migrations/data_model_template_identity_migration_tests.rs"]
mod data_model_template_identity_migration_tests;
#[path = "resource_models/data_source_repository_tests.rs"]
mod data_source_repository_tests;
mod durable_runtime_tests;
#[path = "extension_repositories/extension_installation_repository_tests.rs"]
mod extension_installation_repository_tests;
mod file_management_repository_tests;
mod flow_repository_tests;
mod frontend_block_catalog_repository_tests;
mod frontstage;
#[path = "migrations/i18n_global_key_migration_tests.rs"]
mod i18n_global_key_migration_tests;
mod identity;
#[path = "migrations/legacy_retirement.rs"]
mod legacy_retirement;
#[path = "extension_repositories/lifecycle_outbox_repository_tests.rs"]
mod lifecycle_outbox_repository_tests;
#[path = "migrations/max_output_tokens_migration_tests.rs"]
mod max_output_tokens_migration_tests;
mod mcp_result_receipt_repository_tests;
#[path = "migrations/migration_smoke.rs"]
mod migration_smoke;
#[path = "resource_models/model_definition_repository_tests.rs"]
mod model_definition_repository_tests;
#[path = "resource_models/model_mapper_tests.rs"]
mod model_mapper_tests;
mod model_provider_repository_tests;
#[path = "resource_models/native_sql_tests.rs"]
mod native_sql_tests;
#[path = "extension_repositories/node_contribution_repository_tests.rs"]
mod node_contribution_repository_tests;
mod ordered_tree;
#[path = "resource_models/physical_schema_repository_tests.rs"]
mod physical_schema_repository_tests;
mod plugin;
#[path = "migrations/pricing_rule_upgrade_migration_tests.rs"]
mod pricing_rule_upgrade_migration_tests;
#[path = "migrations/protocol_context_migration_tests.rs"]
mod protocol_context_migration_tests;
pub(crate) mod provider_protocol_capsule_store_tests;
#[path = "resource_models/runtime_registry_health_tests.rs"]
mod runtime_registry_health_tests;
mod ui_management_repository_tests;
#[path = "migrations/unified_extension_installation_migration_tests.rs"]
mod unified_extension_installation_migration_tests;
mod workspace_access_tests;
mod workspace_scope_tests;

#[path = "extension_repositories/plugin_settings_template_tests.rs"]
mod plugin_settings_template_tests;

mod runtime_json_storage_tests;

mod trajectory;

mod client_trajectory;
mod storage_compaction_replay;

#[path = "migrations/communication_timeline.rs"]
mod communication_timeline;
