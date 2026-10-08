use crate::error_response::ApiError;
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use orchestration_runtime::compiler::{FlowValidationError, NodeDiagnostic};
use serde_json::{json, Value};

#[tokio::test]
async fn node_diagnostics_api_returns_public_details_for_typed_configuration_failure() {
    let error = anyhow::Error::new(FlowValidationError {
        diagnostics: vec![NodeDiagnostic {
            node_id: Some("aggregate".into()),
            code: "variable_aggregator_output_mismatch".into(),
            field_path: "/outputs/0/title".into(),
            message: "Output title must match group key; token=private-value".into(),
            expected: Some(json!("result")),
        }],
    })
    .context("compile preview document");
    let response = ApiError(error).into_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["status"], 400);
    assert_eq!(payload["code"], "flow_validation_failed");
    assert_eq!(payload["details"]["phase"], "validation");
    let diagnostic = &payload["details"]["diagnostics"][0];
    assert_eq!(diagnostic["node_id"], "aggregate");
    assert_eq!(diagnostic["field_path"], "/outputs/0/title");
    assert_eq!(diagnostic["expected"], "result");
    assert!(!payload.to_string().contains("private-value"));
}

#[tokio::test]
async fn node_diagnostics_api_keeps_internal_and_authorization_error_semantics() {
    for (error, status, code) in [
        (
            anyhow::anyhow!("database unavailable"),
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
        ),
        (
            anyhow::Error::new(control_plane::errors::ControlPlaneError::PermissionDenied(
                "denied",
            )),
            StatusCode::FORBIDDEN,
            "denied",
        ),
        (
            anyhow::Error::new(control_plane::errors::ControlPlaneError::NotAuthenticated),
            StatusCode::UNAUTHORIZED,
            "not_authenticated",
        ),
    ] {
        let response = ApiError(error).into_response();
        assert_eq!(response.status(), status);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["code"], code);
        assert!(payload.get("details").is_none());
    }
}
