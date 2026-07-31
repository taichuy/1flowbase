use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

// AC-004: full is a compiled backend profile, not a frontend inference over localized options.
#[tokio::test]
async fn role_console_policy_catalog_serializes_compiled_full_profiles() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/console-policy-catalog")
                .header("cookie", cookie)
                .header("x-1flowbase-locale", "zh_Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let applications = payload["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["group_id"].as_str() == Some("system.applications"))
        .unwrap();
    let create = applications["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["operation_id"].as_str() == Some("create_application"))
        .unwrap();
    let view = applications["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["operation_id"].as_str() == Some("list_applications"))
        .unwrap();

    assert_eq!(
        create["full_profile"],
        json!({ "kind": "simple", "enabled": true })
    );
    assert_eq!(
        view["full_profile"],
        json!({ "kind": "row", "scope": "scope_all" })
    );
    assert_eq!(
        create["route"],
        json!({
            "method": "POST",
            "path": "/api/console/applications"
        })
    );
}
