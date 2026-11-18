pub use api_mcp::routes::mcp_protocol::virtual_ui::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/virtual_ui.rs"]
mod tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "virtual_ui/workflow_credentials.rs"]
pub mod workflow_credentials;

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use {
    crate::routes::mcp_management::{
        debug_execute::{self, McpServerBoundInputs},
        McpDebugExecuteBody,
    },
    api_mcp::routes::mcp_protocol::virtual_ui::test_support::*,
    api_mcp::routes::mcp_protocol::{resources, result_delivery},
    axum::{
        http::{HeaderMap, StatusCode},
        response::Response,
    },
    domain::mcp_management::{McpInstanceStatus, McpToolStatus},
    serde_json::{json, Value},
    std::sync::Arc,
};
