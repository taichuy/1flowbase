extern crate self as control_plane;

pub mod application;
pub mod application_public_api;
pub mod audit;
pub mod auth;
pub mod bootstrap;
pub mod capability_plugin_runtime;
pub mod capability_runtime;
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
pub mod host_extension_inventory;
pub mod host_infrastructure_config;
pub mod i18n;
pub mod i18n_catalog;
mod installed_provider_package;
pub mod js_dependency;
pub mod mcp_bundle;
pub mod mcp_management;
pub mod member;
pub mod model_definition;
pub mod model_provider;
pub mod node_contribution;
pub mod orchestration_runtime;
pub mod plugin_isolation;
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
pub mod system_agent;
pub mod system_metadata;
pub mod system_runtime;
pub mod workspace;
pub mod workspace_session;

pub fn crate_name() -> &'static str {
    "control-plane"
}

#[cfg(test)]
pub mod _tests;
