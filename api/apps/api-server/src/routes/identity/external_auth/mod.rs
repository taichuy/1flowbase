#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::{app_state::ApiState, error_response::ApiError};
pub use api_console_interaction::routes::identity_group::external_auth::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use axum::http::HeaderMap;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use axum_extra::extract::cookie::{Cookie, CookieJar};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use control_plane::ports::AuthRepository;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use domain::LoginEntryRecord;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use interface::{PublicInput, PublicOutput};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/authentication_tests.rs"]
mod authentication_tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/authority_tests.rs"]
pub(crate) mod authority_tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/protocol_tests.rs"]
mod tests;
