use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_app,
    test_app_with_database_url, test_config,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

pub(crate) use api_server::test_support::application::*;

#[path = "../../application/application_runtime_routes/artifacts_billing_routes.rs"]
mod artifacts_billing_routes;
#[path = "../../application/application_runtime_routes/lazy_tree_contract_routes/mod.rs"]
mod lazy_tree_contract_routes;
#[path = "../../application/application_runtime_routes/logs_routes/mod.rs"]
mod logs_routes;
#[path = "../../application/application_runtime_routes/mcp_settings.rs"]
mod mcp_settings;
#[path = "../../application/application_runtime_routes/read_side_effect_routes.rs"]
mod read_side_effect_routes;
#[path = "../../application/application_runtime_routes/resume_cancel_routes.rs"]
mod resume_cancel_routes;
#[path = "../../application/application_runtime_routes/stream_routes.rs"]
mod stream_routes;
