#[cfg(test)]
use std::{collections::HashMap, sync::Arc, time::Duration};
#[cfg(test)]
use anyhow::{ensure, Context, Result};
#[cfg(test)]
use control_plane::ports::{NotificationRepository, WorkspaceRepository};
#[cfg(test)]
use domain::{
    ActorContext, CommunicationBindingRecord, NotificationInstanceRecord,
    NotificationInstanceStatus, UserStatus,
};
#[cfg(test)]
use extension_contracts::{
    NotificationAuthorizationStatus, NotificationRuntimeInput, NotificationTypeDescriptor,
};
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use tokio::{sync::watch, task::JoinSet};
#[cfg(test)]
use uuid::Uuid;
#[cfg(test)]
use crate::{app_state::ApiState, notification_runtime::ManagedNotificationProvider};
pub(crate) use api_console_interaction::communication_runtime::*;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(all(test, not(feature = "root-1805-consumer-fixture")))]
#[path = "_tests/acceptance.rs"]
mod acceptance_tests;

#[cfg(test)]
pub(crate) mod commands;

#[cfg(test)]
pub(crate) use api_host_kernel::scoped_tools;
