pub(crate) use application_public_protocol::anthropic_error::{project, runtime_status};

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use axum::http::StatusCode;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use control_plane::application_public_api::native::NativeError;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use serde_json::{json, Value};

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "_tests/error_projection.rs"]
mod tests;
