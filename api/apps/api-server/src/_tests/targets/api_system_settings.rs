pub(crate) mod support {
    pub use api_server::test_support::support::*;
}
#[path = "../system_settings/data_sources_routes.rs"]
mod data_sources_routes;
#[path = "../system_settings/file_management_routes.rs"]
mod file_management_routes;
#[path = "../system_settings/settings_feature_data_models_routes.rs"]
mod settings_feature_data_models_routes;
#[path = "../system_settings/system_branding_routes.rs"]
mod system_branding_routes;
#[path = "../system_settings/system_routes.rs"]
mod system_routes;
