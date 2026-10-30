extern crate self as control_plane;

pub mod application;
pub mod application_public_api;
pub mod audit;
pub mod auth;
pub mod billing;
pub mod bootstrap;
pub mod capability_plugin_runtime;
pub mod capability_runtime;
pub mod client_trajectory;
pub mod data_source;
pub mod errors;
pub mod external_agent_bridge;
pub mod file_management;
pub mod flow;
pub mod flow_run_title;
pub mod frontend_block_catalog;
pub mod frontstage;
pub mod host_extension;
pub mod host_extension_boot;
#[path = "presentation_catalogs/i18n.rs"]
pub mod i18n;
pub mod i18n_catalog;
#[path = "extension_services/installed_provider_package.rs"]
mod installed_provider_package;
#[path = "extension_services/js_dependency.rs"]
pub mod js_dependency;
pub mod lifecycle_outbox_dispatcher;
pub mod mcp_bundle;
pub mod mcp_management;
pub mod member;
pub mod model_definition;
pub mod model_provider;
pub mod navigation_cache;
#[path = "network_services/network_egress.rs"]
pub mod network_egress;
#[path = "network_services/network_egress_pool.rs"]
pub mod network_egress_pool;
#[path = "network_services/network_egress_route.rs"]
pub mod network_egress_route;
#[path = "network_services/network_egress_secret.rs"]
pub mod network_egress_secret;
#[path = "extension_services/node_contribution.rs"]
pub mod node_contribution;
pub mod orchestration_runtime;
#[path = "extension_services/plugin_isolation.rs"]
pub mod plugin_isolation;
#[path = "extension_services/plugin_lifecycle.rs"]
pub mod plugin_lifecycle;
pub mod plugin_management;
pub mod ports;
pub mod profile;
pub mod resource_action;
pub mod resource_crud;
pub mod role;
pub mod runtime_observability;
pub mod runtime_registry_sync;
pub mod session_security;
pub mod state_transition;
#[path = "system_services/system_agent.rs"]
pub mod system_agent;
pub mod system_backup;
#[path = "system_services/system_branding.rs"]
pub mod system_branding;
#[path = "system_services/system_metadata.rs"]
pub mod system_metadata;
pub mod system_recovery;
pub mod system_runtime;
#[path = "system_services/local_command.rs"]
pub mod local_command;
#[path = "presentation_catalogs/ui_component_catalog.rs"]
pub mod ui_component_catalog;
#[path = "presentation_catalogs/ui_management.rs"]
pub mod ui_management;
pub mod workspace;
pub mod workspace_session;

pub fn crate_name() -> &'static str {
    "control-plane"
}

#[cfg(test)]
pub mod _tests;

pub mod managed_event_publication;

pub mod portable_template;

pub mod mcp_oauth;

pub mod organization;

pub mod agent_logs;

pub mod notifications;

/// Offline Responses history verification and replacement proofs.
#[cfg(feature = "responses-history-migration")]
pub use application_public_api::compat::openai::history::migration as responses_history_migration;
