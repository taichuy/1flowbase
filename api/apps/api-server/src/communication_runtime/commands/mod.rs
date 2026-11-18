#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
use super::ApiState;
#[cfg_attr(test, api_test_selection::group(core))]
pub(crate) use api_console_interaction::communication_runtime::commands::*;
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
#[path = "_tests/mod.rs"]
mod tests;
