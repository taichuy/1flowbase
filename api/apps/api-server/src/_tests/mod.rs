mod application;
mod application_public_api;
mod assistant;
#[path = "identity_routes/auth_center_routes.rs"]
mod auth_center_routes;
#[path = "identity_routes/auth_routes.rs"]
mod auth_routes;
#[path = "identity_routes/authentication_activation_tests.rs"]
mod authentication_activation_tests;
#[path = "system_settings/billing_routes.rs"]
mod billing_routes;
#[path = "interface_contracts/compatibility_interface_tests.rs"]
mod compatibility_interface_tests;
#[path = "host_boot/config_tests.rs"]
mod config_tests;
#[path = "console_routes/console_navigation_routes.rs"]
mod console_navigation_routes;
#[path = "console_routes/console_operation_compilation_tests.rs"]
mod console_operation_compilation_tests;
#[path = "console_routes/console_policy_migration_tests.rs"]
mod console_policy_migration_tests;
#[path = "console_routes/core_console_display_i18n_routes.rs"]
mod core_console_display_i18n_routes;
#[path = "system_settings/data_sources_routes.rs"]
mod data_sources_routes;
#[path = "interface_contracts/dynamic_backend_consumer_inventory.rs"]
mod dynamic_backend_consumer_inventory;
#[path = "host_boot/error_response_tests.rs"]
mod error_response_tests;
mod extension_bus;
#[path = "identity_routes/external_auth_trace.rs"]
mod external_auth_trace;
#[path = "interface_contracts/external_endpoint_catalog.rs"]
mod external_endpoint_catalog;
#[path = "system_settings/file_management_routes.rs"]
mod file_management_routes;
#[path = "console_routes/frontend_block_catalog_routes.rs"]
mod frontend_block_catalog_routes;
#[path = "console_routes/frontstage_data_capability_routes.rs"]
mod frontstage_data_capability_routes;
#[path = "console_routes/frontstage_routes.rs"]
mod frontstage_routes;
mod host_extensions;
mod host_infrastructure;
mod i18n_catalog;
#[path = "console_routes/i18n_catalog_routes.rs"]
mod i18n_catalog_routes;
#[path = "interface_contracts/interface_contribution_collector.rs"]
mod interface_contribution_collector;
#[path = "interface_contracts/interface_coverage_inventory.rs"]
mod interface_coverage_inventory;
#[path = "interface_contracts/interface_route_equivalence.rs"]
mod interface_route_equivalence;
#[path = "interface_contracts/interface_vertical_slices.rs"]
mod interface_vertical_slices;
#[path = "mcp_routes/mcp_bundle_routes.rs"]
mod mcp_bundle_routes;
#[path = "mcp_routes/mcp_client_credential_routes.rs"]
mod mcp_client_credential_routes;
#[path = "mcp_routes/mcp_management_debug_execute_routes.rs"]
mod mcp_management_debug_execute_routes;
#[path = "mcp_routes/mcp_management_routes.rs"]
mod mcp_management_routes;
#[path = "mcp_routes/mcp_protocol_routes.rs"]
mod mcp_protocol_routes;
#[path = "mcp_routes/mcp_upstream_routes.rs"]
mod mcp_upstream_routes;
#[path = "identity_routes/me_routes.rs"]
mod me_routes;
#[path = "identity_routes/member_routes.rs"]
mod member_routes;
#[path = "catalog_sources/model_provider_icon_routes.rs"]
mod model_provider_icon_routes;
mod model_provider_routes;
#[path = "interface_contracts/native_read_interface_tests.rs"]
mod native_read_interface_tests;
mod network_center_routes;
mod notifications;
#[path = "catalog_sources/official_extension_catalog_source.rs"]
mod official_extension_catalog_source;
#[path = "catalog_sources/official_i18n_catalog_source.rs"]
mod official_i18n_catalog_source;
mod plugin_routes;
#[path = "catalog_sources/provider_request_log_worker.rs"]
mod provider_request_log_worker;
mod provider_runtime;
#[path = "interface_contracts/public_residual_interface_tests.rs"]
mod public_residual_interface_tests;
#[path = "interface_contracts/public_sign_in_interface_tests.rs"]
mod public_sign_in_interface_tests;
#[path = "identity_routes/role_catalog_full_profile_routes.rs"]
mod role_catalog_full_profile_routes;
#[path = "identity_routes/role_routes.rs"]
mod role_routes;
mod route_docs;
mod runtime_event_stream;
#[path = "console_routes/runtime_i18n_catalog_routes.rs"]
mod runtime_i18n_catalog_routes;
#[path = "interface_contracts/runtime_model_interface_tests.rs"]
mod runtime_model_interface_tests;
#[path = "identity_routes/session_routes.rs"]
mod session_routes;
#[path = "system_settings/settings_feature_data_models_routes.rs"]
mod settings_feature_data_models_routes;
#[path = "console_routes/settings_feature_explicit_namespace_routes.rs"]
mod settings_feature_explicit_namespace_routes;
#[path = "identity_routes/settings_feature_members_routes.rs"]
mod settings_feature_members_routes;
#[path = "identity_routes/settings_feature_roles_routes.rs"]
mod settings_feature_roles_routes;
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
#[path = "system_settings/system_branding_routes.rs"]
mod system_branding_routes;
#[path = "system_settings/system_routes.rs"]
mod system_routes;
#[path = "host_boot/workflow_schedule_worker.rs"]
mod workflow_schedule_worker;
#[path = "identity_routes/workspace_routes.rs"]
mod workspace_routes;

mod interface_lifecycle_acceptance;

#[path = "mcp_routes/mcp_oauth_routes.rs"]
mod mcp_oauth_routes;

// Shared only within the standard API test host; keep the fixture module private.
pub(crate) use host_extensions::host_extension_boot_tests::NativeSettingsFixture;
