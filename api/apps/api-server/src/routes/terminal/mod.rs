#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use super::console_interface::{self, ConsoleInterfaceFuture, ConsoleInterfacePort};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::{
    local_command::jobs::{JobSnapshot, ReadOutput},
    local_terminal::{self, dto::*},
};
pub use api_console_interaction::routes::terminal::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use axum::http::HeaderMap;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use interface_runtime::InterfaceContract;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use serde_json::json;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use std::sync::{Arc, Weak};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
