#[cfg(test)]
use crate::routes::external_auth::transaction::{self, BindingIntent, Purpose};
#[cfg(test)]
use crate::{app_state::ApiState, error_response::ApiError, response::ApiSuccess};
#[cfg(test)]
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
#[cfg(test)]
use axum_extra::extract::cookie::CookieJar;
#[cfg(test)]
use control_plane::{
    auth::AuthenticationMethodsService,
    errors::ControlPlaneError as Error,
    ports::{AuthRepository, ExternalIdentityLifecycleRepository, PluginRepository},
};
#[cfg(test)]
use std::{collections::BTreeMap, sync::Arc};
#[cfg(test)]
use uuid::Uuid;
pub(crate) use api_console_interaction::routes::identity_group::me_authentication_methods::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api.rs"]
mod tests;

#[cfg(test)]
pub(crate) mod proof;
