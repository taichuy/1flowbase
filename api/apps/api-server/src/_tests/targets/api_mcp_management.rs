pub(crate) mod support {
    pub use api_server::test_support::support::*;
}
#[path = "../mcp_routes/mcp_bundle_routes.rs"]
mod mcp_bundle_routes;
#[path = "../mcp_routes/mcp_client_credential_routes.rs"]
mod mcp_client_credential_routes;
#[path = "../mcp_routes/mcp_management_debug_execute_routes.rs"]
mod mcp_management_debug_execute_routes;
#[path = "../mcp_routes/mcp_management_routes.rs"]
mod mcp_management_routes;
#[path = "../mcp_routes/mcp_upstream_routes.rs"]
mod mcp_upstream_routes;
