use std::{fs, path::Path};

use crate::_tests::support::{
    login_and_capture_cookie, read_first_sse_frame, test_app, write_provider_manifest_v2,
    write_test_executable,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

fn create_split_delta_provider_fixture(root: &Path) {
    fs::create_dir_all(root.join("provider")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("models/llm")).unwrap();
    fs::create_dir_all(root.join("i18n")).unwrap();
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
    write_test_executable(
        &root.join("bin/fixture_provider-provider"),
        r#"#!/usr/bin/env node
const fs = require('node:fs');

const request = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
const listModels = [{
  model_id: "fixture_chat",
  display_name: "Fixture Chat",
  source: "dynamic",
  supports_streaming: true,
  supports_tool_call: false,
  supports_multimodal: false,
  provider_metadata: {}
}];

let result = {};
switch (request.method) {
  case 'validate':
    result = {
      sanitized: {
        api_key: request.input?.api_key ? "***" : null
      }
    };
    break;
  case 'list_models':
    result = listModels;
    break;
  case 'invoke': {
    const query = request.input?.messages?.[0]?.content ?? "";
    const lines = [
      { type: "text_delta", delta: "reply:" },
      { type: "text_delta", delta: query },
      { type: "usage_snapshot", usage: { input_tokens: 5, output_tokens: 7, total_tokens: 12 } },
      { type: "finish", reason: "stop" },
      {
        type: "result",
        result: {
          final_content: "reply:" + query,
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
    );
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
}

async fn create_ready_provider_instance(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let package_root = std::env::temp_dir().join(format!(
        "application-runtime-provider-stream-{}",
        uuid::Uuid::now_v7()
    ));
    create_split_delta_provider_fixture(&package_root);

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

async fn seed_agent_flow_application(
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

#[tokio::test]
async fn debug_run_stream_returns_flow_accepted_before_runtime_continuation_finishes() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-runs/stream"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": {
                            "node-start": { "query": "请总结退款政策" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let stream_text = read_first_sse_frame(response).await;

    assert!(
        stream_text.contains("event: flow_accepted"),
        "{stream_text}"
    );
    assert!(stream_text.contains("id: "), "{stream_text}");
    assert!(stream_text.contains("\"event_id\""), "{stream_text}");
    assert!(stream_text.contains("\"sequence\":1"), "{stream_text}");
    assert!(stream_text.contains("\"payload\""), "{stream_text}");
    assert!(
        stream_text.contains("\"type\":\"flow_accepted\""),
        "{stream_text}"
    );
}
