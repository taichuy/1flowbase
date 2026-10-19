mod application;
#[path = "extensions/application_js_dependency.rs"]
mod application_js_dependency;
mod application_public_api;
mod auth_provider_instance_tests;
mod auth_settings;
mod billing_tests;
mod bootstrap_tests;
#[path = "extensions/capability_plugin_runtime_tests.rs"]
mod capability_plugin_runtime_tests;
mod data_source_service_tests;
mod ephemeral_inspection_contract_tests;
mod external_agent_bridge_tests;
mod file_management;
mod flow;
#[path = "extensions/frontend_component_catalog_service_tests.rs"]
mod frontend_component_catalog_service_tests;
mod frontstage_block_tree;
mod host_extensions;
mod host_infrastructure;
mod i18n_catalog;
#[path = "extensions/mcp_bundle_tests.rs"]
mod mcp_bundle_tests;
mod member;
mod model_definition;
mod model_definition_service_tests;
mod model_provider;
mod model_provider_service_tests;
#[path = "extensions/node_contribution_service_tests.rs"]
mod node_contribution_service_tests;
mod orchestration_runtime;
#[path = "extensions/plugin_isolation_tests.rs"]
mod plugin_isolation_tests;
mod plugin_management;
mod profile;
mod resource_action_tests;
mod resource_crud_tests;
mod role;
mod session_security;
mod state_transition_tests;
mod support;
pub(crate) use support::RecordingRuntimeEventStream;
mod system_agent_tests;
mod system_metadata_tests;
mod system_runtime;
#[path = "extensions/ui_component_catalog_tests.rs"]
mod ui_component_catalog_tests;
mod workspace;
mod workspace_session;
