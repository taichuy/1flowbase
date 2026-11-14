#[cfg(test)]
use super::{
    console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    console_route_assembly::{console_post, ConsoleRouteAssembly},
};
#[cfg(test)]
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    local_command::{process::CompletedCommand, HostCommandExecutor},
};
#[cfg(test)]
use axum::{extract::State, http::HeaderMap, response::Response, Json};
#[cfg(test)]
use control_plane::local_command::LocalCommandService;
#[cfg(test)]
use interface_runtime::InterfaceContract;
#[cfg(test)]
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use utoipa::ToSchema;
pub use api_console_interaction::routes::local_commands::*;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
