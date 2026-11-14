#[cfg(test)]
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use super::{
    console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    console_route_assembly::{console_post, ConsoleRouteAssembly},
};
#[cfg(test)]
use crate::openapi_interface::json_contract::{JsonApiInput, JsonApiOutput, JsonInputLocation};
#[cfg(test)]
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    local_command::jobs::{JobError, JobOwner, JobSnapshot, ReadInput, ReadOutput, StartInput},
    local_terminal::{self, dto::*},
};
#[cfg(test)]
use axum::{extract::State, http::HeaderMap, Json};
#[cfg(test)]
use interface_runtime::InterfaceContract;
#[cfg(test)]
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::sync::{Arc, Weak};
#[cfg(test)]
use utoipa::ToSchema;
#[cfg(test)]
use uuid::Uuid;
pub use api_console_interaction::routes::terminal::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/api_event_projection.rs"]
mod api_event_tests;
