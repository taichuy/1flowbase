pub use api_mcp::routes::mcp_protocol::virtual_ui::workflow_credentials::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "../_tests/workflow_credentials/mod.rs"]
mod tests;

use {
    axum::Router, control_plane_contracts::ports::WorkflowRunIdentityRepository,
    orchestration_runtime::execution_engine::WorkflowInternalHttpRequest, uuid::Uuid,
};
