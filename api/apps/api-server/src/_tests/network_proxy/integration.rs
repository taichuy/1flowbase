use super::*;
use crate::_tests::support::{
    create_member, create_role, get_json, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::ports::ProviderRuntimePort;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

#[tokio::test]
async fn network_proxy_authoring_api_permissions_and_selected_socks_execution() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (port, server) = super::protocol::socks_proxy(true, false, None).await;
    let (status, created) = request(&app, "POST", "/api/console/network-center/pools/proxies", &cookie, &csrf, json!({
        "provider_code":"builtin_static_socks5", "display_name":"SOCKS test", "config":{
            "host":"127.0.0.1", "port":port.to_string(), "username":"alice", "password":"proxy-secret"
        }
    })).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let provider_id = created["data"]["id"].as_str().unwrap();
    assert!(!created.to_string().contains("proxy-secret"));
    let detail = get_json(
        &app,
        &format!("/api/console/network-center/pools/proxies/{provider_id}"),
        &cookie,
    )
    .await;
    assert!(detail["data"]["config"].get("password").is_none());
    assert_eq!(
        detail["data"]["configured_secret_fields"],
        json!(["password"])
    );

    let path = "/api/console/network-center/http-node-proxy-options";
    let (unauthenticated, _) = request(&app, "GET", path, "", "", Value::Null).await;
    assert_eq!(unauthenticated, StatusCode::UNAUTHORIZED);
    let member_id = create_member(&app, &cookie, &csrf, "proxy-author", "password-123").await;
    create_role(&app, &cookie, &csrf, "proxy-author").await;
    replace_member_roles(&app, &cookie, &csrf, &member_id, &["proxy-author"]).await;
    let (author_cookie, _) = login_and_capture_cookie(&app, "proxy-author", "password-123").await;
    let (denied, _) = request(&app, "GET", path, &author_cookie, "", Value::Null).await;
    assert_eq!(denied, StatusCode::FORBIDDEN);
    let (policy_status, policy) = request(&app, "PUT", "/api/console/settings/roles/proxy-author/console-policy", &cookie, &csrf,
        json!({"groups":[{"kind":"other","group_id":"other.agent-flow","enabled":true,"strategy":"custom","operations":[{"kind":"simple","operation_id":"agent_flow.http_node_proxy_options.list","enabled":true}]}]})).await;
    assert_eq!(policy_status, StatusCode::NO_CONTENT, "{policy}");
    let (allowed, options) = request(&app, "GET", path, &author_cookie, "", Value::Null).await;
    assert_eq!(allowed, StatusCode::OK, "{options}");
    let choice = &options["data"][0];
    assert_eq!(choice["display_name"], "SOCKS test");
    assert_eq!(choice["selectable"], true);
    assert_eq!(
        choice.as_object().unwrap().len(),
        3,
        "options must not expose credentials or host addresses"
    );
    let (secret_denied, _) = request(
        &app,
        "GET",
        &format!("/api/console/network-center/pools/proxies/{provider_id}"),
        &author_cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(secret_denied, StatusCode::FORBIDDEN);

    let pool_member_id = Uuid::parse_str(choice["pool_member_id"].as_str().unwrap()).unwrap();
    let resolver = NetworkEgressHttpClientResolver::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.provider_secret_master_key.clone(),
        state.api_node_id.clone(),
    );
    let runtime = ApiProviderRuntime::new(state.provider_runtime.clone())
        .with_network_egress(Arc::new(resolver));
    let lease = runtime
        .acquire_http_node_client(
            &domain::HttpNodeNetworkEgress::Proxy { pool_member_id },
            state.bootstrap_workspace_id,
            Duration::from_secs(5),
            true,
        )
        .await
        .unwrap()
        .unwrap();
    let result = super::protocol::run_with_lease(lease, "http://proxy-origin.invalid/").await;
    assert!(result.error_payload.is_none(), "{:?}", result.error_payload);
    assert_eq!(result.output_payload["body"], "proxied");
    server.await.unwrap();

    // Disabled and deleted references must fail before the client can reach an origin.
    sqlx::query("update network_egress_pool_members set enabled=false where id=$1")
        .bind(pool_member_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let options = get_json(&app, path, &author_cookie).await;
    assert_eq!(options["data"][0]["selectable"], false);
    for id in [pool_member_id, Uuid::now_v7()] {
        assert!(runtime
            .acquire_http_node_client(
                &domain::HttpNodeNetworkEgress::Proxy { pool_member_id: id },
                state.bootstrap_workspace_id,
                Duration::from_secs(5),
                true
            )
            .await
            .is_err());
    }
}
