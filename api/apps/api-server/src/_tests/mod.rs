pub(crate) mod application;
mod application_public_api;
mod assistant;
#[path = "identity_routes/authentication_activation_tests.rs"]
mod authentication_activation_tests;
#[path = "system_settings/billing_routes.rs"]
mod billing_routes;
#[path = "interface_contracts/compatibility_interface_tests.rs"]
mod compatibility_interface_tests;
#[path = "console_routes/console_navigation_routes.rs"]
mod console_navigation_routes;
#[path = "console_routes/console_operation_compilation_tests.rs"]
mod console_operation_compilation_tests;
#[path = "console_routes/console_policy_migration_tests.rs"]
mod console_policy_migration_tests;
#[path = "interface_contracts/dynamic_backend_consumer_inventory.rs"]
mod dynamic_backend_consumer_inventory;
pub(crate) mod extension_bus;
#[path = "identity_routes/external_auth_trace.rs"]
mod external_auth_trace;
#[path = "interface_contracts/external_endpoint_catalog.rs"]
mod external_endpoint_catalog;
#[path = "console_routes/frontend_block_catalog_routes.rs"]
mod frontend_block_catalog_routes;
mod host_extensions;
mod host_infrastructure;
mod i18n_catalog;
#[path = "console_routes/i18n_catalog_routes.rs"]
mod i18n_catalog_routes;
#[path = "interface_contracts/interface_contribution_collector.rs"]
mod interface_contribution_collector;
#[path = "interface_contracts/interface_route_equivalence.rs"]
mod interface_route_equivalence;
#[path = "interface_contracts/interface_vertical_slices.rs"]
mod interface_vertical_slices;
#[path = "mcp_routes/mcp_protocol_routes.rs"]
mod mcp_protocol_routes;
#[path = "interface_contracts/native_read_interface_tests.rs"]
mod native_read_interface_tests;
mod notifications;
mod plugin_routes;
#[path = "catalog_sources/provider_request_log_worker.rs"]
mod provider_request_log_worker;
mod provider_runtime;
#[path = "interface_contracts/public_residual_interface_tests.rs"]
mod public_residual_interface_tests;
#[path = "interface_contracts/public_sign_in_interface_tests.rs"]
mod public_sign_in_interface_tests;
mod route_docs;
#[path = "console_routes/runtime_i18n_catalog_routes.rs"]
mod runtime_i18n_catalog_routes;
#[path = "interface_contracts/runtime_model_interface_tests.rs"]
mod runtime_model_interface_tests;
#[path = "support/integration_bridge.rs"]
pub(crate) mod support;
#[path = "system_settings/system_backups_routes.rs"]
mod system_backups_routes;
#[path = "host_boot/system_maintenance.rs"]
mod system_maintenance;
#[path = "system_settings/system_templates_routes.rs"]
mod system_templates_routes;
#[path = "console_routes/ui_component_catalog_source.rs"]
mod ui_component_catalog_source;
#[path = "console_routes/ui_management_routes.rs"]
mod ui_management_routes;
mod webmcp_routes;

pub(crate) use application::{
    create_gated_provider_instance, create_marker_output_provider_instance,
    create_ready_provider_instance, ProviderInvocationGate,
};
#[path = "host_boot/workflow_schedule_worker.rs"]
mod workflow_schedule_worker;

mod interface_lifecycle_acceptance;

#[path = "console_routes/frontstage_contract_private.rs"]
mod frontstage_routes;
mod runtime_event_stream;

#[path = "catalog_sources/official_extension_catalog_source.rs"]
mod official_extension_catalog_source;
#[path = "catalog_sources/official_i18n_catalog_source.rs"]
mod official_i18n_catalog_source;

#[path = "mcp_routes/mcp_oauth_routes.rs"]
mod mcp_oauth_routes;
