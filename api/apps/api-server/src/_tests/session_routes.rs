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
                .uri("/api/public/auth/providers/password-local/sign-in")
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
        .find_user_for_password_login(domain::PASSWORD_LOCAL_AUTHENTICATOR_NAME, "root")
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
