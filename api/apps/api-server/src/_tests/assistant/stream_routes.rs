use crate::_tests::support::{login_and_capture_cookie, read_first_sse_frame, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_published_agent_flow(app: &Router, cookie: &str, csrf: &str) -> String {
    let created = app
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
                        "name": "Embedded Assistant Stream",
                        "description": "assistant stream route fixture",
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
    assert_eq!(created.status(), StatusCode::CREATED);
    let application_id = response_json(created).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let publication = app
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
                                "model_target": "node-start.model",
                                "inputs_target": "node-start",
                                "history_target": "node-start.history",
                                "attachments_target": "node-start.files"
                            },
                            "output": {
                                "answer_selector": "answer",
                                "usage_selector": "usage",
                                "files_selector": null,
                                "error_selector": "error"
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
    assert_eq!(publication.status(), StatusCode::CREATED);

    application_id
}

#[tokio::test]
async fn assistant_stream_starts_a_published_session_run_and_emits_flow_accepted() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_published_agent_flow(&app, &cookie, &csrf).await;

    let settings = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/assistant/settings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_id": application_id,
                        "mcp_instance_ids": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(settings.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/assistant/runs/stream")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "query": "请介绍退款政策",
                        "history": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let frame = read_first_sse_frame(response).await;
    assert!(frame.contains("event: flow_accepted"), "{frame}");
    assert!(frame.contains("\"type\":\"flow_accepted\""), "{frame}");

    let envelope: Value = serde_json::from_str(
        frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("flow_accepted SSE frame has data"),
    )
    .unwrap();
    let run_id = envelope["run_id"].as_str().unwrap();
    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    assert_eq!(
        response_json(snapshot).await["data"]["flow_run"]["run_mode"],
        "assistant_execution"
    );
}

#[tokio::test]
async fn assistant_settings_persist_model_override_for_the_current_session_user() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_published_agent_flow(&app, &cookie, &csrf).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/assistant/settings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_id": application_id,
                        "mcp_instance_ids": [],
                        "model": "1flowbase"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response_json(response).await["data"]["preference"]["model"].is_null());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/assistant/settings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_id": application_id,
                        "mcp_instance_ids": [],
                        "model": "1flowbase"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await["data"]["preference"]["model"],
        "1flowbase"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/assistant/runs/stream")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "query": "hello", "history": [] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let frame = read_first_sse_frame(response).await;
    let run_id = serde_json::from_str::<Value>(
        frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("flow_accepted SSE frame has data"),
    )
    .unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response_json(snapshot).await["data"]["flow_run"]["input_payload"]["node-start"]["model"],
        "1flowbase"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/assistant/settings")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_id": null,
                        "mcp_instance_ids": [],
                        "model": "1flowbase"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response_json(response).await["data"]["preference"]["model"].is_null());
}
