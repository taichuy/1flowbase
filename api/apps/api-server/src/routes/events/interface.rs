#[cfg(test)]
use crate::{
    app_state::ApiState,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};
pub(crate) use api_console_interaction::routes::events::interface::*;
#[cfg(test)]
use control_plane::event_center::{EventCenterConfigurationService, EventCenterRuntime};
#[cfg(test)]
use domain::event_center::{
    CreateEventDefinitionInput, EventCatalogEntry, EventCatalogQuery, EventDefinitionRecord,
    UpdateEventDefinitionInput,
};
#[cfg(test)]
use interface_runtime::{InterfaceContract, UserPrincipal};
#[cfg(test)]
use serde_json::{json, Value};
#[cfg(test)]
use std::sync::{Arc, Weak};
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/interface.rs"]
mod tests;
