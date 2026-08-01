use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::support::{create_member, create_role, replace_member_roles};

#[tokio::test]
async fn root_1545_ac_3_installed_route_reads_generic_inventory_shape() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed?limit=20")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["limit"], 20);
    assert!(payload["data"]["entries"].is_array());
}

#[tokio::test]
async fn root_1545_ac_5_install_route_rejects_missing_csrf_before_catalog_network() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/extension-center/install")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "category": "i18n",
                        "catalog_id": "i18n:taichuy/platform",
                        "version": "2.0.1"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["code"], "not_authenticated");
}

#[tokio::test]
async fn root_1545_ac_5_installed_route_respects_console_operation_acl() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "extension-no-access",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "extension_no_access").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["extension_no_access"],
    )
    .await;
    let (cookie, _) = login_and_capture_cookie(&app, "extension-no-access", "temp-pass").await;
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/extension-center/installed")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
