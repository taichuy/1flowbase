use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use domain::AuthenticatorRecord;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn console_auth_center_overview_lists_authenticators_with_schema_form_values() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    state
        .store
        .upsert_authenticator(&AuthenticatorRecord {
            name: "oidc-main".to_string(),
            auth_type: "oidc".to_string(),
            title: "OIDC".to_string(),
            enabled: false,
            is_builtin: false,
            options: json!({
                "issuer_url": "https://idp.example.com",
                "allow_signup": true,
                "client_secret": "secret-value"
            }),
        })
        .await
        .unwrap();
    let app = crate::app_with_state(state);
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
    assert_eq!(password_local["config_schema"], json!([]));
    assert_eq!(password_local["config_values"], json!({}));

    let oidc = authenticators
        .iter()
        .find(|authenticator| authenticator["name"] == "oidc-main")
        .expect("custom authenticator should be visible in auth center overview");
    assert_eq!(oidc["enabled"], json!(false));
    assert_eq!(
        oidc["config_schema"],
        json!([
            {"key": "allow_signup", "label": "allow_signup", "type": "boolean"},
            {"key": "issuer_url", "label": "issuer_url", "type": "string"}
        ])
    );
    assert_eq!(
        oidc["config_values"],
        json!({
            "allow_signup": true,
            "issuer_url": "https://idp.example.com"
        })
    );
    assert!(oidc["config_values"].get("client_secret").is_none());
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

#[tokio::test]
async fn console_auth_center_enable_authenticator_requires_user_manage_permission() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    state
        .store
        .upsert_authenticator(&AuthenticatorRecord {
            name: "oidc-main".to_string(),
            auth_type: "oidc".to_string(),
            title: "OIDC".to_string(),
            enabled: false,
            is_builtin: false,
            options: json!({}),
        })
        .await
        .unwrap();
    let app = crate::app_with_state(state);
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/auth-center/authenticators/oidc-main/actions/enable")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let overview = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/auth-center/overview")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(overview.into_body(), usize::MAX).await.unwrap()).unwrap();
    let authenticators = payload["data"]["authenticators"].as_array().unwrap();
    let oidc = authenticators
        .iter()
        .find(|authenticator| authenticator["name"] == "oidc-main")
        .expect("enabled authenticator should be visible");
    assert_eq!(oidc["enabled"], json!(true));
}
