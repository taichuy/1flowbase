use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use control_plane::ports::{
    RuntimeEventDurability, RuntimeEventPayload, RuntimeEventSource, RuntimeEventStreamPolicy,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

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

async fn latest_audit_payload(database_url: &str, event_code: &str) -> Value {
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .unwrap()
        .store;
    sqlx::query_scalar(
        "select payload from audit_logs where event_code = $1 order by created_at desc limit 1",
    )
    .bind(event_code)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

fn contract_summary<'a>(payload: &'a Value, contract_code: &str) -> &'a Value {
    payload["data"]["contracts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|contract| contract["contract_code"] == contract_code)
        .unwrap_or_else(|| panic!("missing memory contract summary for {contract_code}"))
}

#[tokio::test]
async fn host_infrastructure_memory_routes_list_categories_and_reveal_with_audit() {
    let (state, database_url) = test_api_state_with_database_url().await;
    state
        .infrastructure
        .cache_store()
        .set_json(
            "application-logs:run:1",
            json!({ "flow_run": { "status": "succeeded" } }),
            Some(time::Duration::seconds(60)),
        )
        .await
        .unwrap();
    state
        .infrastructure
        .rate_limit_store()
        .consume("login:root", 5, time::Duration::minutes(1))
        .await
        .unwrap();
    state
        .infrastructure
        .distributed_lock()
        .acquire("workflow:compile", "worker-1", time::Duration::minutes(5))
        .await
        .unwrap();
    state
        .infrastructure
        .task_queue()
        .enqueue("runtime", json!({ "job": "sync" }), Some("sync-1"))
        .await
        .unwrap();
    state
        .infrastructure
        .event_bus()
        .publish("runtime-events", json!({ "kind": "queued" }))
        .await
        .unwrap();
    let run_id = Uuid::now_v7();
    let runtime_event_stream = state.infrastructure.runtime_event_stream().unwrap();
    runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    runtime_event_stream
        .append(
            run_id,
            RuntimeEventPayload {
                event_type: "text_delta".to_string(),
                source: RuntimeEventSource::Runtime,
                durability: RuntimeEventDurability::Ephemeral,
                persist_required: false,
                trace_visible: true,
                payload: json!({ "delta": "hello", "delta_index": 1 }),
            },
        )
        .await
        .unwrap();

    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let overview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overview_response.status(), StatusCode::OK);
    let overview_payload = response_json(overview_response).await;
    assert_eq!(overview_payload["data"]["can_manage"], true);
    for contract_code in [
        "session-store",
        "cache-store",
        "rate-limit-store",
        "distributed-lock",
        "task-queue",
        "event-bus",
        "runtime-event-stream",
    ] {
        let summary = contract_summary(&overview_payload, contract_code);
        assert_eq!(summary["provider_code"], "local");
        assert_eq!(summary["supported"], true);
        assert!(summary.as_object().unwrap().get("entry_count").is_none());
    }

    let stats_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/session-store/stats")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats_response.status(), StatusCode::OK);
    let stats_payload = response_json(stats_response).await;
    assert_eq!(stats_payload["data"]["contract_code"], "session-store");
    assert_eq!(stats_payload["data"]["provider_code"], "local");
    assert_eq!(stats_payload["data"]["inspection_path"], json!([]));
    assert!(stats_payload["data"]["entry_count"].as_u64().unwrap() >= 1);
    assert!(
        stats_payload["data"]["sensitive_entry_count"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        stats_payload["data"]["total_value_size_bytes"]
            .as_u64()
            .unwrap()
            >= 1
    );

    let stats_overview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/stats")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stats_overview_response.status(), StatusCode::OK);
    let stats_overview_payload = response_json(stats_overview_response).await;
    assert_eq!(stats_overview_payload["data"]["inspection_path"], json!([]));
    assert!(
        stats_overview_payload["data"]["entry_count"]
            .as_u64()
            .unwrap()
            >= stats_payload["data"]["entry_count"].as_u64().unwrap()
    );
    assert!(
        stats_overview_payload["data"]["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|contract| contract["contract_code"] == "session-store")
    );
    assert!(
        stats_overview_payload["data"]["contracts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|contract| contract["inspection_path"] == json!([]))
    );

    let entries_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/session-store/entries")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(entries_response.status(), StatusCode::OK);
    let entries_payload = response_json(entries_response).await;
    let first_session = &entries_payload["data"]["entries"][0];
    assert_eq!(first_session["contract_code"], "session-store");
    assert_eq!(first_session["sensitive"], true);
    assert!(first_session.as_object().unwrap().get("value").is_none());
    let session_key = first_session["key"].as_str().unwrap();
    let session_entry_ref = first_session["entry_ref"].as_str().unwrap();

    let reveal_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/session-store/entries/reveal")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "entry_ref": session_entry_ref, "reveal_mode": "full" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reveal_response.status(), StatusCode::OK);
    let reveal_payload = response_json(reveal_response).await;
    assert_eq!(reveal_payload["data"]["value"]["session_id"], session_key);
    assert_eq!(
        audit_event_count(&database_url, "host_infrastructure.memory_value_revealed").await,
        1
    );
    let audit_payload =
        latest_audit_payload(&database_url, "host_infrastructure.memory_value_revealed").await;
    assert_eq!(audit_payload["contract_code"], "session-store");
    assert_eq!(audit_payload["key"], session_key);
    assert!(audit_payload.as_object().unwrap().get("value").is_none());
}

#[tokio::test]
async fn host_infrastructure_memory_routes_page_tree_search_and_reveal_policy() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    let large_value = "x".repeat(300 * 1024);
    for index in 0..3 {
        state
            .infrastructure
            .cache_store()
            .set_json(
                &format!("application-logs:run:{index}"),
                json!({ "index": index, "blob": large_value }),
                Some(time::Duration::seconds(60)),
            )
            .await
            .unwrap();
    }
    state
        .infrastructure
        .cache_store()
        .set_json(
            "runtime-records:row:0",
            json!({ "index": 0 }),
            Some(time::Duration::seconds(60)),
        )
        .await
        .unwrap();

    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let tree_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/tree?limit=1&byte_limit=512")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tree_response.status(), StatusCode::OK);
    let tree_payload = response_json(tree_response).await;
    assert_eq!(
        tree_payload["data"]["nodes"][0]["inspection_path"][0],
        "application-logs"
    );
    assert!(tree_payload["data"]["next_cursor"].as_str().is_some());
    assert!(tree_payload["data"]["emitted_bytes"].as_u64().unwrap() <= 512);

    let entries_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries?path=application-logs&limit=1&byte_limit=1024")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(entries_response.status(), StatusCode::OK);
    let entries_payload = response_json(entries_response).await;
    let first_entry = &entries_payload["data"]["entries"][0];
    assert_eq!(first_entry["contract_code"], "cache-store");
    assert_eq!(first_entry["inspection_path"][0], "application-logs");
    assert!(
        first_entry["entry_ref"]
            .as_str()
            .unwrap()
            .starts_with("application-logs:run:")
    );
    assert!(first_entry["metadata_size_bytes"].as_u64().unwrap() > 0);
    assert!(first_entry.as_object().unwrap().get("value").is_none());
    assert!(entries_payload["data"]["next_cursor"].as_str().is_some());
    assert!(entries_payload["data"]["emitted_bytes"].as_u64().unwrap() <= 1024);

    let search_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/search?q=run:2&limit=10")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search_response.status(), StatusCode::OK);
    let search_payload = response_json(search_response).await;
    assert_eq!(
        search_payload["data"]["entries"].as_array().unwrap().len(),
        1
    );
    assert!(
        search_payload["data"]["entries"][0]["entry_ref"]
            .as_str()
            .unwrap()
            .ends_with("run:2")
    );

    let entry_ref = first_entry["entry_ref"].as_str().unwrap();
    let preview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/reveal")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "entry_ref": entry_ref, "reveal_mode": "preview" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview_payload = response_json(preview_response).await;
    assert_eq!(preview_payload["data"]["reveal_mode"], "preview");
    assert_eq!(preview_payload["data"]["value_state"], "preview");
    assert!(preview_payload["data"]["value"].is_null());
    assert!(
        preview_payload["data"]["value_preview"]
            .as_str()
            .unwrap()
            .len()
            <= 8192
    );

    let full_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/reveal")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "entry_ref": entry_ref, "reveal_mode": "full" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(full_response.status(), StatusCode::OK);
    let full_payload = response_json(full_response).await;
    assert_eq!(full_payload["data"]["reveal_mode"], "full");
    assert_eq!(full_payload["data"]["value_state"], "value_too_large");
    assert!(full_payload["data"]["value"].is_null());
}

#[tokio::test]
async fn memory_observation_feature_redacts_lists_until_explicit_reveal() {
    let (state, _database_url) = test_api_state_with_database_url().await;
    state
        .infrastructure
        .cache_store()
        .set_json("application-logs:run:1", json!({ "secret": true }), None)
        .await
        .unwrap();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "memory_settings_feature").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "memory_settings_feature",
        &["settings_feature.access.system.memory-observation"],
    )
    .await;
    let member_id =
        create_member(&app, &root_cookie, &root_csrf, "memory-viewer", "change-me").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["memory_settings_feature"],
    )
    .await;
    let (viewer_cookie, viewer_csrf) =
        login_and_capture_cookie(&app, "memory-viewer", "change-me").await;

    let overview_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory")
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
                .method("GET")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries")
                .header("cookie", &viewer_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(entries_response.status(), StatusCode::OK);
    let entries_payload = response_json(entries_response).await;
    let cache_entry_ref = entries_payload["data"]["entries"][0]["entry_ref"]
        .as_str()
        .unwrap();
    assert!(
        entries_payload["data"]["entries"][0]
            .as_object()
            .unwrap()
            .get("value")
            .is_none()
    );

    let reveal_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/host-infrastructure/memory/contracts/cache-store/entries/reveal")
                .header("cookie", &viewer_cookie)
                .header("x-csrf-token", &viewer_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "entry_ref": cache_entry_ref, "reveal_mode": "full" }).to_string(),
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
}
