use super::*;
use axum::http::HeaderMap;
use orchestration_runtime::execution_engine::RuntimeInternalToolInvoker;
use serde_json::{json, Value};

mod guards;
mod host_dispatch;

fn request(url: &str) -> WorkflowInternalHttpRequest {
    WorkflowInternalHttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: vec![],
        body: vec![],
        max_response_bytes: 1024 * 1024,
    }
}
