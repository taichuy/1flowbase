#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use axum::{extract::State, http::HeaderMap, Json, Router};
#[cfg(test)]
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
#[cfg(test)]
use control_plane::auth::{
    AuthKernel, AuthenticatorRegistry, LoginCommand, SessionIssuer, SignUpCommand,
};
#[cfg(test)]
use control_plane::ports::SessionStore;
#[cfg(test)]
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::{Map, Value};
#[cfg(test)]
use storage_durable_postgres::MainDurableStore;
#[cfg(test)]
use utoipa::ToSchema;
#[cfg(test)]
use uuid::Uuid;
#[cfg(test)]
use interface_runtime::{
    InterfaceInvocationError, InterfaceInvocationKernel, InterfaceProtocol, PublicPrincipal,
};
#[cfg(test)]
use super::login_entries_interface::{
    self, PublicLoginEntriesFuture, PublicLoginEntriesInput, PublicLoginEntriesOutput,
    PublicLoginEntriesPort, PublicLoginEntriesTargetError,
};
#[cfg(test)]
use super::public_residual_interface::{
    self, PublicProvidersFuture, PublicProvidersInput, PublicProvidersOutput, PublicProvidersPort,
    PublicResidualTargetError, PublicSignUpFuture, PublicSignUpInput, PublicSignUpOutput,
    PublicSignUpPort,
};
#[cfg(test)]
use super::sign_in_interface::{
    self, PublicSignInInput, PublicSignInOutput, PublicSignInTargetError,
};
#[cfg(test)]
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
pub use api_console_interaction::routes::auth::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "../../_tests/identity_routes/entry_action.rs"]
mod entry_action_tests;
