pub(crate) mod support {
    pub use api_server::test_support::support::*;
}
pub(crate) use api_server::test_support::application::{
    ProviderInvocationGate, create_gated_provider_instance, create_marker_output_provider_instance,
    create_ready_provider_instance,
};
#[path = "../mcp_routes/mcp_bundle_routes.rs"]
mod mcp_bundle_routes;
#[path = "../mcp_routes/mcp_client_credential_routes.rs"]
mod mcp_client_credential_routes;
#[path = "../mcp_routes/mcp_management_debug_execute_routes.rs"]
mod mcp_management_debug_execute_routes;
#[path = "../mcp_routes/mcp_management_routes.rs"]
mod mcp_management_routes;
#[path = "../mcp_routes/mcp_oauth_routes.rs"]
mod mcp_oauth_routes;
#[path = "../mcp_routes/mcp_upstream_routes.rs"]
mod mcp_upstream_routes;
