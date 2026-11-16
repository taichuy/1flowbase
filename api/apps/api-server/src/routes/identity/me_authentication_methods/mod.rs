#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::app_state::ApiState;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use crate::routes::external_auth::transaction::{self, Purpose};
pub use api_console_interaction::routes::identity_group::me_authentication_methods::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use axum::{
    http::{header, StatusCode},
    response::Response,
};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use control_plane::{
    auth::AuthenticationMethodsService, ports::ExternalIdentityLifecycleRepository,
};
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use std::sync::Arc;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api.rs"]
mod tests;

#[cfg(test)]
pub(crate) mod proof;
