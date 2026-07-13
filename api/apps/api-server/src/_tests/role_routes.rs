use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn role_routes_create_replace_permissions_and_protect_root() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "qa",
                        "name": "QA",
                        "introduction": "qa role"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);

    let replace_permissions_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/qa/permissions")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "permission_codes": [
                            "application.view.own",
                            "application.edit.own"
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        replace_permissions_response.status(),
        StatusCode::NO_CONTENT
    );

    let role_permissions_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/roles/qa/permissions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(role_permissions_response.status(), StatusCode::OK);
    let body = to_bytes(role_permissions_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["role_code"].as_str(), Some("qa"));

    let protect_root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/root/permissions")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "permission_codes": ["workspace.configure.all"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(protect_root_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn role_routes_roundtrip_data_policy_and_protect_root_policy_mutation() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "data_editor",
                        "name": "Data Editor",
                        "introduction": "data editor role"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let default_policy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/roles/data_editor/data-policy")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(default_policy_response.status(), StatusCode::OK);
    let default_body = to_bytes(default_policy_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let default_payload: serde_json::Value = serde_json::from_slice(&default_body).unwrap();
    assert_eq!(
        default_payload["data"]["default_policy"]["can_view"].as_bool(),
        Some(false)
    );
    assert_eq!(
        default_payload["data"]["default_policy"]["default_view_scope"].as_str(),
        Some("own")
    );

    let models_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_response.status(), StatusCode::OK);
    let models_body = to_bytes(models_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let models_payload: serde_json::Value = serde_json::from_slice(&models_body).unwrap();
    let model_id = models_payload["data"][0]["id"]
        .as_str()
        .expect("bootstrapped test app should expose at least one data model");

    let replace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/data_editor/data-policy")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "default_policy": {
                            "can_view": true,
                            "can_create": true,
                            "can_update": false,
                            "can_delete": false,
                            "default_view_scope": "own",
                            "default_update_scope": "own",
                            "default_delete_scope": "own"
                        },
                        "model_policies": [{
                            "data_model_id": model_id,
                            "can_create_override": false,
                            "view_scope_override": "scope_all",
                            "update_scope_override": null,
                            "delete_scope_override": null
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace_response.status(), StatusCode::OK);
    let replace_body = to_bytes(replace_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let replace_payload: serde_json::Value = serde_json::from_slice(&replace_body).unwrap();
    assert_eq!(
        replace_payload["data"]["role_code"].as_str(),
        Some("data_editor")
    );
    assert_eq!(
        replace_payload["data"]["default_policy"]["can_create"].as_bool(),
        Some(true)
    );
    assert_eq!(
        replace_payload["data"]["model_policies"][0]["view_scope_override"].as_str(),
        Some("scope_all")
    );
    assert_eq!(
        replace_payload["data"]["model_policies"][0]["can_create_override"].as_bool(),
        Some(false)
    );

    let reject_default_system_all_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/data_editor/data-policy")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "default_policy": {
                            "can_view": true,
                            "can_create": true,
                            "can_update": true,
                            "can_delete": true,
                            "default_view_scope": "system_all",
                            "default_update_scope": "scope_all",
                            "default_delete_scope": "own"
                        },
                        "model_policies": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        reject_default_system_all_response.status(),
        StatusCode::BAD_REQUEST
    );

    let reject_model_system_all_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/data_editor/data-policy")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "default_policy": {
                            "can_view": true,
                            "can_create": true,
                            "can_update": true,
                            "can_delete": true,
                            "default_view_scope": "scope_all",
                            "default_update_scope": "scope_all",
                            "default_delete_scope": "own"
                        },
                        "model_policies": [{
                            "data_model_id": model_id,
                            "view_scope_override": "system_all",
                            "update_scope_override": null,
                            "delete_scope_override": null
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        reject_model_system_all_response.status(),
        StatusCode::BAD_REQUEST
    );

    let protect_root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/root/data-policy")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "default_policy": {
                            "can_view": true,
                            "can_create": true,
                            "can_update": true,
                            "can_delete": true,
                            "default_view_scope": "system_all",
                            "default_update_scope": "system_all",
                            "default_delete_scope": "system_all"
                        },
                        "model_policies": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(protect_root_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn role_routes_roundtrip_policy_flags_and_protect_default_role_from_clear() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "qa",
                        "name": "QA",
                        "introduction": "qa role",
                        "auto_grant_new_permissions": true,
                        "is_default_member_role": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_payload: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    assert_eq!(
        create_payload["data"]["auto_grant_new_permissions"].as_bool(),
        Some(true)
    );
    assert_eq!(
        create_payload["data"]["is_default_member_role"].as_bool(),
        Some(false)
    );

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/settings/roles/qa")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "QA Updated",
                        "introduction": "updated qa role",
                        "auto_grant_new_permissions": false,
                        "is_default_member_role": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::NO_CONTENT);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let list_payload: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
    let roles = list_payload["data"].as_array().unwrap();
    let qa = roles
        .iter()
        .find(|role| role["code"].as_str() == Some("qa"))
        .unwrap();
    let manager = roles
        .iter()
        .find(|role| role["code"].as_str() == Some("manager"))
        .unwrap();

    assert_eq!(qa["name"].as_str(), Some("QA Updated"));
    assert_eq!(qa["auto_grant_new_permissions"].as_bool(), Some(false));
    assert_eq!(qa["is_default_member_role"].as_bool(), Some(true));
    assert_eq!(manager["is_default_member_role"].as_bool(), Some(false));

    let clear_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/settings/roles/qa")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "QA Updated",
                        "introduction": "updated qa role",
                        "auto_grant_new_permissions": false,
                        "is_default_member_role": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(clear_response.status(), StatusCode::BAD_REQUEST);
    let clear_body = to_bytes(clear_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let clear_payload: serde_json::Value = serde_json::from_slice(&clear_body).unwrap();
    assert_eq!(
        clear_payload["code"].as_str(),
        Some("default_member_role_required")
    );
}

#[tokio::test]
async fn root_can_login_create_member_create_role_and_bind_permissions() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root@example.com", "change-me").await;

    let member_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": "qa-1",
                        "email": "qa-1@example.com",
                        "phone": null,
                        "password": "temp-pass",
                        "name": "QA 1",
                        "nickname": "QA 1",
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(member_response.status(), StatusCode::CREATED);

    let role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "viewer",
                        "name": "Viewer",
                        "introduction": "viewer role"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(role_response.status(), StatusCode::CREATED);

    let bind_permissions_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/viewer/permissions")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "permission_codes": ["application.view.own"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(bind_permissions_response.status(), StatusCode::NO_CONTENT);

    let permissions_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/roles/permission-options")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(permissions_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn settings_roles_feature_grants_roles_api_without_legacy_role_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "settings-route-role-member",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "settings_route_roles_only").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "settings_route_roles_only",
        &["settings_feature.access.system.roles"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["settings_route_roles_only"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "settings-route-role-member", "temp-pass").await;

    let roles_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/roles")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(roles_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn legacy_role_permission_without_settings_route_permission_is_forbidden_on_roles_api() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy-role-permission-member",
        "temp-pass",
    )
    .await;
    create_role(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_role_permission_only",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_role_permission_only",
        &["role_permission.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["legacy_role_permission_only"],
    )
    .await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "legacy-role-permission-member", "temp-pass").await;

    let roles_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/roles")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(roles_response.status(), StatusCode::FORBIDDEN);
}
