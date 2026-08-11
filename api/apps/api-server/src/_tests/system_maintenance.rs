use std::time::Duration;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use domain::RecoveryJobId;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;

use crate::_tests::support::{test_api_state_with_database_url, test_config};

#[tokio::test]
async fn maintenance_rejects_mutations_before_route_auth_or_database_access() {
    let (state, _) = test_api_state_with_database_url().await;
    let lease = state
        .system_maintenance
        .begin(RecoveryJobId::new(), OffsetDateTime::now_utc())
        .unwrap();
    lease.wait_for_drain(Duration::from_secs(1)).await.unwrap();
    let app = crate::app_with_state_and_config(state, &test_config());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"identifier":"root","password":"change-me"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["retry-after"], "5");
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 32 * 1024).await.unwrap()).unwrap();
    assert_eq!(body["code"], "system_maintenance");
}
