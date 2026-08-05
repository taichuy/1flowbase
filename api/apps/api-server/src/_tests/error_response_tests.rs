use api_server::error_response::ApiError;
use api_server::official_extension_catalog::OfficialExtensionCatalogUnavailable;
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use control_plane::errors::ControlPlaneError;
use plugin_framework::error::PluginFrameworkError;

#[tokio::test]
async fn error_response_includes_status_field() {
    let response = ApiError(anyhow::anyhow!("test error")).into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], 500);
    assert_eq!(payload["code"], "internal_error");
    assert!(payload["message"].as_str().is_some());
}

#[tokio::test]
async fn inconsistent_extension_catalog_is_a_retryable_service_failure() {
    let response = ApiError(anyhow::Error::new(
        OfficialExtensionCatalogUnavailable::new(anyhow::anyhow!("checksum mismatch")),
    ))
    .into_response();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["status"], 503);
    assert_eq!(
        payload["code"],
        "extension_catalog_temporarily_inconsistent"
    );
}

#[tokio::test]
async fn error_response_http_status_matches_body_status() {
    let test_cases = vec![
        (
            ControlPlaneError::NotAuthenticated,
            StatusCode::UNAUTHORIZED,
            401,
            "not_authenticated",
        ),
        (
            ControlPlaneError::PermissionDenied("insufficient_permissions"),
            StatusCode::FORBIDDEN,
            403,
            "insufficient_permissions",
        ),
        (
            ControlPlaneError::NotFound("resource_not_found"),
            StatusCode::NOT_FOUND,
            404,
            "resource_not_found",
        ),
        (
            ControlPlaneError::Conflict("resource_conflict"),
            StatusCode::CONFLICT,
            409,
            "resource_conflict",
        ),
        (
            ControlPlaneError::InvalidInput("invalid_request"),
            StatusCode::BAD_REQUEST,
            400,
            "invalid_request",
        ),
    ];

    for (error, expected_status, expected_status_code, expected_code) in test_cases {
        let response = ApiError(anyhow::Error::from(error)).into_response();

        assert_eq!(response.status(), expected_status);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(
            payload["status"].as_u64().unwrap(),
            expected_status_code,
            "Body status should match HTTP status for code: {}",
            expected_code
        );
        assert_eq!(payload["code"], expected_code);
    }
}

#[tokio::test]
async fn internal_error_exposes_real_message_with_sanitization() {
    let response = ApiError(anyhow::anyhow!(
        "connection failed: api_key=secret123 token"
    ))
    .into_response();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], 500);
    assert_eq!(payload["code"], "internal_error");

    let message = payload["message"].as_str().unwrap();
    // Should contain the real error context
    assert!(message.contains("connection failed"));
    // Should NOT expose the actual secret
    assert!(!message.contains("secret123"));
    // Should sanitize sensitive patterns
    assert!(message.contains("<redacted>") || !message.contains("api_key=secret123"));
}

#[tokio::test]
async fn sanitize_bearer_token() {
    let response =
        ApiError(anyhow::anyhow!("auth failed: Bearer sk-1234567890abcdef")).into_response();

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let message = payload["message"].as_str().unwrap();
    assert!(message.contains("auth failed"));
    assert!(!message.contains("sk-1234567890abcdef"));
}

#[tokio::test]
async fn sanitize_custom_headers() {
    let response = ApiError(anyhow::anyhow!(
        "request failed: x-api-key: sensitive_value"
    ))
    .into_response();

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let message = payload["message"].as_str().unwrap();
    assert!(message.contains("request failed"));
    assert!(!message.contains("sensitive_value"));
}

#[tokio::test]
async fn preserve_non_sensitive_error_details() {
    let response = ApiError(anyhow::anyhow!(
        "failed to connect to https://api.example.com: connection timeout after 30s"
    ))
    .into_response();

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let message = payload["message"].as_str().unwrap();
    // All non-sensitive details should be preserved
    assert!(message.contains("https://api.example.com"));
    assert!(message.contains("connection timeout"));
    assert!(message.contains("30s"));
}

#[tokio::test]
async fn provider_runtime_error_preserves_detailed_message() {
    use plugin_framework::provider_contract::{ProviderRuntimeError, ProviderRuntimeErrorKind};

    let runtime_error = PluginFrameworkError::RuntimeContract {
        error: Box::new(
            ProviderRuntimeError::new(
                ProviderRuntimeErrorKind::ModelNotFound,
                "model 'gpt-5' not found: available models are gpt-4, gpt-3.5-turbo",
            )
            .with_provider_summary("openai provider error"),
        ),
    };
    let response = ApiError(anyhow::Error::from(runtime_error)).into_response();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["status"], 502);
    assert_eq!(payload["code"], "provider_runtime");

    let message = payload["message"].as_str().unwrap();
    // Provider runtime errors should surface detailed upstream messages
    assert!(message.contains("gpt-5"));
    assert!(message.contains("not found"));
    assert!(message.contains("available models"));
}
