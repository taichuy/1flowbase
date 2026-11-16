#[cfg_attr(test, api_test_selection::group(core))]
pub(crate) use api_plugins_models::managed_services::stream::*;
#[cfg_attr(test, api_test_selection::group(core))]
use serde_json::json;
#[cfg_attr(test, api_test_selection::group(core))]
#[path = "_tests/stream_lifecycle.rs"]
mod lifecycle_tests;

#[cfg_attr(test, api_test_selection::group(core))]
use axum::http::HeaderMap;
#[cfg_attr(test, api_test_selection::group(core))]
use std::{collections::BTreeMap, sync::Arc};
#[cfg_attr(test, api_test_selection::group(core))]
use tokio::sync::{mpsc, oneshot, watch};
