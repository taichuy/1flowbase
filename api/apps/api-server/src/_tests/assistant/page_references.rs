use super::stream_routes::{
    create_published_agent_flow, response_json, select_assistant_application,
};
use crate::_tests::support::{login_and_capture_cookie, read_first_sse_frame, test_app};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn assistant_page_reference_keeps_display_content_separate_from_model_context() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_published_agent_flow(&app, &cookie, &csrf).await;
    select_assistant_application(&app, &cookie, &csrf, &application_id).await;

    let settings = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/assistant/settings")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(settings.status(), StatusCode::OK);
    let settings = response_json(settings).await;
    assert_eq!(settings["data"]["page_reference_max_bytes"], 65_536);
    assert_eq!(settings["data"]["page_reference_max_count"], 5);
    assert_eq!(settings["data"]["page_reference_max_total_bytes"], 65_536);

    let first_page_reference = json!({
        "page_url": "http://console.test/applications/app-1/logs",
        "page_title": "运行日志",
        "outer_html": "<span id=\"selected-run\">退款失败</span>"
    });
    let second_page_reference = json!({
        "page_url": "http://console.test/applications/app-1/logs",
        "page_title": "运行日志",
        "outer_html": "<button id=\"retry-run\">重试</button>"
    });
    let fifth_page_reference = json!({
        "page_url": "http://console.test/applications/app-1/logs",
        "page_title": "运行日志",
        "outer_html": "<strong id=\"run-owner\">客服 A</strong>"
    });
    let page_references = vec![
        first_page_reference.clone(),
        second_page_reference.clone(),
        json!({
            "page_url": "http://console.test/applications/app-1/logs",
            "page_title": "运行日志",
            "outer_html": "<span id=\"run-status\">失败</span>"
        }),
        json!({
            "page_url": "http://console.test/applications/app-1/logs",
            "page_title": "运行日志",
            "outer_html": "<time id=\"run-time\">12:00</time>"
        }),
        fifth_page_reference.clone(),
    ];
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
                        "application_id": application_id,
                        "query": "为什么这条运行失败？",
                        "history": [],
                        "page_references": page_references
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let frame = read_first_sse_frame(response).await;
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
    let snapshot = response_json(snapshot).await;
    let input_payload = &snapshot["data"]["flow_run"]["input_payload"];
    assert_eq!(
        input_payload["__embedded_assistant_user_message"]["content"],
        "为什么这条运行失败？"
    );
    assert_eq!(
        input_payload["__embedded_assistant_user_message"]["page_references"][0],
        first_page_reference
    );
    assert_eq!(
        input_payload["__embedded_assistant_user_message"]["page_references"][1],
        second_page_reference
    );
    assert_eq!(
        input_payload["__embedded_assistant_user_message"]["page_references"][4],
        fifth_page_reference
    );
    let model_query = input_payload["node-start"]["query"].as_str().unwrap();
    assert!(model_query.starts_with("为什么这条运行失败？"));
    assert!(model_query.contains("untrusted"));
    assert!(model_query.contains("<span id=\\\"selected-run\\\">"));
    assert!(model_query.contains("<button id=\\\"retry-run\\\">"));
    assert!(model_query.contains("<strong id=\\\"run-owner\\\">"));
}

#[tokio::test]
async fn assistant_page_references_reject_count_and_total_byte_overflow() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_published_agent_flow(&app, &cookie, &csrf).await;
    select_assistant_application(&app, &cookie, &csrf, &application_id).await;

    let six_references = (0..6)
        .map(|index| {
            json!({
                "page_url": "http://console.test/",
                "page_title": "首页",
                "outer_html": format!("<span>{index}</span>")
            })
        })
        .collect::<Vec<_>>();
    let count_response = app
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
                        "application_id": application_id,
                        "query": "检查页面",
                        "history": [],
                        "page_references": six_references
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(count_response.status(), StatusCode::BAD_REQUEST);

    let total_response = app
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
                        "application_id": application_id,
                        "query": "检查页面",
                        "history": [],
                        "page_references": [
                            {
                                "page_url": "http://console.test/",
                                "page_title": "首页",
                                "outer_html": format!("<div>{}</div>", "a".repeat(32_760))
                            },
                            {
                                "page_url": "http://console.test/",
                                "page_title": "首页",
                                "outer_html": format!("<div>{}</div>", "b".repeat(32_760))
                            }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(total_response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn assistant_page_reference_rejects_oversized_html_without_truncating_it() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_published_agent_flow(&app, &cookie, &csrf).await;
    select_assistant_application(&app, &cookie, &csrf, &application_id).await;
    let outer_html = format!("<div>{}</div>", "x".repeat(65_536));

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
                        "application_id": application_id,
                        "query": "检查页面",
                        "history": [],
                        "page_references": [{
                            "page_url": "http://console.test/",
                            "page_title": "首页",
                            "outer_html": outer_html
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
