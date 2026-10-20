use super::*;

#[tokio::test]
async fn console_auth_center_updates_authenticator_enabled_state() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    let oidc_id = Uuid::now_v7();
    state
        .store
        .upsert_login_entry(&LoginEntryRecord {
            id: oidc_id,
            connection_id: oidc_id,
            auth_type: "oidc".to_string(),
            title: "OIDC".to_string(),
            enabled: true,
            is_builtin: false,
            sort_order: 10,
            public_ui_block: "export default { main } satisfies BlockModule;".to_string(),
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
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{oidc_id}/enabled"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "enabled": false }).to_string()))
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
    let login_entries = payload["data"]["login_entries"].as_array().unwrap();
    let oidc = login_entries
        .iter()
        .find(|authenticator| authenticator["id"] == json!(oidc_id))
        .expect("updated authenticator should be visible");
    assert_eq!(oidc["enabled"], json!(false));
}

#[tokio::test]
async fn console_auth_center_update_config_updates_editable_fields_and_preserves_schema_values() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    let oidc_id = Uuid::now_v7();
    state
        .store
        .upsert_login_entry(&LoginEntryRecord {
            id: oidc_id,
            connection_id: oidc_id,
            auth_type: "oidc".to_string(),
            title: "OIDC".to_string(),
            enabled: false,
            is_builtin: false,
            sort_order: 10,
            public_ui_block: "export default { main } satisfies BlockModule;".to_string(),
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
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{oidc_id}/config"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "OIDC Login",
                        "enabled": true,
                        "description": "Updated corporate OIDC",
                        "self_registration_enabled": false
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
    assert_eq!(authenticator["id"], json!(oidc_id));
    assert_eq!(authenticator["title"], json!("OIDC Login"));
    assert_eq!(authenticator["enabled"], json!(true));
    assert!(authenticator["config_values"].get("name").is_none());
    assert_eq!(authenticator["config_values"]["title"], json!("OIDC Login"));
    assert_eq!(authenticator["config_values"]["enabled"], json!(true));
    assert_eq!(
        authenticator["public_ui_block"],
        json!("export default { main } satisfies BlockModule;")
    );
    assert!(authenticator["config_values"]
        .get("public_ui_block")
        .is_none());
    assert_eq!(
        authenticator["config_values"]["description"],
        json!("Updated corporate OIDC")
    );
    assert!(authenticator["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "issuer_url" && field["pattern"] == "^https://"));
    assert!(!authenticator["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "public_ui_block" && field["required"] == true));
    assert_eq!(
        authenticator["config_values"]["extension_config"],
        json!({
            "issuer_url": "https://idp.example.com"
        })
    );

    let saved = state
        .store
        .find_login_entry(oidc_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.title, "OIDC Login");
    assert!(saved.enabled);
    assert_eq!(
        saved.public_ui_block,
        "export default { main } satisfies BlockModule;"
    );
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

    let updated_block = "export default { main: customMain } satisfies BlockModule;";
    let block_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{oidc_id}/public-ui-block"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "public_ui_block": updated_block }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(block_response.status(), StatusCode::OK);
    let block_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(block_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        block_payload["data"]["public_ui_block"],
        json!(updated_block)
    );
    assert!(block_payload["data"]["config_values"]
        .get("public_ui_block")
        .is_none());
    let saved = state
        .store
        .find_login_entry(oidc_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.public_ui_block, updated_block);
}

#[tokio::test]
async fn console_auth_center_update_config_rejects_blank_title() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{}/config",
                    domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "   ",
                        "enabled": true,
                        "description": "Local password authentication",
                        "self_registration_enabled": false
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
async fn console_auth_center_public_ui_block_rejects_blank_source() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{}/public-ui-block",
                    domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "public_ui_block": "   " }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["code"], json!("public_ui_block"));
}

#[tokio::test]
async fn console_auth_center_update_config_requires_session_csrf_and_manage_permission() {
    let app = test_app().await;
    let body = json!({
        "title": "Password",
        "enabled": true,
        "description": "Local password authentication",
        "self_registration_enabled": false
    })
    .to_string();

    let missing_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{}/config",
                    domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID
                ))
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
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{}/config",
                    domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID
                ))
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
                .uri(format!(
                    "/api/console/settings/auth-center/login-entries/{}/config",
                    domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID
                ))
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
