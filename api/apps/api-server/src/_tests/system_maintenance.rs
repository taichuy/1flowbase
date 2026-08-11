use std::time::Duration;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use domain::RecoveryJobId;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;

use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};

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

#[tokio::test]
async fn maintenance_control_semantics_do_not_allow_unknown_or_ordinary_mutations() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let lease = state
        .system_maintenance
        .begin(RecoveryJobId::new(), OffsetDateTime::now_utc())
        .unwrap();
    lease.wait_for_drain(Duration::from_secs(1)).await.unwrap();

    for (path, content_type) in [
        ("/api/console/settings/system-backups", None),
        (
            "/api/console/settings/system-backups/import",
            Some("application/octet-stream"),
        ),
        ("/api/console/settings/system-backups/unknown", None),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri(path)
            .header("cookie", &cookie)
            .header("x-csrf-token", &csrf);
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert_eq!(response.headers()["retry-after"], "5", "{path}");
    }
}
