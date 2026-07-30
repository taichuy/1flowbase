use std::fs;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use crate::_tests::support::write_provider_manifest_v2;

use super::*;

const V2_OPAQUE_CANARY: &str = "opaque-v2-canary-k3";
const PRIVATE_TURN_MARKER: &str = "K3-CODEX-TURN-METADATA-MUST-NOT-PERSIST";

#[derive(Clone, Copy)]
enum CompactFixtureMode {
    Success,
    ProviderFailure,
}

#[tokio::test]
async fn k3_codex_turn_metadata_is_not_parsed_before_application_key_authentication() {
    let app = test_app().await;

    let response = post_openai_responses(
        &app,
        "/v1/responses",
        "not-an-application-api-key",
        responses_body(false),
        Some(json!("not a Codex metadata object")),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("not_authenticated"));
}

#[tokio::test]
async fn k3_legacy_compact_returns_exact_provider_items_from_a_workflow_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_compact_published_app(
        &app,
        "OpenAI Legacy Compact Exact Items App",
        CompactFixtureMode::Success,
    )
    .await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_openai_responses(
        &app,
        "/v1/responses/compact",
        &token,
        responses_body(false),
        None,
    )
    .await;

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(
        payload,
        json!([
            {
                "id": "msg_compact_canary",
                "type": "message",
                "status": "completed",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": "legacy compact exact canary",
                    "annotations": []
                }]
            }
        ])
    );
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn k3_v2_compact_stream_preserves_one_opaque_item_from_a_workflow_run() {
    let (app, state) = test_app_with_state().await;
    let token = setup_compact_published_app(
        &app,
        "OpenAI V2 Compact Opaque SSE App",
        CompactFixtureMode::Success,
    )
    .await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_openai_responses(
        &app,
        "/v1/responses",
        &token,
        v2_compaction_body(true),
        Some(codex_turn_metadata("responses_compaction_v2")),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Compact SSE body should be readable")
            .to_vec(),
    )
    .expect("Compact SSE body should be UTF-8");
    assert!(body.contains("event: response.completed"), "{body}");
    assert_eq!(body.matches(V2_OPAQUE_CANARY).count(), 1, "{body}");
    let event = sse_json_event(&body, "response.completed");
    assert_eq!(event["type"], json!("response.completed"));
    assert_eq!(event["response"]["id"], json!("resp-v2-canary"));
    let output = event["response"]["output"]
        .as_array()
        .expect("completed Compact response should contain an output array");
    assert_eq!(output.len(), 1);
    assert_eq!(output[0]["type"], json!("compaction"));
    assert_eq!(output[0]["encrypted_content"], json!(V2_OPAQUE_CANARY));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

#[tokio::test]
async fn k3_local_summary_keeps_generate_sse_and_does_not_persist_codex_turn_header() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Local Summary Generate App").await;
    let before = flow_run_count(state.as_ref()).await;
    let metadata = json!({
        "request_kind": "compaction",
        "compaction": {"implementation": "responses"},
        "private_turn_marker": PRIVATE_TURN_MARKER
    });

    let response = post_openai_responses(
        &app,
        "/v1/responses",
        &token,
        responses_body(true),
        Some(metadata),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("local summary SSE body should be readable")
            .to_vec(),
    )
    .expect("local summary SSE body should be UTF-8");
    assert!(body.contains("event: response.completed"), "{body}");
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
    let input_payload: Value = sqlx::query_scalar(
        "select input_payload from flow_runs order by created_at desc, id desc limit 1",
    )
    .fetch_one(state.store.pool())
    .await
    .expect("local summary flow run should retain its normal input payload");
    assert!(
        !input_payload.to_string().contains(PRIVATE_TURN_MARKER),
        "captured Codex turn metadata must not enter the persisted Generate input"
    );
}

#[tokio::test]
async fn k3_compact_provider_failure_is_an_error_without_completed_projection() {
    let (app, state) = test_app_with_state().await;
    let token = setup_compact_published_app(
        &app,
        "OpenAI Compact Provider Failure App",
        CompactFixtureMode::ProviderFailure,
    )
    .await;
    let before = flow_run_count(state.as_ref()).await;

    let response = post_openai_responses(
        &app,
        "/v1/responses",
        &token,
        v2_compaction_body(true),
        Some(codex_turn_metadata("responses_compaction_v2")),
    )
    .await;

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{payload}");
    assert_eq!(payload["error"]["code"], json!("provider_upstream_error"));
    assert!(payload.get("response").is_none());
    assert!(!payload.to_string().contains("response.completed"));
    assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
}

async fn setup_compact_published_app(app: &Router, name: &str, mode: CompactFixtureMode) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    let provider_instance_id = create_compact_provider_instance(app, &cookie, &csrf, mode).await;
    publish_compact_application(app, &cookie, &csrf, &application_id, &provider_instance_id).await;
    token
}

async fn create_compact_provider_instance(
    app: &Router,
    cookie: &str,
    csrf: &str,
    mode: CompactFixtureMode,
) -> String {
    let package_root = std::env::temp_dir().join(format!(
        "application-public-api-compact-provider-{}",
        Uuid::now_v7()
    ));
    write_compact_provider_fixture(&package_root);

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
                .expect("Compact fixture install request should build"),
        )
        .await
        .expect("Compact fixture package install should respond");
    assert_eq!(install.status(), StatusCode::CREATED);
    let install_payload: Value = serde_json::from_slice(
        &to_bytes(install.into_body(), usize::MAX)
            .await
            .expect("Compact fixture install body should be readable"),
    )
    .expect("Compact fixture install body should be JSON");
    let installation_id = install_payload["data"]["installation"]["id"]
        .as_str()
        .expect("Compact fixture install should return an installation id");

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
                    .expect("Compact fixture plugin lifecycle request should build"),
            )
            .await
            .expect("Compact fixture plugin lifecycle should respond");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let mode = match mode {
        CompactFixtureMode::Success => "success",
        CompactFixtureMode::ProviderFailure => "provider_failure",
    };
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
                        "display_name": format!("Compact Fixture Runtime {}", Uuid::now_v7()),
                        "configured_models": [{"model_id": "fixture_compact", "enabled": true}],
                        "config": {
                            "base_url": "https://api.example.com",
                            "api_key": "super-secret",
                            "test_compact_mode": mode
                        }
                    })
                    .to_string(),
                ))
                .expect("Compact fixture provider create request should build"),
        )
        .await
        .expect("Compact fixture provider create should respond");
    assert_eq!(create.status(), StatusCode::CREATED);
    let create_payload: Value = serde_json::from_slice(
        &to_bytes(create.into_body(), usize::MAX)
            .await
            .expect("Compact fixture provider create body should be readable"),
    )
    .expect("Compact fixture provider create body should be JSON");
    let instance_id = create_payload["data"]["id"]
        .as_str()
        .expect("Compact fixture provider create should return an instance id")
        .to_string();

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
                .expect("Compact fixture provider validation request should build"),
        )
        .await
        .expect("Compact fixture provider validation should respond");
    assert_eq!(validate.status(), StatusCode::OK);

    instance_id
}

async fn publish_compact_application(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    provider_instance_id: &str,
) {
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
                .expect("Compact fixture orchestration request should build"),
        )
        .await
        .expect("Compact fixture orchestration should respond");
    assert_eq!(state.status(), StatusCode::OK);
    let mut document = response_json(state).await["data"]["draft"]["document"].clone();
    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("Compact fixture draft should contain nodes");
    let start_node = nodes
        .iter_mut()
        .find(|node| node["type"] == "start")
        .expect("Compact fixture draft should contain a start node");
    start_node["config"]["model_list"] = json!(["fixture_compact"]);
    let llm_node = nodes
        .iter_mut()
        .find(|node| node["type"] == "llm")
        .expect("Compact fixture draft should contain an LLM node");
    llm_node["config"]["model_provider"] = json!({
        "provider_code": "fixture_compact_provider",
        "source_instance_id": provider_instance_id,
        "model_id": "fixture_compact"
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
                        "summary": "Configure Compact provider route"
                    })
                    .to_string(),
                ))
                .expect("Compact fixture draft save request should build"),
        )
        .await
        .expect("Compact fixture draft save should respond");
    assert_eq!(save.status(), StatusCode::OK);

    let publish = app
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
                .expect("Compact fixture publication request should build"),
        )
        .await
        .expect("Compact fixture publication should respond");
    assert_eq!(publish.status(), StatusCode::CREATED);
}

fn write_compact_provider_fixture(root: &std::path::Path) {
    fs::create_dir_all(root.join("provider")).expect("Compact fixture provider dir should exist");
    fs::create_dir_all(root.join("bin")).expect("Compact fixture binary dir should exist");
    fs::create_dir_all(root.join("models/llm")).expect("Compact fixture models dir should exist");
    fs::create_dir_all(root.join("i18n")).expect("Compact fixture i18n dir should exist");
    write_provider_manifest_v2(
        root,
        "fixture_compact_provider",
        "Fixture Compact Provider",
        "0.1.0",
    );
    let mut manifest = fs::read_to_string(root.join("manifest.yaml"))
        .expect("Compact fixture manifest should be readable");
    manifest.push_str(
        "  capabilities:\n    - compact.responses_compact\n    - compact.responses_compaction_v2\n    - responses.native_passthrough\n",
    );
    fs::write(root.join("manifest.yaml"), manifest)
        .expect("Compact fixture manifest should declare both Compact capabilities");
    fs::write(
        root.join("icon.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .expect("Compact fixture icon should be writable");
    fs::write(
        root.join("provider/fixture_compact_provider.yaml"),
        r#"provider_code: fixture_compact_provider
display_name: Fixture Compact Provider
protocol: openai_compatible
model_discovery: static
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
  - key: test_compact_mode
    type: string
    required: false
"#,
    )
    .expect("Compact fixture provider schema should be writable");
    fs::write(
        root.join("bin/fixture_compact_provider-provider"),
        r#"#!/usr/bin/env node
const fs = require('node:fs');

const request = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
let result = {};

switch (request.method) {
  case 'validate':
    result = { sanitized: { api_key: request.input?.api_key ? '***' : null } };
    break;
  case 'list_models':
    result = [{
      model_id: 'fixture_compact',
      display_name: 'Fixture Compact',
      source: 'dynamic',
      supports_streaming: false,
      supports_tool_call: false,
      supports_multimodal: false,
      provider_metadata: {}
    }];
    break;
  case 'invoke': {
    const input = request.input ?? {};
    if (input.operation !== 'compact') {
      process.stdout.write(JSON.stringify({
        ok: false,
        error: { kind: 'provider_invalid_response', message: 'expected Compact operation' }
      }));
      process.exit(0);
    }
    if (input.provider_config?.test_compact_mode === 'provider_failure') {
      process.stdout.write(JSON.stringify({
        ok: false,
        error: { kind: 'provider_upstream_error', message: 'fixture Compact upstream failure' }
      }));
      process.exit(0);
    }
    if (input.profile === 'responses_compact') {
      result = {
        result_type: 'response_items',
        operation: 'compact',
        profile: 'responses_compact',
        response_items: [{
          id: 'msg_compact_canary',
          type: 'message',
          status: 'completed',
          role: 'assistant',
          content: [{
            type: 'output_text',
            text: 'legacy compact exact canary',
            annotations: []
          }]
        }]
      };
    } else if (input.profile === 'responses_compaction_v2') {
      result = {
        result_type: 'completed_opaque_compaction_item',
        operation: 'compact',
        profile: 'responses_compaction_v2',
        response_id: 'resp-v2-canary',
        compaction_item: {
          id: 'compaction_v2_canary',
          type: 'compaction',
          encrypted_content: 'opaque-v2-canary-k3'
        },
        encrypted_content: 'opaque-v2-canary-k3'
      };
    } else {
      process.stdout.write(JSON.stringify({
        ok: false,
        error: { kind: 'provider_invalid_response', message: 'unexpected Compact profile' }
      }));
      process.exit(0);
    }
    break;
  }
  default:
    result = {};
}

process.stdout.write(JSON.stringify({ ok: true, result }));
"#,
    )
    .expect("Compact fixture runtime should be writable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let runtime_path = root.join("bin/fixture_compact_provider-provider");
        let mut permissions = fs::metadata(&runtime_path)
            .expect("Compact fixture runtime metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(runtime_path, permissions)
            .expect("Compact fixture runtime should be executable");
    }
    fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_compact\n",
    )
    .expect("Compact fixture model position should be writable");
    fs::write(
        root.join("models/llm/fixture_compact.yaml"),
        "model: fixture_compact\nlabel: Fixture Compact\nfamily: llm\ncapabilities:\n  - stream\n",
    )
    .expect("Compact fixture model should be writable");
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{ "plugin": { "label": "Fixture Compact Provider" } }"#,
    )
    .expect("Compact fixture i18n should be writable");
}

async fn post_openai_responses(
    app: &Router,
    uri: &str,
    token: &str,
    body: Value,
    codex_metadata: Option<Value>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json");
    if let Some(codex_metadata) = codex_metadata {
        request = request.header("x-codex-turn-metadata", codex_metadata.to_string());
    }
    app.clone()
        .oneshot(
            request
                .body(Body::from(body.to_string()))
                .expect("OpenAI Compact request should build"),
        )
        .await
        .expect("OpenAI Compact request should respond")
}

fn v2_compaction_body(stream: bool) -> Value {
    json!({
        "model": "provider/custom-model:latest",
        "stream": stream,
        "input": [
            {"type": "message", "role": "user", "content": "retain the current turn"},
            {"type": "compaction_trigger"}
        ],
        "metadata": {"trace_id": "compact-v2-trace"}
    })
}

fn codex_turn_metadata(implementation: &str) -> Value {
    json!({
        "request_kind": "compaction",
        "compaction": {"implementation": implementation}
    })
}

fn sse_json_event(body: &str, event_name: &str) -> Value {
    let mut current_event = None;
    for line in body.lines() {
        if let Some(name) = line.strip_prefix("event: ") {
            current_event = Some(name);
            continue;
        }
        if current_event == Some(event_name) {
            if let Some(data) = line.strip_prefix("data: ") {
                return serde_json::from_str(data).expect("OpenAI Compact SSE data should be JSON");
            }
        }
    }
    panic!("OpenAI Compact SSE should contain {event_name}");
}
