#![recursion_limit = "256"]
//! MCP management, protocol, OAuth, WebMCP and resource import adapters.
#[macro_use]
mod test_support_visibility;
pub use api_host_kernel::*;
pub mod managed_service_mcp;
pub mod resource_import;
mod runtime_factory_handle;
pub mod routes {
    pub use api_host_kernel::routes::*;
    pub mod mcp_oauth;
    pub mod mcp_protocol;
    #[path = "settings/mod.rs"]
    pub mod settings_group;
    pub mod webmcp;
    pub use settings_group::mcp_management;
}

#[doc(hidden)]
pub mod openapi;
