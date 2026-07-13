use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn audit_event_count(database_url: &str, event_code: &str) -> i64 {
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .unwrap()
        .store;
    sqlx::query_scalar("select count(*) from audit_logs where event_code = $1")
        .bind(event_code)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn host_infrastructure_cache_routes_reveal_and_clear_with_audit() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let cache = state.infrastructure.cache_store();
    cache
        .set_json(
            "application-logs:run:1",
            json!({ "flow_run": { "status": "succeeded" } }),
            Some(time::Duration::seconds(60)),
        )
        .await
        .unwrap();
    cache
        .set_json("application-logs:run:2", json!({ "flow_run": 2 }), None)
        .await
        .unwrap();
    cache
        .set_json("runtime-records:row:1", json!({ "name": "Ada" }), None)
        .await
        .unwrap();

    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let overview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/cache")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overview_response.status(), StatusCode::OK);
    let overview_payload = response_json(overview_response).await;
    assert_eq!(overview_payload["data"]["provider_code"], "local");
    assert_eq!(overview_payload["data"]["can_manage"], true);
    assert_eq!(
        overview_payload["data"]["capabilities"]["reveal_value"],
        true
    );
    assert!(overview_payload["data"]["domains"]
        .as_array()
        .unwrap()
        .iter()
        .any(|domain| domain["domain_code"] == "application-logs"));

    let entries_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/entries")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(entries_response.status(), StatusCode::OK);
    let entries_payload = response_json(entries_response).await;
    assert_eq!(
        entries_payload["data"]["entries"][0]["domain_code"],
        "application-logs"
    );

    let reveal_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/entries/reveal")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "key": "application-logs:run:1" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reveal_response.status(), StatusCode::OK);
    let reveal_payload = response_json(reveal_response).await;
    assert_eq!(
        reveal_payload["data"]["value"],
        json!({ "flow_run": { "status": "succeeded" } })
    );
    assert_eq!(
        audit_event_count(&database_url, "host_infrastructure.cache_value_revealed").await,
        1
    );

    let clear_entry_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/entries/clear")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "key": "application-logs:run:1" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_entry_response.status(), StatusCode::OK);
    let clear_entry_payload = response_json(clear_entry_response).await;
    assert_eq!(clear_entry_payload["data"]["cleared"], true);
    assert_eq!(
        audit_event_count(&database_url, "host_infrastructure.cache_entry_cleared").await,
        1
    );

    let clear_domain_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/clear")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_domain_response.status(), StatusCode::OK);
    let clear_domain_payload = response_json(clear_domain_response).await;
    assert_eq!(clear_domain_payload["data"]["cleared_count"], 1);
    assert_eq!(
        audit_event_count(&database_url, "host_infrastructure.cache_domain_cleared").await,
        1
    );

    assert_eq!(
        cache
            .list_cache_entries("runtime-records")
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn host_infrastructure_cache_feature_redacts_lists_and_allows_explicit_reveal_and_clear() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    state
        .infrastructure
        .cache_store()
        .set_json("application-logs:run:1", json!({ "secret": true }), None)
        .await
        .unwrap();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "cache_settings_feature").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "cache_settings_feature",
        &["settings_feature.access.system.host-infrastructure"],
    )
    .await;
    let member_id =
        create_member(&app, &root_cookie, &root_csrf, "cache-viewer", "change-me").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["cache_settings_feature"],
    )
    .await;
    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "cache-viewer", "change-me").await;

    let overview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/cache")
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overview_response.status(), StatusCode::OK);
    let overview_payload = response_json(overview_response).await;
    assert_eq!(overview_payload["data"]["can_manage"], true);

    let entries_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/entries")
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(entries_response.status(), StatusCode::OK);
    let entries_payload = response_json(entries_response).await;
    assert!(entries_payload["data"]["entries"][0]
        .as_object()
        .unwrap()
        .get("value")
        .is_none());

    let reveal_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/entries/reveal")
                .header("cookie", &viewer_cookie)
                .header("x-csrf-token", &viewer_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "key": "application-logs:run:1" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reveal_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(reveal_response).await["data"]["value"],
        json!({ "secret": true })
    );

    let clear_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/cache/domains/application-logs/clear")
                .header("cookie", &viewer_cookie)
                .header("x-csrf-token", &viewer_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clear_response.status(), StatusCode::OK);
}
