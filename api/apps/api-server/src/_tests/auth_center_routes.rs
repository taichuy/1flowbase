use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn console_auth_center_overview_lists_authenticators_without_options() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/auth-center/overview")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["default_authenticator_name"],
        json!("password-local")
    );

    let authenticators = payload["data"]["authenticators"].as_array().unwrap();
    let password_local = authenticators
        .iter()
        .find(|authenticator| authenticator["name"] == "password-local")
        .expect("password-local should be visible in auth center overview");
    assert_eq!(password_local["auth_type"], json!("password-local"));
    assert_eq!(password_local["title"], json!("Password"));
    assert_eq!(password_local["enabled"], json!(true));
    assert_eq!(password_local["is_builtin"], json!(true));
    assert!(password_local.get("options").is_none());
}

#[tokio::test]
async fn console_auth_center_overview_requires_user_view_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "auth-center-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "auth_center_no_access").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["auth_center_no_access"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "auth-center-viewer", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/auth-center/overview")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
