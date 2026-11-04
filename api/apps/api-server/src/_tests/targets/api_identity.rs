pub(crate) mod support {
    pub use api_server::test_support::support::*;
}
pub(crate) use api_server::test_support::application::{
    ProviderInvocationGate, create_gated_provider_instance, create_marker_output_provider_instance,
    create_ready_provider_instance,
};
#[path = "../identity_routes/auth_center_routes.rs"]
mod auth_center_routes;
#[path = "../identity_routes/auth_routes.rs"]
mod auth_routes;
#[path = "../identity_routes/me_routes.rs"]
mod me_routes;
#[path = "../identity_routes/member_routes.rs"]
mod member_routes;
#[path = "../identity_routes/role_catalog_full_profile_routes.rs"]
mod role_catalog_full_profile_routes;
#[path = "../identity_routes/role_routes.rs"]
mod role_routes;
#[path = "../identity_routes/session_routes.rs"]
mod session_routes;
#[path = "../identity_routes/settings_feature_members_routes.rs"]
mod settings_feature_members_routes;
#[path = "../identity_routes/settings_feature_roles_routes.rs"]
mod settings_feature_roles_routes;
#[path = "../identity_routes/workspace_routes.rs"]
mod workspace_routes;
