use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn system_branding_public_read_and_persistence_keep_html_verbatim() {
    let app = test_app().await;
    let (status, defaults) = request(
        &app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(defaults["data"]["system_name"], "1flowbase");
    assert_eq!(defaults["data"].as_object().unwrap().len(), 5);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let value = json!({"logo_url":"data:image/svg+xml;base64,PHN2Zy8+", "system_name":"新系统", "login_title":"欢迎来到平台", "system_description":"系统介绍", "filing_html":"<style>.filing {color:red}</style>\n<a class=filing href='https://beian.miit.gov.cn/' data-any='yes'>备案号</a>"});
    let (status, saved) = request(
        &app,
        "PUT",
        "/api/console/settings/system",
        &cookie,
        &csrf,
        value.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["data"], value);
    let (_, public) = request(
        &app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(public["data"], value);
    let (_, persisted) = request(
        &app,
        "GET",
        "/api/console/settings/system",
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(persisted["data"], value);
    let mut invalid = value.clone();
    invalid["system_name"] = json!("   ");
    let (status, _) = request(
        &app,
        "PUT",
        "/api/console/settings/system",
        &cookie,
        &csrf,
        invalid,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, unchanged) = request(
        &app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(unchanged["data"], value);
    let (status, _) = request(&app, "PUT", "/api/console/settings/system", "", "", value).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn system_branding_read_grant_does_not_allow_save() {
    let app = test_app().await;
    let (root, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member = create_member(&app, &root, &root_csrf, "branding_viewer", "temp-pass").await;
    create_role(&app, &root, &root_csrf, "branding_viewer").await;
    replace_member_roles(&app, &root, &root_csrf, &member, &["branding_viewer"]).await;
    let (cookie, _) = login_and_capture_cookie(&app, "branding_viewer", "temp-pass").await;
    let (status, _) = request(
        &app,
        "GET",
        "/api/console/settings/system",
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let policy_path = "/api/console/settings/roles/branding_viewer/console-policy";
    let policy = json!({"groups":[{"kind":"settings_feature","group_id":"system.branding","enabled":true,"strategy":"custom","operations":[{"kind":"simple","operation_id":"system.branding.get","enabled":true}]}]});
    let (status, body) = request(&app, "PUT", policy_path, &root, &root_csrf, policy).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (cookie, csrf) = login_and_capture_cookie(&app, "branding_viewer", "temp-pass").await;
    let (status, value) = request(
        &app,
        "GET",
        "/api/console/settings/system",
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let (status, _) = request(
        &app,
        "PUT",
        "/api/console/settings/system",
        &cookie,
        &csrf,
        value["data"].clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let policy = json!({"groups":[{"kind":"settings_feature","group_id":"system.branding","enabled":true,"strategy":"full","operations":[]}]});
    let (status, body) = request(&app, "PUT", policy_path, &root, &root_csrf, policy).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (cookie, csrf) = login_and_capture_cookie(&app, "branding_viewer", "temp-pass").await;
    let (status, body) = request(
        &app,
        "PUT",
        "/api/console/settings/system",
        &cookie,
        &csrf,
        value["data"].clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
