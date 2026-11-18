#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::routes::mcp_protocol::virtual_ui;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::routes::mcp_protocol::virtual_ui::VirtualMcpScope;
pub use api_console_interaction::routes::assistant::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use control_plane::profile::ASSISTANT_META_KEY;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use serde_json::json;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use std::sync::Arc;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "assistant/_tests/route_contracts.rs"]
mod tests;
