pub(crate) use api_mcp::routes::mcp_protocol::virtual_ui::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/virtual_ui.rs"]
mod tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "virtual_ui/workflow_credentials.rs"]
mod workflow_credentials;

#[cfg(test)]
use {
    crate::error_response::ApiError,
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
    control_plane::mcp_management::McpManagementService,
    domain::mcp_management::{McpInstanceStatus, McpToolStatus},
    serde_json::{json, Value},
    std::sync::Arc,
};
