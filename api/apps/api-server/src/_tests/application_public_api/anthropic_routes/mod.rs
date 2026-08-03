use crate::{
    _tests::{
        create_ready_provider_instance,
        support::{login_and_capture_cookie, test_api_state_with_database_url, test_config},
    },
    app_state::ApiState,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const ANTHROPIC_FIXTURE_MODEL: &str = "fixture_chat";

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
                        "description": "anthropic compatible route test",
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
                        "name": "Anthropic compatible route key",
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

async fn publish_application(app: &Router, cookie: &str, csrf: &str, application_id: &str) {
    let provider_instance_id = create_ready_provider_instance(app, cookie, csrf).await;
    let state = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(state.status(), StatusCode::OK);
    let mut document = response_json(state).await["data"]["draft"]["document"].clone();
    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("nodes array");
    {
        let start_node = nodes
            .iter_mut()
            .find(|node| node["type"] == "start")
            .expect("default draft should include a start node");
        start_node["config"]["model_list"] = json!([ANTHROPIC_FIXTURE_MODEL]);
    }
    let llm_node = nodes
        .iter_mut()
        .find(|node| node["type"] == "llm")
        .expect("default draft should include an LLM node");
    llm_node["config"]["model_provider"] = json!({
        "provider_code": "fixture_provider",
        "source_instance_id": provider_instance_id,
        "model_id": ANTHROPIC_FIXTURE_MODEL
    });

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/draft"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "document": document,
                        "change_kind": "logical",
                        "summary": "Configure anthropic compatible model list"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::OK);

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
                                "history_target": "node-start.history",
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

    let status = response.status();
    if status != StatusCode::CREATED {
        let payload = response_json(response).await;
        panic!("expected publication creation, got {status}: {payload}");
    }
}

async fn setup_published_app(app: &Router, name: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    publish_application(app, &cookie, &csrf, &application_id).await;
    token
}

async fn setup_unpublished_app_key(app: &Router, name: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    create_application_key(app, &cookie, &csrf, &application_id).await
}

async fn test_app_with_state() -> (Router, Arc<ApiState>) {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn flow_run_count(state: &ApiState) -> i64 {
    sqlx::query_scalar("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap()
}

async fn assert_published_anthropic_plan_has_provider_route(state: &ApiState) {
    let plan: Value = sqlx::query_scalar(
        "select plan from flow_compiled_plans order by created_at desc, id desc limit 1",
    )
    .fetch_one(state.store.pool())
    .await
    .unwrap();

    assert_eq!(plan["compile_issues"], json!([]), "{plan}");
    let runtime = &plan["nodes"]["node-llm"]["llm_runtime"];
    assert_eq!(
        runtime["provider_code"],
        json!("fixture_provider"),
        "{plan}"
    );
    assert_eq!(runtime["model"], json!(ANTHROPIC_FIXTURE_MODEL), "{plan}");
    assert!(
        runtime["provider_instance_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "{plan}"
    );
}

async fn post_json(
    app: &Router,
    uri: &str,
    token_header: (&str, String),
    body: Value,
) -> axum::response::Response {
    post_json_with_headers(app, uri, token_header, Vec::new(), body).await
}

async fn post_json_with_headers(
    app: &Router,
    uri: &str,
    token_header: (&str, String),
    extra_headers: Vec<(&str, String)>,
    body: Value,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(token_header.0, token_header.1)
        .header("content-type", "application/json");
    for (name, value) in extra_headers {
        request = request.header(name, value);
    }

    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

fn anthropic_body() -> Value {
    json!({
        "model": ANTHROPIC_FIXTURE_MODEL,
        "max_tokens": 64,
        "messages": [
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ],
        "metadata": {
            "expand_id": "external-user-123"
        }
    })
}

fn anthropic_multimodal_body() -> Value {
    let mut body = anthropic_body();
    body["messages"] = json!([
        {
            "role": "user",
            "content": [
                {"type": "text", "text": "Describe this image"},
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "iVBORw0KGgo="
                    }
                }
            ]
        }
    ]);
    body
}

mod count_tokens;
mod message_creation;
mod probe_and_title;
mod tool_resume;
