#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use crate::{app_state::ApiState, notification_runtime::ManagedNotificationProvider};
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use anyhow::Result;
pub(crate) use api_console_interaction::communication_runtime::*;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use control_plane::ports::NotificationRepository;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use domain::ActorContext;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use serde_json::Value;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use std::{sync::Arc, time::Duration};
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use tokio::sync::watch;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use uuid::Uuid;
#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(all(test, not(feature = "root-1805-consumer-fixture")))]
#[path = "_tests/acceptance.rs"]
mod acceptance_tests;

#[cfg(test)]
pub(crate) mod commands;

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
pub(crate) use api_host_kernel::scoped_tools;
