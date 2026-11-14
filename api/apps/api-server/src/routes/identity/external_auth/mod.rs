#[cfg(test)]
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
#[cfg(test)]
use axum::{
    extract::{RawQuery, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Redirect, Response},
    Json,
};
#[cfg(test)]
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
#[cfg(test)]
use control_plane::ports::{AuthRepository, ExternalIdentityLifecycleRepository};
#[cfg(test)]
use domain::{LoginEntryRecord, VerifiedExternalIdentity};
#[cfg(test)]
use extension_contracts::{AuthenticationRequest, AuthenticationResponse};
#[cfg(test)]
use interface::{PublicInput, PublicOutput};
#[cfg(test)]
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[cfg(test)]
use transaction::{BindingIntent, Transaction};
#[cfg(test)]
use uuid::Uuid;
pub(crate) use api_console_interaction::routes::identity_group::external_auth::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/protocol_tests.rs"]
mod tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/authentication_tests.rs"]
mod authentication_tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/authority_tests.rs"]
pub(crate) mod authority_tests;
