use crate::_tests::support::{create_member, login_and_capture_cookie, test_app};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

/// AC-014: a valid console session without the registered SettingsFeature must be rejected by
/// the real route before it can observe the provider registry.
#[tokio::test]
async fn network_center_provider_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-center-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-center-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/providers")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-GP04: the proxy plugin catalog is a Network Center capability, not an extension-center
/// backdoor that happens to be rendered on the proxy types page.
#[tokio::test]
async fn network_center_proxy_plugin_catalog_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-plugins-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-plugins-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/proxy-plugins/official-catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-NCP02: version history is governed by the same Network Center feature scope as the
/// official catalog; it must not become a model-provider management backdoor.
#[tokio::test]
async fn network_center_proxy_plugin_families_reject_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-plugin-families-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-plugin-families-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/network-center/proxy-plugins/families")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-NCP03: removing a proxy plugin family is a Network Center action and must not be
/// reachable by a session that lacks the Network Center SettingsFeature.
#[tokio::test]
async fn network_center_proxy_plugin_family_uninstall_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-plugin-family-uninstall-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(
        &app,
        "network-plugin-family-uninstall-without-scope",
        "temp-pass",
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/network-center/proxy-plugins/families/clash-proxy")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-015: pool state is protected by the same backend-owned Network Center feature scope.
#[tokio::test]
async fn network_center_pool_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-pool-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-pool-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/network-center/pools")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-GP03: creating a proxy is a Network Center action and must not be exposed solely because
/// the route happens to live below the pool URL.
#[tokio::test]
async fn network_center_proxy_creation_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-proxy-create-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) =
        login_and_capture_cookie(&app, "network-proxy-create-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/network-center/pools/proxies")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"provider_code":"builtin_static_http","display_name":"Blocked","config":{"host":"198.65.36.212","port":"37867"}}"#,
                ))
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-OP05: connection tests remain a Network Center operation and never become a public proxy
/// endpoint merely because the browser has a pool-member id.
#[tokio::test]
async fn network_center_connection_test_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-probe-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, csrf) =
        login_and_capture_cookie(&app, "network-probe-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/network-center/pools/00000000-0000-0000-0000-000000000001/members/00000000-0000-0000-0000-000000000002/test-connection")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// AC-015: the route selector API is owned by the existing Network Center SettingsFeature too.
#[tokio::test]
async fn network_center_route_registry_rejects_session_without_feature_scope() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "network-route-without-scope",
        "temp-pass",
    )
    .await;
    let (cookie, _) =
        login_and_capture_cookie(&app, "network-route-without-scope", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/network-center/routes")
                .header("cookie", cookie)
                .body(Body::empty())
                .expect("request should be valid"),
        )
        .await
        .expect("router should return a response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
