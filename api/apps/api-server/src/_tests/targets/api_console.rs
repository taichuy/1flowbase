pub(crate) mod support {
    pub use api_server::test_support::support::*;
}
pub(crate) use api_server::test_support::application::{
    ProviderInvocationGate, create_gated_provider_instance, create_marker_output_provider_instance,
    create_ready_provider_instance,
};
#[path = "../console_routes/console_policy_migration_tests.rs"]
mod console_policy_migration_tests;
#[path = "../console_routes/core_console_display_i18n_routes.rs"]
mod core_console_display_i18n_routes;
#[path = "../console_routes/frontstage_data_capability_routes.rs"]
mod frontstage_data_capability_routes;
#[path = "../console_routes/frontstage_routes.rs"]
mod frontstage_routes;
#[path = "../console_routes/settings_feature_explicit_namespace_routes.rs"]
mod settings_feature_explicit_namespace_routes;
