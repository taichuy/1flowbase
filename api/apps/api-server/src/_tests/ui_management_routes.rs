use axum::{
    body::{to_bytes, Body},
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

#[tokio::test]
async fn wp_d2_component_record_routes_expose_custom_crud_at_the_stable_namespace() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/ui-management/components")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "component_code": "local.status-panel",
                        "name": "Status panel",
                        "description": "Shows system status",
                        "import_code": "opaque import {{{",
                        "source_code": "opaque source }}}",
                        "source": "local",
                        "group": "operations",
                        "upstream": { "identity": "@local/status-panel", "version": "0.1.0" },
                        "version": "1.0.0",
                        "keywords": ["status"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = payload["data"]["id"].as_str().unwrap();
    assert_eq!(payload["data"]["origin"], "custom");
    assert_eq!(
        payload["data"]["scope_id"],
        "00000000-0000-0000-0000-000000000000"
    );

    let get = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "System status panel",
                        "description": "Shows system status",
                        "import_code": "opaque import {{{",
                        "source_code": "opaque source }}}",
                        "source": "local",
                        "group": "operations",
                        "upstream": { "identity": "@local/status-panel", "version": "0.2.0" },
                        "version": "1.1.0",
                        "keywords": ["status"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/ui-management/components")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let list_payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(list.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        list_payload["data"][0]["component_code"],
        "local.status-panel"
    );

    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);

    let missing = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/ui-management/components/{id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
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
