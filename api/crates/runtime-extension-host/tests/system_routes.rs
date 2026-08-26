use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use runtime_extension_host::app;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn runner_runtime_profile_route_returns_snapshot() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/system/runtime-profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["service"], "plugin-runner");
    assert!(payload["host_fingerprint"]
        .as_str()
        .unwrap()
        .starts_with("host_"));
    assert!(payload["memory"]["total_gb"].is_number());
}
