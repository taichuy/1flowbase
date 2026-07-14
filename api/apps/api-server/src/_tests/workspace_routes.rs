use crate::_tests::support::{
    login_and_capture_cookie, seed_workspace, test_app_with_database_url,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn current_workspace_id(app: &axum::Router, cookie: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    payload["data"]["session"]["current_workspace_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_member(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    account: &str,
    password: &str,
) {
    let response = app
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
                        "phone": null,
                        "password": password,
                        "name": account,
                        "nickname": account,
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

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn workspaces_route_lists_accessible_workspaces_with_current_marker() {
    let (app, database_url) = test_app_with_database_url().await;
    let target_workspace_id = seed_workspace(&database_url, "Secondary Workspace").await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let current_workspace_id = current_workspace_id(&app, &cookie).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/workspaces")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let workspaces = payload["data"].as_array().unwrap();

    assert_eq!(workspaces.len(), 2);
    assert_eq!(workspaces[0]["id"], current_workspace_id);
    assert_eq!(workspaces[0]["is_current"], true);
    assert_eq!(workspaces[1]["id"], target_workspace_id.to_string());
    assert_eq!(workspaces[1]["is_current"], false);
}

#[tokio::test]
async fn switch_workspace_route_updates_current_workspace_and_returns_new_csrf() {
    let (app, database_url) = test_app_with_database_url().await;
    let target_workspace_id = seed_workspace(&database_url, "Workspace Switch Target").await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "workspace_id": target_workspace_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let next_csrf = payload["data"]["csrf_token"].as_str().unwrap().to_string();

    assert_ne!(next_csrf, csrf);
    assert_eq!(
        payload["data"]["actor"]["current_workspace_id"],
        target_workspace_id.to_string()
    );
    assert_eq!(
        payload["data"]["session"]["current_workspace_id"],
        target_workspace_id.to_string()
    );

    let follow_up = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(follow_up.status(), StatusCode::OK);
    let body = to_bytes(follow_up.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["data"]["session"]["current_workspace_id"],
        target_workspace_id.to_string()
    );
    assert_eq!(payload["data"]["csrf_token"], next_csrf);
}

#[tokio::test]
async fn switch_workspace_route_rejects_inaccessible_target_for_member() {
    let (app, database_url) = test_app_with_database_url().await;
    let target_workspace_id = seed_workspace(&database_url, "Forbidden Workspace").await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "workspace-member",
        "member-pass",
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "workspace-member", "member-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", member_cookie)
                .header("x-csrf-token", member_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "workspace_id": target_workspace_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn current_workspace_route_reads_and_updates_workspace_metadata() {
    let app = test_app_with_database_url().await.0;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/workspace")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let body = to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["name"], "1flowbase");

    let patch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/workspace")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Flowse Workspace",
                        "logo_url": "https://example.com/workspace.png",
                        "introduction": "workspace intro"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(patch_response.status(), StatusCode::OK);
    let body = to_bytes(patch_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["name"], "Flowse Workspace");
    assert_eq!(
        payload["data"]["logo_url"],
        Value::String("https://example.com/workspace.png".to_string())
    );
    assert_eq!(payload["data"]["introduction"], "workspace intro");
}
