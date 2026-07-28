pub mod application_public_api;
#[path = "applications/mod.rs"]
mod applications_group;
pub mod console_route_assembly;
pub(crate) mod core_console_i18n;
pub(crate) mod core_console_operation_specs;
#[path = "files.rs"]
pub mod files;
#[path = "frontstage/mod.rs"]
pub mod frontstage;
pub(crate) mod helpers;
#[path = "identity/mod.rs"]
mod identity_group;
pub mod mcp_protocol;
#[path = "plugins_and_models/mod.rs"]
mod plugins_and_models_group;
pub mod runtime_i18n_catalog;
mod runtime_i18n_catalog_cache;
#[path = "settings/mod.rs"]
mod settings_group;

pub use applications_group::{
    application_api, application_orchestration, application_runtime, applications,
};
pub use identity_group::{auth, me, session, user_api_keys};
pub use plugins_and_models_group::{
    data_sources, frontend_block_catalog, js_dependencies, model_definitions, model_providers,
    node_contributions, plugins, runtime_models,
};
pub use settings_group::{
    application_management, auth_center, data_models, docs, file_storages, file_tables,
    host_infrastructure, i18n_catalog, mcp_management, members, navigation, permissions, roles,
    system, workspace, workspaces,
};

pub const PUBLIC_API_PATH_PREFIX: &str = "/api/public/";

#[cfg(test)]
mod _tests;
