use serde_json::json;
pub(crate) use api_plugins_models::managed_services::stream::*;
#[cfg_attr(test, api_test_selection::group(core))]
#[path = "_tests/stream_lifecycle.rs"]
mod lifecycle_tests;

use std::{collections::BTreeMap, sync::Arc};
use axum::http::HeaderMap;
use tokio::sync::{mpsc, oneshot, watch};
