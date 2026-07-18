use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use plugin_runner::app;
use serde_json::Value;
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn runner_health_route_returns_ok_payload() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["service"], "plugin-runner");
    assert_eq!(payload["status"], "ok");
}

#[tokio::test]
async fn runner_runtime_profile_keeps_sampler_state_between_requests() {
    let app = app();
    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/system/runtime-profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = to_bytes(first.into_body(), usize::MAX).await.unwrap();
    let first_payload: Value = serde_json::from_slice(&first_body).unwrap();
    assert_eq!(first_payload["service"], "plugin-runner");
    assert_eq!(
        first_payload["metrics"]["cpu"]["availability"],
        "warming_up"
    );

    tokio::time::sleep(Duration::from_millis(250)).await;

    let second = app
        .oneshot(
            Request::builder()
                .uri("/system/runtime-profile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_body = to_bytes(second.into_body(), usize::MAX).await.unwrap();
    let second_payload: Value = serde_json::from_slice(&second_body).unwrap();
    assert_eq!(
        second_payload["metrics"]["cpu"]["availability"],
        "available"
    );
    assert!(second_payload["metrics"]["sample_interval_milliseconds"].is_number());
}
