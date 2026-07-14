use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

async fn create_member(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    account: &str,
    password: &str,
) -> String {
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": "13800000000",
                        "password": password,
                        "name": "Manager 1",
                        "nickname": "Manager 1",
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

    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_member: serde_json::Value = serde_json::from_slice(&body).unwrap();
    created_member["data"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn member_routes_create_disable_and_reset_password() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &cookie, &csrf, "manager-1", "temp-pass").await;

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/members")
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
    assert!(list_payload["data"].is_array());
    assert!(list_payload["meta"].is_null());

    let replace_roles_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{member_id}/roles"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": ["admin"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(replace_roles_response.status(), StatusCode::NO_CONTENT);

    let reset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{member_id}/reset-password"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "new_password": "next-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_response.status(), StatusCode::NO_CONTENT);

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/settings/members/{member_id}/disable"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(disable_response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn root_member_profile_and_roles_can_be_updated_without_removing_root_role() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(session_response.status(), StatusCode::OK);
    let session_body = to_bytes(session_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let session_payload: serde_json::Value = serde_json::from_slice(&session_body).unwrap();
    let root_user_id = session_payload["data"]["actor"]["id"].as_str().unwrap();

    let create_role_response = app
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
                        "code": "operator",
                        "name": "Operator",
                        "introduction": "operator role",
                        "auto_grant_new_permissions": false,
                        "is_default_member_role": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_role_response.status(), StatusCode::CREATED);

    let profile_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/settings/members/{root_user_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Root Next",
                        "nickname": "Captain Root",
                        "email": "root-next@example.com",
                        "phone": "13900000000",
                        "introduction": "updated root profile"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(profile_response.status(), StatusCode::OK);
    let profile_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(profile_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(profile_payload["data"]["account"], "root");
    assert_eq!(profile_payload["data"]["name"], "Root Next");
    assert_eq!(profile_payload["data"]["nickname"], "Captain Root");
    assert_eq!(profile_payload["data"]["email"], "root-next@example.com");
    assert_eq!(profile_payload["data"]["phone"], "13900000000");
    assert_eq!(
        profile_payload["data"]["introduction"],
        "updated root profile"
    );

    let append_role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/members/{root_user_id}/roles"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": ["root", "operator"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(append_role_response.status(), StatusCode::NO_CONTENT);

    let remove_root_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/members/{root_user_id}/roles"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": ["operator"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(remove_root_response.status(), StatusCode::FORBIDDEN);
    let remove_root_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(remove_root_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(remove_root_payload["code"], "root_user_immutable");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/members")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let root_member = list_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["id"] == root_user_id)
        .unwrap();
    assert_eq!(root_member["name"], "Root Next");
    assert_eq!(root_member["role_codes"], json!(["operator", "root"]));

    let reset_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{root_user_id}/reset-password"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "new_password": "root-next-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn member_creation_uses_workspace_default_member_role() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_role_response = app
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
                        "auto_grant_new_permissions": false,
                        "is_default_member_role": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_role_response.status(), StatusCode::CREATED);

    let create_response = app
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
                        "phone": "13800000000",
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

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_payload: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    assert_eq!(
        create_payload["data"]["default_display_role"].as_str(),
        Some("qa")
    );
    assert_eq!(
        create_payload["data"]["role_codes"].as_array().unwrap(),
        &vec![serde_json::Value::String("qa".to_string())]
    );
}

#[tokio::test]
async fn reset_password_invalidates_member_session() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "manager-2", "temp-pass").await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "manager-2", "temp-pass").await;

    let reset_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{member_id}/reset-password"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "new_password": "next-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_response.status(), StatusCode::NO_CONTENT);

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(session_response.status(), StatusCode::UNAUTHORIZED);

    let new_login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "identifier": "manager-2",
                        "password": "next-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(new_login_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn delete_member_physically_removes_identity_records() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "delete-me", "temp-pass").await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "delete-me", "temp-pass").await;

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/settings/members/{member_id}"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/members")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(!list_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|member| member["id"] == member_id));

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(session_response.status(), StatusCode::UNAUTHORIZED);

    let old_login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "identifier": "delete-me",
                        "password": "temp-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_login_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn enable_member_restores_disabled_member_login() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "restore-me", "temp-pass").await;

    let disable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/settings/members/{member_id}/disable"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disable_response.status(), StatusCode::NO_CONTENT);

    let disabled_login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "identifier": "restore-me",
                        "password": "temp-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled_login_response.status(), StatusCode::FORBIDDEN);

    let enable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/settings/members/{member_id}/enable"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable_response.status(), StatusCode::NO_CONTENT);

    let enabled_login_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "identifier": "restore-me",
                        "password": "temp-pass"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enabled_login_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn disable_root_member_is_forbidden() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(session_response.status(), StatusCode::OK);
    let session_body = to_bytes(session_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let session_payload: serde_json::Value = serde_json::from_slice(&session_body).unwrap();
    let root_user_id = session_payload["data"]["actor"]["id"].as_str().unwrap();

    let disable_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/members/{root_user_id}/disable"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(disable_response.status(), StatusCode::FORBIDDEN);
    let disable_body = to_bytes(disable_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let disable_payload: serde_json::Value = serde_json::from_slice(&disable_body).unwrap();
    assert_eq!(disable_payload["code"], "root_user_immutable");
}

#[tokio::test]
async fn delete_root_member_is_forbidden() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let session_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(session_response.status(), StatusCode::OK);
    let session_body = to_bytes(session_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let session_payload: serde_json::Value = serde_json::from_slice(&session_body).unwrap();
    let root_user_id = session_payload["data"]["actor"]["id"].as_str().unwrap();

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/settings/members/{root_user_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::FORBIDDEN);
    let delete_body = to_bytes(delete_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let delete_payload: serde_json::Value = serde_json::from_slice(&delete_body).unwrap();
    assert_eq!(delete_payload["code"], "root_user_immutable");
}

#[tokio::test]
async fn delete_current_member_is_forbidden() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "self-delete", "temp-pass").await;

    let replace_roles_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{member_id}/roles"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": ["admin"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace_roles_response.status(), StatusCode::NO_CONTENT);

    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "self-delete", "temp-pass").await;

    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/settings/members/{member_id}"))
                .header("cookie", &member_cookie)
                .header("x-csrf-token", &member_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::FORBIDDEN);
    let delete_body = to_bytes(delete_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let delete_payload: serde_json::Value = serde_json::from_slice(&delete_body).unwrap();
    assert_eq!(delete_payload["code"], "member_self_delete_forbidden");
}
