use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_url, test_app,
    test_app_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::{I18nCatalogRepository, UpsertCatalogTranslationInput};
use domain::AuthenticatorRecord;
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogTranslation};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn seed_authentication_translation(
    state: &crate::app_state::ApiState,
    msgid: &str,
    value: &str,
) {
    let workspace_id = state.bootstrap_workspace_id;
    let catalog_state =
        I18nCatalogRepository::bootstrap_workspace_catalog_state(&state.store, workspace_id)
            .await
            .unwrap();
    I18nCatalogRepository::upsert_catalog_override(
        &state.store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: CatalogTranslation::new(
                CatalogMessageIdentity::new(
                    CatalogModuleId::new("@taichuy/platform/authentication").unwrap(),
                    msgid,
                )
                .unwrap(),
                CatalogLocale::new("zh_Hans").unwrap(),
                value,
            )
            .unwrap(),
            expected_revision: catalog_state.revision(),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn ac_012_013_auth_center_localizes_builtin_displays_and_preserves_customized_title() {
    let (state, _) = test_api_state_with_database_url().await;
    for (msgid, value) in [
        ("Password", "密码"),
        ("Authenticator ID", "认证器 ID"),
        ("Authentication event", "认证事件"),
    ] {
        seed_authentication_translation(&state, msgid, value).await;
    }
    let custom_id = Uuid::now_v7();
    state
        .store
        .upsert_authenticator(&AuthenticatorRecord {
            id: custom_id,
            auth_type: "password-local".to_owned(),
            title: "Staff Password".to_owned(),
            enabled: true,
            is_builtin: false,
            sort_order: 10,
            public_ui_block: control_plane::auth::public_ui::PASSWORD_LOCAL_PUBLIC_UI_BLOCK
                .to_owned(),
            options: json!({}),
        })
        .await
        .unwrap();
    let app = crate::app_with_state(state);
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/auth-center/overview")
                .header("cookie", &cookie)
                .header("x-1flowbase-locale", "zh-Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let authenticators = payload["data"]["authenticators"].as_array().unwrap();
    let builtin = authenticators
        .iter()
        .find(|item| item["id"] == json!(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID))
        .unwrap();
    assert_eq!(builtin["title"], "密码");
    assert_eq!(builtin["config_values"]["title"], "密码");
    assert_eq!(builtin["auth_type"], "password-local");
    let runtime = builtin["context_variables"].as_array().unwrap();
    assert!(runtime.iter().any(
        |item| item["label"] == "认证器 ID" && item["member_path"] == "inputs.authenticator_id"
    ));
    assert!(runtime
        .iter()
        .any(|item| item["label"] == "认证事件" && item["member_path"] == "inputs.auth_event"));
    assert!(runtime
        .iter()
        .any(|item| item["label"] == "API" && item["member_path"] == "api"));
    assert_eq!(
        authenticators
            .iter()
            .find(|item| item["id"] == custom_id.to_string())
            .unwrap()["title"],
        "Staff Password"
    );

    let english = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/auth-center/overview")
                .header("cookie", cookie)
                .header("x-1flowbase-locale", "en-US")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let english: serde_json::Value =
        serde_json::from_slice(&to_bytes(english.into_body(), usize::MAX).await.unwrap()).unwrap();
    let builtin = english["data"]["authenticators"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == json!(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID))
        .unwrap();
    assert_eq!(builtin["title"], "Password");
}

#[tokio::test]
async fn console_auth_center_overview_lists_authenticators_with_schema_form_values() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    state
        .store
        .upsert_authenticator(&AuthenticatorRecord {
            id: Uuid::now_v7(),
            auth_type: "oidc".to_string(),
            title: "OIDC".to_string(),
            enabled: false,
            is_builtin: false,
            sort_order: 0,
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
                    },
                    {
                        "key": "public_ui_block",
                        "label": "Legacy public UI Block",
                        "type": "string",
                        "control": "textarea",
                        "required": true
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
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
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
        payload["data"]["default_authenticator_id"],
        json!(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
    );

    let authenticators = payload["data"]["authenticators"].as_array().unwrap();
    let password_local = authenticators
        .iter()
        .find(|authenticator| authenticator["id"] == json!(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID))
        .expect("password-local should be visible in auth center overview");
    assert_eq!(password_local["auth_type"], json!("password-local"));
    assert_eq!(password_local["title"], json!("Password"));
    assert_eq!(password_local["enabled"], json!(true));
    assert_eq!(password_local["is_builtin"], json!(true));
    assert_eq!(password_local["sort_order"], json!(0));
    assert!(password_local["public_ui_block"]
        .as_str()
        .is_some_and(
            |source| source.contains("export default function PasswordLocalAuth")
                && source.contains("onSubmit={submitSignIn}")
                && source.contains("self_registration_enabled === true")
                && source.contains("onClick={() => setMode")
                && !source.contains("function main")
        ));
    assert_eq!(
        password_local["public_variables"],
        json!({
            "title": "Password",
            "description": "Local password authentication",
            "enabled": true,
            "self_registration_enabled": false
        })
    );
    assert_eq!(
        password_local["interface_path_prefixes"],
        json!(["/api/public/"])
    );
    let context_variables = password_local["context_variables"].as_array().unwrap();
    assert_eq!(context_variables.len(), 7);
    for (label, member_path, schema_type) in [
        (
            "Authenticator title",
            "inputs.public_variables.title",
            "string",
        ),
        (
            "Description",
            "inputs.public_variables.description",
            "string",
        ),
        ("Enabled", "inputs.public_variables.enabled", "boolean"),
        (
            "Allow self registration",
            "inputs.public_variables.self_registration_enabled",
            "boolean",
        ),
    ] {
        assert!(context_variables.iter().any(|variable| {
            variable["group"] == "configuration"
                && variable["label"] == label
                && variable["member_path"] == member_path
                && variable["schema"]["type"] == schema_type
        }));
    }
    for member_path in ["inputs.authenticator_id", "inputs.auth_event", "api"] {
        assert!(context_variables.iter().any(|variable| {
            variable["group"] == "runtime" && variable["member_path"] == member_path
        }));
    }
    assert!(!context_variables.iter().any(|variable| {
        matches!(
            variable["member_path"].as_str(),
            Some("inputs.public_variables" | "inputs.public_variables.public_ui_block")
        )
    }));
    assert!(!context_variables
        .iter()
        .any(|variable| variable["member_path"]
            .as_str()
            .is_some_and(|path| path.contains("secret"))));
    assert_eq!(
        password_local["config_values"]["description"],
        json!("Local password authentication")
    );
    assert!(password_local["config_values"].get("name").is_none());
    assert_eq!(password_local["config_values"]["title"], json!("Password"));
    assert_eq!(password_local["config_values"]["enabled"], json!(true));
    assert!(password_local["config_values"]
        .get("public_ui_block")
        .is_none());
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
        .all(|field| field["key"] != "name"));
    assert!(password_local["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "description"
            && field["control"] == "textarea"
            && field["required"] == false));
    assert!(password_local["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field["key"] != "public_ui_block"));

    let oidc = authenticators
        .iter()
        .find(|authenticator| authenticator["title"] == "OIDC")
        .expect("custom authenticator should be visible in auth center overview");
    assert!(oidc["public_variables"].is_null());
    assert_eq!(oidc["enabled"], json!(false));
    assert_eq!(oidc["sort_order"], json!(0));
    assert_eq!(
        oidc["public_ui_block"],
        json!("export default { main } satisfies BlockModule;")
    );
    assert_eq!(
        oidc["config_values"]["description"],
        json!("Corporate OIDC")
    );
    assert!(oidc["config_values"].get("name").is_none());
    assert_eq!(oidc["config_values"]["title"], json!("OIDC"));
    assert_eq!(oidc["config_values"]["enabled"], json!(false));
    assert!(oidc["config_values"].get("public_ui_block").is_none());
    assert!(oidc["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "issuer_url" && field["pattern"] == "^https://"));
    assert!(!oidc["config_schema"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["key"] == "public_ui_block"));
    assert_eq!(
        oidc["config_values"]["extension_config"],
        json!({
            "issuer_url": "https://idp.example.com"
        })
    );
    assert_eq!(
        payload["data"]["supported_auth_types"],
        json!(["password-local"])
    );

    let round_trip = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{}/config",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": password_local["config_values"]["title"],
                        "enabled": password_local["config_values"]["enabled"],
                        "description": password_local["config_values"]["description"],
                        "self_registration_enabled": password_local["config_values"]
                            ["self_registration_enabled"],
                        "extension_config": password_local["config_values"]["extension_config"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(round_trip.status(), StatusCode::OK);
}

#[tokio::test]
async fn console_auth_center_creates_copies_reorders_and_deletes_authenticators() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/auth-center/authenticators")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auth_type": "password-local",
                        "title": "Staff Password",
                        "description": "Staff-only password login",
                        "enabled": true,
                        "sort_order": 20
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created: serde_json::Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let staff_password_id = created["data"]["id"].as_str().unwrap().to_string();
    let staff_password_uuid = Uuid::parse_str(&staff_password_id).unwrap();
    assert_eq!(created["data"]["auth_type"], json!("password-local"));
    assert_eq!(created["data"]["sort_order"], json!(20));
    assert_eq!(
        created["data"]["config_values"]["description"],
        json!("Staff-only password login")
    );
    assert!(created["data"]["public_ui_block"]
        .as_str()
        .is_some_and(
            |source| source.contains("export default function PasswordLocalAuth")
                && source.contains("onSubmit={submitSignUp}")
        ));

    let update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{staff_password_id}/config"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Staff Password",
                        "enabled": true,
                        "description": "Staff-only password login",
                        "self_registration_enabled": false,
                        "extension_config": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update.status(), StatusCode::OK);

    let copy = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{staff_password_id}/copy"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Staff Password Backup",
                        "sort_order": 30
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(copy.status(), StatusCode::CREATED);
    let copied: serde_json::Value =
        serde_json::from_slice(&to_bytes(copy.into_body(), usize::MAX).await.unwrap()).unwrap();
    let staff_backup_id = copied["data"]["id"].as_str().unwrap().to_string();
    let staff_backup_uuid = Uuid::parse_str(&staff_backup_id).unwrap();
    assert_eq!(copied["data"]["auth_type"], json!("password-local"));
    assert_eq!(copied["data"]["title"], json!("Staff Password Backup"));
    assert_eq!(copied["data"]["enabled"], json!(false));
    assert_eq!(copied["data"]["sort_order"], json!(30));
    assert_eq!(
        copied["data"]["config_values"]["description"],
        json!("Staff-only password login")
    );
    assert_eq!(
        copied["data"]["public_ui_block"],
        created["data"]["public_ui_block"]
    );

    let reorder = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/auth-center/authenticators/order")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "ids": [
                            staff_backup_uuid,
                            domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
                            staff_password_uuid
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reorder.status(), StatusCode::OK);
    let reordered: serde_json::Value =
        serde_json::from_slice(&to_bytes(reorder.into_body(), usize::MAX).await.unwrap()).unwrap();
    let ids = reordered["data"]["authenticators"]
        .as_array()
        .unwrap()
        .iter()
        .map(|authenticator| authenticator["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            staff_backup_id.clone(),
            domain::PASSWORD_LOCAL_AUTHENTICATOR_ID.to_string(),
            staff_password_id.clone()
        ]
    );
    assert_eq!(
        reordered["data"]["authenticators"][0]["sort_order"],
        json!(0)
    );
    assert_eq!(
        reordered["data"]["authenticators"][1]["sort_order"],
        json!(10)
    );
    assert_eq!(
        reordered["data"]["authenticators"][2]["sort_order"],
        json!(20)
    );

    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{staff_backup_id}"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);

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
    assert!(payload["data"]["authenticators"]
        .as_array()
        .unwrap()
        .iter()
        .all(|authenticator| authenticator["id"] != staff_backup_id));
}

#[tokio::test]
async fn console_auth_center_lifecycle_rejects_unsafe_inputs() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = PgPool::connect(&database_url).await.unwrap();
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let unknown_type = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/auth-center/authenticators")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "auth_type": "oidc",
                        "title": "OIDC",
                        "enabled": true,
                        "sort_order": 20
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_type.status(), StatusCode::BAD_REQUEST);
    let unknown_type_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(unknown_type.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(unknown_type_payload["code"], json!("auth_type"));

    let builtin_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{}",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(builtin_delete.status(), StatusCode::BAD_REQUEST);
    let builtin_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(builtin_delete.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(builtin_payload["code"], json!("builtin_authenticator"));

    let bound_password_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into authenticators (id, auth_type, title, enabled, is_builtin, sort_order, options)
        values ($1, 'password-local', 'Bound Password', true, false, 40, '{}')
        "#,
    )
    .bind(bound_password_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into user_auth_identities (id, user_id, authenticator_id, subject_type, subject_value, metadata)
        select $1, id, $2, 'account', account, '{}'
        from users
        where account = 'root'
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(bound_password_id)
    .execute(&pool)
    .await
    .unwrap();

    let bound_delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{bound_password_id}"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bound_delete.status(), StatusCode::CONFLICT);
    let bound_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(bound_delete.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        bound_payload["code"],
        json!("authenticator_identity_bindings")
    );

    let missing_password_id = Uuid::now_v7();
    for (ids, expected_code) in [
        (
            json!([
                domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
                domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
                bound_password_id
            ]),
            "authenticator_order_duplicate",
        ),
        (
            json!([domain::PASSWORD_LOCAL_AUTHENTICATOR_ID]),
            "authenticator_order_missing",
        ),
        (
            json!([
                domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
                bound_password_id,
                missing_password_id
            ]),
            "authenticator_order_unknown",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/console/settings/auth-center/authenticators/order")
                    .header("cookie", &root_cookie)
                    .header("x-csrf-token", &root_csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "ids": ids }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(payload["code"], json!(expected_code));
    }
}

#[tokio::test]
async fn console_auth_center_create_requires_session_csrf_and_manage_permission() {
    let app = test_app().await;
    let body = json!({
        "auth_type": "password-local",
        "title": "Staff Password",
        "enabled": true,
        "sort_order": 20
    })
    .to_string();

    let missing_session = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/auth-center/authenticators")
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
                .method("POST")
                .uri("/api/console/settings/auth-center/authenticators")
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
        "auth-center-create-viewer",
        "temp-pass",
    )
    .await;
    create_role(
        &app,
        &root_cookie,
        &root_csrf,
        "auth_center_create_view_only",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "auth_center_create_view_only",
        &["user.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["auth_center_create_view_only"],
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "auth-center-create-viewer", "temp-pass").await;

    let missing_manage_permission = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/auth-center/authenticators")
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
        .clone()
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
    let oidc_id = Uuid::now_v7();
    state
        .store
        .upsert_authenticator(&AuthenticatorRecord {
            id: oidc_id,
            auth_type: "oidc".to_string(),
            title: "OIDC".to_string(),
            enabled: false,
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
                .method("POST")
                .uri(format!(
                    "/api/console/settings/auth-center/authenticators/{oidc_id}/actions/enable"
                ))
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
        .find(|authenticator| authenticator["id"] == json!(oidc_id))
        .expect("enabled authenticator should be visible");
    assert_eq!(oidc["enabled"], json!(true));
}

#[tokio::test]
async fn console_auth_center_update_config_updates_editable_fields_and_preserves_schema_values() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    let oidc_id = Uuid::now_v7();
    state
        .store
        .upsert_authenticator(&AuthenticatorRecord {
            id: oidc_id,
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
                    "/api/console/settings/auth-center/authenticators/{oidc_id}/config"
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
        .find_authenticator(oidc_id)
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
                    "/api/console/settings/auth-center/authenticators/{oidc_id}/public-ui-block"
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
        .find_authenticator(oidc_id)
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
                    "/api/console/settings/auth-center/authenticators/{}/config",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
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
                    "/api/console/settings/auth-center/authenticators/{}/public-ui-block",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
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
                    "/api/console/settings/auth-center/authenticators/{}/config",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
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
                    "/api/console/settings/auth-center/authenticators/{}/config",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
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
                    "/api/console/settings/auth-center/authenticators/{}/config",
                    domain::PASSWORD_LOCAL_AUTHENTICATOR_ID
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
