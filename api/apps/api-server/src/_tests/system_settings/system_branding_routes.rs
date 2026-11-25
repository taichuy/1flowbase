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
    assert_eq!(defaults["data"]["page_title"], "1flowbase");
    assert_eq!(defaults["data"]["login_hero_title"], "1flowbase");
    assert_eq!(defaults["data"]["login_hero_tagline"], Value::Null);
    assert_eq!(defaults["data"]["hide_powered_by"], false);
    assert_eq!(defaults["data"]["hide_help_menu"], false);
    assert_eq!(defaults["data"]["hide_login_tagline"], false);
    assert!(defaults["data"].get("login_title").is_none());
    assert_eq!(defaults["data"].as_object().unwrap().len(), 10);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let value = json!({"logo_url":"data:image/svg+xml;base64,PHN2Zy8+", "system_name":"新系统", "hide_login_tagline":true, "system_description":"系统介绍", "filing_html":"<style>.filing {color:red}</style>\n<a class=filing href='https://beian.miit.gov.cn/' data-any='yes'>备案号</a>"});
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
    let expected_public = json!({
        "logo_url":"data:image/svg+xml;base64,PHN2Zy8+",
        "system_name":"新系统",
        "hide_login_tagline":true,
        "system_description":"系统介绍",
        "filing_html":"<style>.filing {color:red}</style>\n<a class=filing href='https://beian.miit.gov.cn/' data-any='yes'>备案号</a>",
        "page_title":"新系统",
        "login_hero_title":"1flowbase",
        "login_hero_tagline":null,
        "hide_powered_by":false,
        "hide_help_menu":false
    });
    let (_, public) = request(
        &app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(public["data"], expected_public);
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
    assert_eq!(unchanged["data"], expected_public);
    let mut visible = value.clone();
    visible["hide_login_tagline"] = json!(false);
    let (status, _) = request(
        &app,
        "PUT",
        "/api/console/settings/system",
        &cookie,
        &csrf,
        visible,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, public) = request(
        &app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(public["data"]["hide_login_tagline"], false);

    let (status, _) = request(&app, "PUT", "/api/console/settings/system", "", "", value).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn public_system_metadata_projects_static_json_only_when_commercial_license_plugin_is_active() {
    use crate::_tests::support::test_app_with_commercial_branding_fixture;
    use uuid::Uuid;

    let temp_root = std::env::temp_dir().join(format!(
        "1flowbase-commercial-license-test-{}",
        Uuid::now_v7()
    ));
    let family_dir = temp_root.join("installed").join("commercial-license");
    let default_assets_dir = family_dir.join("0.1.0").join("_assets");
    std::fs::create_dir_all(&default_assets_dir).unwrap();
    std::fs::write(
        default_assets_dir.join("default-config.json"),
        serde_json::to_vec(&json!({
            "hide_help_menu": true,
            "hide_powered_by": false,
            "page_title": "Packaged OEM",
            "login_hero_title": "OEM Cloud",
            "login_hero_tagline": "Packaged Tagline"
        }))
        .unwrap(),
    )
    .unwrap();

    // Even when files exist on disk, an inactive/uninstalled plugin must NOT affect public metadata.
    let inactive_app =
        test_app_with_commercial_branding_fixture(temp_root.display().to_string(), None).await;
    let (status, inactive_res) = request(
        &inactive_app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(inactive_res["data"]["page_title"], "1flowbase");
    assert_eq!(inactive_res["data"]["login_hero_title"], "1flowbase");
    assert_eq!(inactive_res["data"]["login_hero_tagline"], Value::Null);
    assert_eq!(inactive_res["data"]["hide_help_menu"], false);
    assert_eq!(inactive_res["data"]["hide_powered_by"], false);

    // Activate `@taichuy/commercial-license`: first reads packaged `_assets/default-config.json`,
    // then reflects static `installed/commercial-license/config.json` immediately without DB changes.
    let active_app =
        test_app_with_commercial_branding_fixture(temp_root.display().to_string(), Some("0.1.0"))
            .await;

    let (status, fallback_res) = request(
        &active_app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{fallback_res}");
    assert_eq!(fallback_res["data"]["page_title"], "Packaged OEM");
    assert_eq!(fallback_res["data"]["login_hero_title"], "OEM Cloud");
    assert_eq!(fallback_res["data"]["login_hero_tagline"], "Packaged Tagline");
    assert_eq!(fallback_res["data"]["hide_help_menu"], true);
    assert_eq!(fallback_res["data"]["hide_powered_by"], false);

    std::fs::write(
        family_dir.join("config.json"),
        serde_json::to_vec(&json!({
            "hide_help_menu": true,
            "hide_powered_by": true,
            "page_title": "Customer Product",
            "login_hero_title": "Customer AI",
            "login_hero_tagline": "Customer Slogan"
        }))
        .unwrap(),
    )
    .unwrap();
    let (status, custom_res) = request(
        &active_app,
        "GET",
        "/api/public/system-metadata",
        "",
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{custom_res}");
    assert_eq!(custom_res["data"]["page_title"], "Customer Product");
    assert_eq!(custom_res["data"]["login_hero_title"], "Customer AI");
    assert_eq!(custom_res["data"]["login_hero_tagline"], "Customer Slogan");
    assert_eq!(custom_res["data"]["hide_help_menu"], true);
    assert_eq!(custom_res["data"]["hide_powered_by"], true);

    let _ = std::fs::remove_dir_all(&temp_root);
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
