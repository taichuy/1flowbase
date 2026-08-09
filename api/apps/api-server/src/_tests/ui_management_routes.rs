use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles, test_app,
};

async fn list_templates(app: &axum::Router, cookie: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/ui-management/templates")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

async fn grant_template_list_operation(app: &axum::Router, cookie: &str, csrf: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/ui_management_limited/console-policy")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "groups": [{
                            "kind": "settings_feature",
                            "group_id": "system.ui-management",
                            "enabled": true,
                            "strategy": "custom",
                            "operations": [{
                                "kind": "simple",
                                "operation_id": "ui_management.templates.list",
                                "enabled": true
                            }]
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn ac_001_ui_management_api_requires_its_list_operation_grant() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "ui-management-member",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "ui_management_limited").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["ui_management_limited"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "ui-management-member", "temp-pass").await;

    assert_eq!(
        list_templates(&app, &member_cookie).await,
        StatusCode::FORBIDDEN
    );

    grant_template_list_operation(&app, &root_cookie, &root_csrf).await;
    assert_eq!(list_templates(&app, &member_cookie).await, StatusCode::OK);
}
