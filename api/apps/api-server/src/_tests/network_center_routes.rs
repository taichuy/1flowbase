use crate::_tests::support::{create_member, login_and_capture_cookie, test_app};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

/// AC-014: a valid console session without the registered SettingsFeature must be rejected by
/// the real route before it can observe the provider registry.
#[tokio::test]
async fn network_center_provider_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-center-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-center-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/providers")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
