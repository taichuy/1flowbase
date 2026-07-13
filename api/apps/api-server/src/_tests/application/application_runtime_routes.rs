use std::{fs, path::Path};

use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_app,
    test_app_with_database_url, test_config, write_provider_manifest_v2,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const DEBUG_SESSION_ID: &str = "application-runtime-debug-session";

fn create_provider_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
    fs::create_dir_all(root.join("demo")).unwrap();
    fs::create_dir_all(root.join("scripts")).unwrap();
    write_provider_manifest_v2(root, "fixture_provider", "Fixture Provider", "0.1.0");
    fs::write(
        root.join("provider/fixture_provider.yaml"),
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
parameter_form:
  schema_version: 1.0.0
  title: LLM Parameters
  fields:
    - key: temperature
      label: Temperature
      type: number
      send_mode: optional
      enabled_by_default: true
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
"#,
    )
    .unwrap();
    fs::write(
        root.join("bin/fixture_provider-provider"),
        r#"#!/usr/bin/env node
const fs = require('node:fs');

const request = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');

let result = {};
switch (request.method) {
  case 'validate':
    result = { sanitized: { api_key: request.input?.api_key ? "***" : null } };
    break;
  case 'list_models':
    result = [{
      model_id: "fixture_chat",
      display_name: "Fixture Chat",
      source: "dynamic",
      supports_streaming: true,
      supports_tool_call: false,
      supports_multimodal: false,
      provider_metadata: {}
    }];
    break;
  case 'invoke': {
    const query = request.input?.messages?.[0]?.content ?? "";
    const text = "reply:" + query;
    const lines = [
      ...Array.from(text.padEnd(70, "."), (delta) => ({ type: "text_delta", delta })),
      { type: "usage_snapshot", usage: { input_tokens: 5, output_tokens: 7, total_tokens: 12 } },
      { type: "finish", reason: "stop" },
      {
        type: "result",
        result: {
          final_content: text,
          usage: { input_tokens: 5, output_tokens: 7, total_tokens: 12 },
          finish_reason: "stop"
        }
      }
    ];
    process.stdout.write(lines.map((line) => JSON.stringify(line)).join("\n") + "\n");
    process.exit(0);
  }
  default:
    result = {};
}

process.stdout.write(JSON.stringify({ ok: true, result }));
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let runtime_path = root.join("bin/fixture_provider-provider");
        let mut permissions = fs::metadata(&runtime_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(runtime_path, permissions).unwrap();
    }
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_chat\n",
    )
    .unwrap();
    fs::write(
        root.join("models/llm/fixture_chat.yaml"),
        r#"model: fixture_chat
label: Fixture Chat
family: llm
capabilities:
  - stream
"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{ "plugin": { "label": "Fixture Provider" } }"#,
    )
    .unwrap();
    fs::write(root.join("demo/index.html"), "<html></html>").unwrap();
    fs::write(root.join("scripts/demo.sh"), "echo demo").unwrap();
}

pub(super) async fn create_ready_provider_instance(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
) -> String {
    let package_root = std::env::temp_dir().join(format!(
        "application-runtime-provider-{}",
        uuid::Uuid::now_v7()
    ));
    create_provider_fixture(&package_root);

    let install = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/plugins/install")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "package_root": package_root.display().to_string() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(install.status(), StatusCode::CREATED);
    let install_payload: Value =
        serde_json::from_slice(&to_bytes(install.into_body(), usize::MAX).await.unwrap()).unwrap();
    let installation_id = install_payload["data"]["installation"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    for suffix in ["enable", "assign"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/console/plugins/{installation_id}/{suffix}"))
                    .header("cookie", cookie)
                    .header("x-csrf-token", csrf)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/model-providers/instances")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": installation_id,
                        "display_name": "Fixture Runtime",
                        "configured_models": [
                            { "model_id": "fixture_chat", "enabled": true }
                        ],
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "super-secret"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let instance_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let validate = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/model-providers/instances/{instance_id}/validate"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(validate.status(), StatusCode::OK);

    instance_id
}

fn build_ready_provider_document(flow_id: &str, provider_instance_id: &str) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id, "name": "Support Agent", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "source_instance_id": provider_instance_id,
                            "model_id": "fixture_chat"
                        },
                        "temperature": 0.2
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "selector", "value": ["node-llm", "text"] }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-llm", "source": "node-start", "target": "node-llm", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-llm-answer", "source": "node-llm", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

pub(super) async fn seed_agent_flow_application(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    provider_instance_id: &str,
) -> String {
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
                        "name": "Support Agent",
                        "description": "runtime",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    let application_id = payload["data"]["id"].as_str().unwrap().to_string();
    let state = app
        .clone()
        .oneshot(
            Request::builder()
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
    let state_body = to_bytes(state.into_body(), usize::MAX).await.unwrap();
    let state_payload: Value = serde_json::from_slice(&state_body).unwrap();
    let flow_id = state_payload["data"]["draft"]["document"]["meta"]["flowId"]
        .as_str()
        .unwrap();

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
                        "document": build_ready_provider_document(flow_id, provider_instance_id),
                        "change_kind": "logical",
                        "summary": "seed provider-ready flow"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(save.status(), StatusCode::OK);
    application_id
}

async fn create_application_public_api_key(
    app: &axum::Router,
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
                        "name": "Support Agent public key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["token"].as_str().unwrap().to_string()
}

async fn publish_application_public_api(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
) {
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
                                "inputs_target": "node-start",
                                "history_target": "node-start.history",
                                "attachments_target": "node-start.files"
                            },
                            "output": {
                                "answer_selector": null,
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

fn build_human_input_document(flow_id: &str, provider_instance_id: &str) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id, "name": "Support Agent", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-llm",
                    "type": "llm",
                    "alias": "LLM",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "model_provider": {
                            "provider_code": "fixture_provider",
                            "source_instance_id": provider_instance_id,
                            "model_id": "fixture_chat"
                        },
                        "temperature": 0.2
                    },
                    "bindings": {
                        "prompt_messages": { "kind": "prompt_messages", "value": [{ "id": "user-1", "role": "user", "content": { "kind": "templated_text", "value": "{{node-start.query}}" } }] }
                    },
                    "outputs": [{ "key": "text", "title": "模型输出", "valueType": "string" }]
                },
                {
                    "id": "node-human",
                    "type": "human_input",
                    "alias": "Human Input",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "prompt": { "kind": "templated_text", "value": "请审核：{{ node-llm.text }}" }
                    },
                    "outputs": [{ "key": "input", "title": "人工输入", "valueType": "string" }]
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 720, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "selector", "value": ["node-human", "input"] }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-llm", "source": "node-start", "target": "node-llm", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-llm-human", "source": "node-llm", "target": "node-human", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-human-answer", "source": "node-human", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

async fn seed_human_input_application(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    provider_instance_id: &str,
) -> String {
    let application_id = seed_agent_flow_application(app, cookie, csrf, provider_instance_id).await;
    let state = app
        .clone()
        .oneshot(
            Request::builder()
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
    let state_body = to_bytes(state.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&state_body).unwrap();
    let flow_id = payload["data"]["draft"]["document"]["meta"]["flowId"]
        .as_str()
        .unwrap();

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
                        "document": build_human_input_document(flow_id, provider_instance_id),
                        "change_kind": "logical",
                        "summary": "seed human input flow"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(save.status(), StatusCode::OK);
    application_id
}

async fn wait_for_run_detail(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    expected_statuses: &[&str],
) -> Value {
    let mut last_status = String::new();
    let mut last_error = Value::Null;
    for _ in 0..200 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/console/applications/{application_id}/logs/runs/{run_id}/trace-tree"
                    ))
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        let status = payload["data"]["flow_run"]["status"]
            .as_str()
            .unwrap_or_default();
        last_status = status.to_string();
        last_error = payload["data"]["flow_run"]["error_payload"].clone();
        if expected_statuses.contains(&status) {
            return payload["data"].clone();
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!(
        "timed out waiting for run status: {expected_statuses:?}, last status: {last_status}, last error: {last_error}"
    );
}

async fn load_trace_node_content_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    trace_node_id: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/content"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_node_detail_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    trace_node_id: &str,
    detail_ref_id: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/trace-tree/nodes/{trace_node_id}/details/{detail_ref_id}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_node_detail_payload_for_kind(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    trace_node_id: &str,
    detail_kind: &str,
) -> Option<Value> {
    let content_payload =
        load_trace_node_content_payload(app, cookie, application_id, run_id, trace_node_id).await;
    let detail_ref_id = content_payload["data"]["detail_refs"]
        .as_array()
        .and_then(|detail_refs| {
            detail_refs
                .iter()
                .find(|detail_ref| detail_ref["detail_kind"].as_str() == Some(detail_kind))
        })
        .and_then(|detail_ref| detail_ref["detail_ref_id"].as_str())?
        .to_string();

    Some(
        load_trace_node_detail_payload(
            app,
            cookie,
            application_id,
            run_id,
            trace_node_id,
            &detail_ref_id,
        )
        .await,
    )
}

async fn resolve_runtime_debug_artifact_value(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    value: &Value,
) -> Value {
    if value["__runtime_debug_artifact"] != true {
        return value.clone();
    }

    let artifact_ref = value["artifact_ref"]
        .as_str()
        .expect("debug artifact preview should include artifact_ref");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/{artifact_ref}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn wait_for_persisted_text_delta_events(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
) -> Vec<Value> {
    wait_for_run_detail(
        app,
        cookie,
        application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    let mut last_event_types = Vec::new();
    for _ in 0..200 {
        let detail = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/console/applications/{application_id}/logs/runs/{run_id}/debug-stream"
                    ))
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(detail.status(), StatusCode::OK);
        let body = to_bytes(detail.into_body(), usize::MAX).await.unwrap();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        let parts = payload["data"]["parts"].as_array().unwrap();
        last_event_types = parts
            .iter()
            .filter_map(|part| {
                part["payload"]["event_type"]
                    .as_str()
                    .map(ToString::to_string)
            })
            .collect();
        let has_terminal_stream_event = parts.iter().any(|part| {
            matches!(
                part["payload"]["event_type"].as_str(),
                Some(
                    "flow_finished"
                        | "flow_failed"
                        | "flow_cancelled"
                        | "finish"
                        | "flow_run_completed"
                        | "flow_run_failed"
                        | "flow_run_cancelled"
                )
            )
        });
        if has_terminal_stream_event {
            let text_delta_events = parts
                .iter()
                .filter(|part| part["payload"]["event_type"].as_str() == Some("text_delta"))
                .cloned()
                .collect::<Vec<_>>();
            if !text_delta_events.is_empty() {
                return text_delta_events;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!(
        "timed out waiting for persisted runtime stream terminal event, last event types: {last_event_types:?}"
    );
}

fn sse_data_payload(frame: &str) -> Value {
    let data = frame
        .lines()
        .find_map(|line| line.strip_prefix("data:"))
        .expect("sse frame should include data")
        .trim();
    serde_json::from_str(data).expect("sse data should be json")
}

mod artifacts_billing_routes;
mod lazy_tree_contract_routes;
mod logs_routes;
mod read_side_effect_routes;
mod resume_cancel_routes;
mod stream_routes;
