use crate::_tests::support::{
    login_and_capture_cookie, seed_session, seed_workspace, test_api_state_with_database_url,
    test_app, test_app_with_database_url, test_config,
};
use crate::app_with_state_and_config;
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use control_plane::ports::{AuthRepository, SessionStore};
use domain::SessionRecord;
use serde_json::json;
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn production_login_cookie_is_marked_secure() {
    let (state, _) = test_api_state_with_database_url().await;
    let mut config = test_config();
    config.env = api_server::config::ApiEnvironment::Production;
    config.cookie_secure = true;
    config.cors_allowed_origins = Some(vec![header::HeaderValue::from_static(
        "https://console.example.com",
    )]);
    let state = std::sync::Arc::new(api_server::app_state::ApiState {
        cookie_secure: true,
        ..(*state).clone()
    });
    let app = app_with_state_and_config(state, &config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/auth/sign-in")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "identifier": "root",
                        "password": "change-me"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("login should set session cookie");

    assert!(cookie.contains("Secure"));
}

#[tokio::test]
async fn session_route_returns_wrapped_actor_payload_and_csrf_token() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/session")
                .header(header::ORIGIN, "http://127.0.0.1:3100")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let cors_header = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .cloned();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(payload.get("data").is_some());
    assert!(payload.get("meta").is_some());
    assert_eq!(
        cors_header,
        Some(header::HeaderValue::from_static("http://127.0.0.1:3100"))
    );
    assert_eq!(payload["data"]["actor"]["account"], "root");
    assert!(payload["data"]["actor"]["current_workspace_id"].is_string());
    assert!(payload["data"]["session"]["current_workspace_id"].is_string());
    assert_eq!(payload["data"]["csrf_token"], csrf);
    assert_eq!(payload["data"]["cookie_name"], "flowbase_console_session");
    assert_eq!(
        payload["data"]["actor"]["current_workspace_id"],
        payload["data"]["session"]["current_workspace_id"]
    );
}

#[tokio::test]
async fn delete_session_route_clears_current_session() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/session")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let session_response = app
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

    assert_eq!(session_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoke_all_route_invalidates_current_session() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/revoke-all")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let session_response = app
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

    assert_eq!(session_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn switch_workspace_route_requires_csrf() {
    let (app, database_url) = test_app_with_database_url().await;
    let target_workspace_id = seed_workspace(&database_url, "Workspace Without Csrf").await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-workspace")
                .header("cookie", cookie)
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

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn switch_role_replaces_the_session_authorization_scope() {
    let (app, database_url) = test_app_with_database_url().await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let user_id: Uuid = sqlx::query_scalar("select id from users where account = 'root'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let workspace_id: Uuid = sqlx::query_scalar(
        "select workspace_id from workspace_memberships where user_id = $1 order by created_at asc limit 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let manager_role_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction,
            is_builtin, is_editable, created_by, updated_by
        ) values ($1, $2, 'workspace', $2, 'manager', 'Manager', '', false, true, $3, $3)
        "#,
    )
    .bind(manager_role_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into user_role_bindings (id, user_id, role_id, scope_id, created_by, updated_by) values ($1, $2, $3, $4, $2, $2)",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(manager_role_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .unwrap();

    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let invalid_switch = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-role")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "role_code": "auditor" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_switch.status(), StatusCode::FORBIDDEN);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/console-policy-catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let body = to_bytes(catalog_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let catalog: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let revision = catalog["data"]["settings_order_revision"].as_i64().unwrap();
    let mut group_ids = catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|group| group["kind"] == "settings_feature")
        .map(|group| group["group_id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    group_ids.reverse();

    let reordered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/console-policy-catalog/order")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "expected_revision": revision,
                        "group_ids": group_ids
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reordered.status(), StatusCode::OK);
    let body = to_bytes(reordered.into_body(), usize::MAX).await.unwrap();
    let reordered_catalog: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let reordered_group_ids = reordered_catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|group| group["kind"] == "settings_feature")
        .map(|group| group["group_id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(reordered_group_ids, group_ids);

    let navigation = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/navigation")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(navigation.status(), StatusCode::OK);
    let body = to_bytes(navigation.into_body(), usize::MAX).await.unwrap();
    let navigation: serde_json::Value = serde_json::from_slice(&body).unwrap();
    for (group_id, item_id) in [
        ("system.roles", "settings.roles"),
        ("system.members", "settings.members"),
    ] {
        let expected_order = group_ids
            .iter()
            .position(|candidate| candidate == group_id)
            .unwrap() as i64;
        let actual_order = navigation["data"]["navigation_items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["item_id"] == item_id)
            .unwrap()["order"]
            .as_i64()
            .unwrap();
        assert_eq!(actual_order, expected_order);
    }
    let order_audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where actor_user_id = $1 and event_code = 'workspace.console_settings_order_replaced'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(order_audit_count, 1);

    let stale_reorder = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/console-policy-catalog/order")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "expected_revision": revision,
                        "group_ids": group_ids
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale_reorder.status(), StatusCode::CONFLICT);

    let incomplete_reorder = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/console-policy-catalog/order")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "expected_revision": revision + 1,
                        "group_ids": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(incomplete_reorder.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-role")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "role_code": "manager" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["data"]["actor"]["effective_display_role"],
        "manager"
    );
    assert_eq!(payload["data"]["session"]["active_role_code"], "manager");
    let switch_audit_count: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where actor_user_id = $1 and event_code = 'session.switch_active_role'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(switch_audit_count, 1);

    let protected = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(protected.status(), StatusCode::FORBIDDEN);

    let unauthorized_reorder = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/console-policy-catalog/order")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "expected_revision": revision + 1,
                        "group_ids": group_ids
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized_reorder.status(), StatusCode::FORBIDDEN);

    let frontstage_authoring = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/frontstage/{workspace_id}/pages/groups"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Manager must not inherit root authoring",
                        "placement": "sidebar"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(frontstage_authoring.status(), StatusCode::FORBIDDEN);

    let me_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_response.status(), StatusCode::OK);
    let body = to_bytes(me_response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["data"]["effective_display_role"], "manager");
    assert_eq!(payload["data"]["permissions"], json!([]));

    sqlx::query("delete from user_role_bindings where user_id = $1 and role_id = $2")
        .bind(user_id)
        .bind(manager_role_id)
        .execute(&pool)
        .await
        .unwrap();
    let revoked_role_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/me")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_role_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_memory_session_is_rejected_by_require_session() {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = app_with_state_and_config(state.clone(), &config);
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let session_id = cookie
        .split(';')
        .next()
        .and_then(|pair| pair.split_once('='))
        .map(|(_, value)| value.to_string())
        .unwrap();
    let user = state
        .store
        .find_user_for_password_login(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID, "root")
        .await
        .unwrap()
        .unwrap();
    let scope = state.store.default_scope_for_user(user.id).await.unwrap();

    seed_session(
        &state,
        SessionRecord {
            session_id: session_id.clone(),
            user_id: user.id,
            tenant_id: scope.tenant_id,
            current_workspace_id: scope.workspace_id,
            active_role_code: "root".into(),
            session_version: user.session_version,
            csrf_token: "expired-csrf".into(),
            expires_at_unix: OffsetDateTime::now_utc().unix_timestamp() - 1,
        },
    )
    .await;

    let response = app
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

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(state
        .session_store
        .get(&session_id)
        .await
        .unwrap()
        .is_none());
}
