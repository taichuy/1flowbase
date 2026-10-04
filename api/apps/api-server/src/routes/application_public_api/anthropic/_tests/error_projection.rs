use super::*;

#[test]
fn upstream_errors_project_anthropic_shape_with_safe_request_id() {
    let error = NativeError {
        code: "provider_upstream_error".to_string(),
        message: "Your Claude Code version is too old. Please update.".to_string(),
        details: json!({
            "status_code": 400,
            "upstream_error": {"type":"invalid_request_error","message":"Your Claude Code version is too old. Please update.","request_id":"req_old"},
            "raw_body": "original upstream diagnostic",
            "configuration": "SECRET"
        }),
    };
    let body = project(
        Some(&error),
        runtime_status(Some(&error)),
        "runtime_error",
        &error.message,
    );
    assert_eq!(
        body,
        json!({"type":"error","error":{"type":"invalid_request_error","message":"Your Claude Code version is too old. Please update."},"request_id":"req_old"})
    );
    assert!(!body.to_string().contains("SECRET"));
}

#[test]
fn plain_non_json_and_rate_limit_errors_keep_message_and_status_type() {
    for (status, kind) in [
        (400, "invalid_request_error"),
        (429, "rate_limit_error"),
        (529, "overloaded_error"),
    ] {
        let error = NativeError {
            code: "provider_upstream_error".to_string(),
            message: "plain upstream failure".to_string(),
            details: json!({"status_code":status,"raw_body":"plain upstream failure"}),
        };
        let body = project(
            Some(&error),
            runtime_status(Some(&error)),
            "provider_upstream_error",
            &error.message,
        );
        assert_eq!(body["error"]["type"], kind);
        assert_eq!(body["error"]["message"], "plain upstream failure");
        assert_eq!(runtime_status(Some(&error)).as_u16(), status);
    }
}

#[tokio::test]
async fn http_boundary_preserves_status_type_message_and_request_id() {
    use crate::routes::application_public_api::{
        anthropic::AnthropicRouteError, native::NativeApiError,
    };
    use axum::response::IntoResponse;
    let runtime = NativeError {
        code: "provider_upstream_error".to_string(),
        message: "obsolete client".to_string(),
        details: json!({"status_code":400,"upstream_error":{"type":"invalid_request_error","message":"Update Claude Code to continue.","request_id":"req_client"}}),
    };
    let mut native = NativeApiError::new(
        StatusCode::BAD_REQUEST,
        "provider_upstream_error",
        &runtime.message,
    );
    native.runtime_error = Some(Box::new(runtime));
    let response = AnthropicRouteError::Native(native).into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["request-id"], "req_client");
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["message"], "Update Claude Code to continue.");
    assert_eq!(body["request_id"], "req_client");
}
