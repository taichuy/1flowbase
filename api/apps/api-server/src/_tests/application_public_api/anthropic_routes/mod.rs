use crate::{
    _tests::support::{login_and_capture_cookie, test_api_state_with_database_url, test_config},
    app_state::ApiState,
    routes::application_public_api::tool_callback_ids::encode_anthropic_callback_tool_use_id,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::ports::{
    CreateCallbackTaskInput, CreateNodeRunInput, OrchestrationRuntimeRepository, UpdateFlowRunInput,
};
use serde_json::{json, Value};
use std::sync::Arc;
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

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
    let start_node = document["graph"]["nodes"]
        .as_array_mut()
        .expect("nodes array")
        .iter_mut()
        .find(|node| node["type"] == "start")
        .expect("default draft should include a start node");
    start_node["config"]["model_list"] = json!([
        {
            "id": "qwen3.6-35b-a3b",
            "name": "Qwen 3.6 35B",
            "context_window": 128000,
            "max_output_tokens": 32000,
            "auto_compact_token_limit": 110000,
            "capabilities": {
                "reasoning": true,
                "tool_call": true,
                "multimodal": false,
                "structured_output": true
            },
            "reasoning": {
                "default_effort": "medium",
                "supported_efforts": ["low", "medium", "high"]
            }
        },
        "deepseek-v4-flash"
    ]);

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

    assert_eq!(response.status(), StatusCode::CREATED);
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

async fn seed_pending_anthropic_llm_callback(
    state: &ApiState,
    flow_run_id: Uuid,
    tool_use_id: &str,
) -> domain::CallbackTaskRecord {
    seed_pending_anthropic_llm_callback_with_tools(state, flow_run_id, &[tool_use_id]).await
}

async fn seed_pending_anthropic_llm_callback_with_tools(
    state: &ApiState,
    flow_run_id: Uuid,
    tool_use_ids: &[&str],
) -> domain::CallbackTaskRecord {
    let tool_calls = tool_use_ids
        .iter()
        .map(|tool_use_id| {
            json!({
                "id": tool_use_id,
                "name": "Grep",
                "arguments": { "pattern": "image-1.png" }
            })
        })
        .collect::<Vec<_>>();
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id,
            status: domain::FlowRunStatus::WaitingCallback,
            output_payload: json!({ "tool_calls": tool_calls.clone() }),
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    let node_run = state
        .store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: json!({}),
            debug_payload: json!({ "llm_rounds": [] }),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();

    state
        .store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id,
            node_run_id: node_run.id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({
                "tool_calls": tool_calls,
                "finish_reason": "tool_call"
            }),
            external_ref_payload: None,
        })
        .await
        .unwrap()
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
        "model": "anthropic/custom-model:latest",
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
