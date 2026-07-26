use super::compat_routes::{
    test_app_with_runtime_event_stream, DropTerminalRuntimeEventStream,
    NeverCloseDropTerminalRuntimeEventStream,
};
use super::native_routes::{
    assert_published_native_generate_route, configure_runnable_native_generate_target,
};
use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_application(app: &Router, cookie: &str, csrf: &str, name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": name,
                        "description": "native public streaming test",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_application_key(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Native streaming key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn publish_native_application(app: &Router, cookie: &str, csrf: &str, application_id: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": {
                            "input": {
                                "query_target": "node-start.query",
                                "model_target": null,
                                "inputs_target": null,
                                "history_target": null,
                                "attachments_target": null
                            },
                            "output": {
                                "answer_selector": "answer",
                                "usage_selector": null,
                                "files_selector": null,
                                "error_selector": null
                            }
                        },
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = response_json(response).await;
    assert!(payload["data"].get("operation_bindings").is_none());
}

async fn test_app_with_state() -> (Router, std::sync::Arc<crate::app_state::ApiState>) {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn setup_published_native_app(
    app: &Router,
    state: &crate::app_state::ApiState,
    name: &str,
) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    configure_runnable_native_generate_target(app, &cookie, &csrf, &application_id).await;
    publish_native_application(app, &cookie, &csrf, &application_id).await;
    assert_published_native_generate_route(state, &application_id).await;
    token
}

async fn post_streaming_run(app: &Router, token: &str, stream_options: Value) -> (String, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/v1/runs")
                .header("authorization", format!("Bearer {token}"))
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "query": "Hello from public streaming",
                        "response_mode": "streaming",
                        "stream_options": stream_options,
                        "conversation": {
                            "id": "stream-conversation",
                            "user": "customer-1"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap()
        .to_string();
    let body = timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("native SSE should reach a terminal event")
    .unwrap();

    (content_type, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn native_streaming_create_returns_sse_started_and_terminal_events() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(&app, state.as_ref(), "Native Streaming App").await;

    let (content_type, body) = post_streaming_run(&app, &token, json!({})).await;

    assert_eq!(content_type, "text/event-stream");
    assert!(body.contains("event: run.started"), "{body}");
    assert!(
        body.contains("event: run.completed")
            || body.contains("event: run.failed")
            || body.contains("event: run.cancelled"),
        "{body}"
    );
}

#[tokio::test]
async fn native_streaming_default_hides_workflow_and_debug_internals() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Streaming Filter App").await;

    let (_, body) = post_streaming_run(&app, &token, json!({})).await;

    assert!(!body.contains("event: workflow.event"), "{body}");
    assert!(!body.contains("flow_started"), "{body}");
    assert!(!body.contains("node_started"), "{body}");
    assert!(!body.contains("debug_payload"), "{body}");
    assert!(!body.contains("node_run_id"), "{body}");
}

#[tokio::test]
async fn native_streaming_emits_terminal_fallback_after_runtime_stream_closes() {
    let (app, state) =
        test_app_with_runtime_event_stream(Arc::new(DropTerminalRuntimeEventStream::new())).await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Dropped Terminal Stream App")
            .await;

    let (_, body) = post_streaming_run(&app, &token, json!({})).await;

    assert!(
        body.contains("event: run.completed")
            || body.contains("event: run.failed")
            || body.contains("event: run.cancelled"),
        "{body}"
    );
}

#[tokio::test]
async fn native_streaming_does_not_poll_durable_terminal_while_runtime_stream_stays_open() {
    let (app, state) = test_app_with_runtime_event_stream(Arc::new(
        NeverCloseDropTerminalRuntimeEventStream::new(),
    ))
    .await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Stuck Runtime Stream App").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/v1/runs")
                .header("authorization", format!("Bearer {token}"))
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "query": "Keep the runtime stream open",
                        "response_mode": "streaming",
                        "stream_options": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = timeout(
        Duration::from_millis(900),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await;

    assert!(
        body.is_err(),
        "native SSE should wait for an ephemeral close signal instead of polling durable state"
    );
}
