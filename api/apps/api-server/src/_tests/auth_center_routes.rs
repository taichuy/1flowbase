use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_url, test_app,
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
                "description": "Corporate OIDC",
                "config_form_schema": [
                    {
                        "key": "issuer_url",
                        "label": "Issuer URL",
                        "type": "string",
                        "control": "url",
                        "read_only": false,
                        "required": true,
                        "pattern": "^https://"
                    }
                ],
                "extension_config": {
                    "issuer_url": "https://idp.example.com",
                    "allow_signup": true
                }
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
    assert_eq!(
        password_local["config_values"]["description"],
        json!("Local password authentication")
    );
    assert_eq!(
        password_local["config_values"]["name"],
        json!("password-local")
    );
    assert_eq!(password_local["config_values"]["title"], json!("Password"));
    assert_eq!(password_local["config_values"]["enabled"], json!(true));
    assert_eq!(
        password_local["config_values"]["extension_config"],
        json!({})
    );
    assert!(password_local.get("options").is_none());
    assert!(password_local.get("description").is_none());
    assert!(password_local.get("extension_config").is_none());
    assert!(password_local["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "name"
            && field["read_only"] == true
            && field["required"] == true));
    assert!(password_local["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "description"
            && field["control"] == "textarea"
            && field["required"] == false));

    let oidc = authenticators
        .iter()
        .find(|authenticator| authenticator["name"] == "oidc-main")
        .expect("custom authenticator should be visible in auth center overview");
    assert_eq!(oidc["enabled"], json!(false));
    assert_eq!(
        oidc["config_values"]["description"],
        json!("Corporate OIDC")
    );
    assert_eq!(oidc["config_values"]["name"], json!("oidc-main"));
    assert_eq!(oidc["config_values"]["title"], json!("OIDC"));
    assert_eq!(oidc["config_values"]["enabled"], json!(false));
    assert_eq!(
        oidc["config_schema"],
        json!([
            {
                "key": "issuer_url",
                "label": "Issuer URL",
                "type": "string",
                "control": "url",
                "read_only": false,
                "required": true,
                "pattern": "^https://"
            }
        ])
    );
    assert_eq!(
        oidc["config_values"]["extension_config"],
        json!({
            "allow_signup": true,
            "issuer_url": "https://idp.example.com"
        })
    );
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

#[tokio::test]
async fn console_auth_center_update_config_updates_editable_fields_and_preserves_schema_values() {
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
                "description": "Corporate OIDC",
                "config_form_schema": [
                    {
                        "key": "issuer_url",
                        "label": "Issuer URL",
                        "type": "string",
                        "control": "url",
                        "read_only": false,
                        "required": true,
                        "pattern": "^https://"
                    }
                ],
                "extension_config": {
                    "issuer_url": "https://idp.example.com",
                    "allow_signup": true
                }
            }),
        })
        .await
        .unwrap();
    let app = crate::app_with_state(state.clone());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/oidc-main/config")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "oidc-main",
                        "title": "OIDC Login",
                        "enabled": true,
                        "description": "Updated corporate OIDC"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let authenticator = &payload["data"];
    assert_eq!(authenticator["name"], json!("oidc-main"));
    assert_eq!(authenticator["title"], json!("OIDC Login"));
    assert_eq!(authenticator["enabled"], json!(true));
    assert_eq!(authenticator["config_values"]["name"], json!("oidc-main"));
    assert_eq!(authenticator["config_values"]["title"], json!("OIDC Login"));
    assert_eq!(authenticator["config_values"]["enabled"], json!(true));
    assert_eq!(
        authenticator["config_values"]["description"],
        json!("Updated corporate OIDC")
    );
    assert_eq!(
        authenticator["config_schema"],
        json!([
            {
                "key": "issuer_url",
                "label": "Issuer URL",
                "type": "string",
                "control": "url",
                "read_only": false,
                "required": true,
                "pattern": "^https://"
            }
        ])
    );
    assert_eq!(
        authenticator["config_values"]["extension_config"],
        json!({
            "issuer_url": "https://idp.example.com",
            "allow_signup": true
        })
    );

    let saved = state
        .store
        .find_authenticator("oidc-main")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.title, "OIDC Login");
    assert!(saved.enabled);
    assert_eq!(
        saved.options["description"],
        json!("Updated corporate OIDC")
    );
    assert!(saved.options.get("name").is_none());
    assert!(saved.options.get("title").is_none());
    assert!(saved.options.get("enabled").is_none());
    assert_eq!(
        saved.options["extension_config"],
        json!({
            "issuer_url": "https://idp.example.com",
            "allow_signup": true
        })
    );
}

#[tokio::test]
async fn console_auth_center_update_config_rejects_body_name_mismatch() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/password-local/config")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "other-authenticator",
                        "title": "Password",
                        "enabled": true,
                        "description": "Local password authentication"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["code"], json!("authenticator_name"));
}

#[tokio::test]
async fn console_auth_center_update_config_rejects_blank_title() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/password-local/config")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "password-local",
                        "title": "   ",
                        "enabled": true,
                        "description": "Local password authentication"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["code"], json!("title"));
}

#[tokio::test]
async fn console_auth_center_update_config_requires_session_csrf_and_manage_permission() {
    let app = test_app().await;
    let body = json!({
        "name": "password-local",
        "title": "Password",
        "enabled": true,
        "description": "Local password authentication"
    })
    .to_string();

    let missing_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/password-local/config")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_session.status(), StatusCode::UNAUTHORIZED);

    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/password-local/config")
                .header("cookie", &root_cookie)
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "auth-center-config-viewer",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "auth_center_view_only").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "auth_center_view_only",
        &["user.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["auth_center_view_only"],
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "auth-center-config-viewer", "temp-pass").await;

    let missing_manage_permission = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/password-local/config")
                .header("cookie", &member_cookie)
                .header("x-csrf-token", &member_csrf)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_manage_permission.status(), StatusCode::FORBIDDEN);
}
