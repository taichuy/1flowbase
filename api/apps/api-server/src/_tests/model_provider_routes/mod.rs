use std::{fs, path::Path};

use crate::_tests::support::{login_and_capture_cookie, test_app, write_provider_manifest_v2};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

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
      control: slider
      send_mode: optional
      enabled_by_default: true
      default_value: 0.7
      min: 0
      max: 2
      step: 0.1
      visible_when:
        - field: mode
          operator: equals
          value: balanced
      disabled_when:
        - field: quota
          operator: in
          values: [0, null]
    - key: mode
      label: Mode
      type: string
      control: select
      options:
        - label: Balanced
          value: balanced
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
  - key: api_protocol
    type: enum
    required: false
    control: select
    default_value: openai_chat
    options:
      - label: OpenAI Chat Completions
        value: openai_chat
      - label: OpenAI Responses
        value: openai_responses
  - key: organization
    type: string
    required: false
    advanced: true
"#,
    )
    .unwrap();
    write_fixture_provider_runtime_script(&root.join("bin/fixture_provider-provider"));
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
provider_metadata: {}
"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/en_US.json"),
        r#"{
  "plugin": {
    "label": "Fixture Provider",
    "description": "Fixture provider plugin"
  },
  "provider": {
    "label": "Fixture Provider",
    "description": "Fixture provider"
  },
  "models": {
    "fixture_chat": {
      "label": "Fixture Chat",
      "description": "Fixture chat model"
    }
  }
}"#,
    )
    .unwrap();
    fs::write(
        root.join("i18n/zh_Hans.json"),
        r#"{
  "plugin": {
    "label": "示例供应商插件",
    "description": "示例供应商插件"
  },
  "provider": {
    "label": "示例供应商",
    "description": "示例供应商"
  },
  "models": {
    "fixture_chat": {
      "label": "示例聊天模型",
      "description": "示例聊天模型"
    }
  }
}"#,
    )
    .unwrap();
    fs::write(root.join("demo/index.html"), "<html></html>").unwrap();
    fs::write(root.join("scripts/demo.sh"), "echo demo").unwrap();
}

fn write_fixture_provider_runtime_script(path: &Path) {
    fs::write(
        path,
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
  case 'balance':
    result = {
      is_available: true,
      balance_infos: [{
        currency: "CNY",
        total_balance: "110.00",
        granted_balance: "10.00",
        topped_up_balance: "100.00"
      }],
      provider_metadata: { provider: "deepseek", echoed_api_key: request.input?.api_key }
    };
    break;
  case 'invoke': {
    const query = request.input?.messages?.[0]?.content ?? "";
    const lines = [
      { type: "text_delta", delta: "reply:" + query },
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
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

async fn install_enable_assign(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let package_root =
        std::env::temp_dir().join(format!("model-provider-route-{}", uuid::Uuid::now_v7()));
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

    let enable = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_id}/enable"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(enable.status(), StatusCode::OK);

    let assign = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/plugins/{installation_id}/assign"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(assign.status(), StatusCode::OK);

    installation_id
}

async fn openapi_payload() -> Value {
    let response = crate::app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn schema_ref_name(schema: &Value) -> Option<String> {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|value| value.split('/').next_back())
        .map(str::to_string)
        .or_else(|| {
            schema
                .get("allOf")
                .and_then(Value::as_array)
                .and_then(|items| items.iter().find_map(schema_ref_name))
        })
        .or_else(|| {
            schema
                .get("anyOf")
                .and_then(Value::as_array)
                .and_then(|items| items.iter().find_map(schema_ref_name))
        })
        .or_else(|| {
            schema
                .get("oneOf")
                .and_then(Value::as_array)
                .and_then(|items| items.iter().find_map(schema_ref_name))
        })
        .or_else(|| {
            schema
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get("data"))
                .and_then(schema_ref_name)
        })
}

mod lifecycle;
mod refresh_and_settings;
mod request_log_maintenance;
mod settings_feature;
