#[cfg(test)]
use super::*;
#[cfg(test)]
use crate::openapi_interface::contract::{self as event, ApiInput, ApiOperationContext, ApiOutput};
#[cfg(test)]
use serde_json::{json, Value};
#[cfg(test)]
use std::sync::Arc;
pub(crate) use api_console_interaction::routes::local_commands::api_event_projection::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
