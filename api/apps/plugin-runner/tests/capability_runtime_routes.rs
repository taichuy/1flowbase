use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use plugin_runner::{app_with_state, AppState, CapabilityHost};
use serde_json::{json, Value};
use tower::ServiceExt;

struct TempCapabilityPackage {
    root: PathBuf,
}

impl TempCapabilityPackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("plugin-runner-capability-tests-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
}

impl Drop for TempCapabilityPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_fixture_runtime(package: &TempCapabilityPackage) {
    let validate_output = json!({
        "ok": true,
        "result": {
            "ok": true,
            "sanitized": {
                "api_key": "***",
                "endpoint": "https://api.example.com"
            }
        }
    })
    .to_string();
    let resolve_dynamic_options_output = json!({
        "ok": true,
        "result": {
            "fields": [
                {
                    "key": "tone",
                    "label": "Tone",
                    "type": "string",
                    "options": [
                        {
                            "label": "Formal",
                            "value": "formal"
                        }
                    ]
                }
            ]
        }
    })
    .to_string();
    let resolve_output_schema_output = json!({
        "ok": true,
        "result": {
            "schema_version": "1flowbase.capability.output/v1",
            "title": "Fixture Output Schema"
        }
    })
    .to_string();
    let execute_output = json!({
        "ok": true,
        "result": {
            "answer": "echo:fixture_capability"
        }
    })
    .to_string();

    package.write(
        "bin/fixture_capability",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${{payload}}" in
  *'"method":"validate_config"'*)
    printf '%s' '{validate_output}'
    ;;
  *'"method":"resolve_dynamic_options"'*)
    printf '%s' '{resolve_dynamic_options_output}'
    ;;
  *'"method":"resolve_output_schema"'*)
    printf '%s' '{resolve_output_schema_output}'
    ;;
  *'"method":"execute"'*)
    printf '%s' '{execute_output}'
    ;;
  *)
    printf '%s' '{{"ok":false,"error":{{"message":"unknown method"}}}}'
    exit 1
    ;;
esac
"#
        ),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_capability");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn make_fixture_package() -> TempCapabilityPackage {
    let package = TempCapabilityPackage::new();
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_capability@0.1.0
version: 0.1.0
publisher_namespace: taichuy
vendor: taichuy
display_name: Fixture Capability
description: Fixture capability package
icon: icon.svg
source_kind: uploaded
trust_level: unverified
consumption_kind: capability_plugin
execution_mode: process_per_call
slot_codes:
  - node_contribution
binding_targets:
  - workspace
selection_mode: manual_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.capability/v1
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: none
  secrets: none
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_capability
  limits:
    memory_bytes: 134217728
    timeout_ms: 5000
node_contributions:
  - contribution_code: fixture_action
    node_shell: action
    category: automation
    title: Fixture Action
    description: Fixture capability node
    icon: puzzle
    schema_ui: {}
    schema_version: 1flowbase.node-contribution/v2
    output_schema:
      outputs:
        - key: result
          title: Result
          valueType: json
    side_effect_policy: external_read
    infra_contracts: []
    required_auth:
      - provider_instance
    visibility: public
    experimental: false
    dependency:
      installation_kind: optional
      plugin_version_range: ">=0.1.0"
"#,
    );
    write_fixture_runtime(&package);
    package.write(
        "i18n/en_US.json",
        r#"{
  "plugin": {
    "label": "Fixture Capability"
  }
}
"#,
    );
    package
}

async fn request_json(app: &Router, method: Method, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice(&body).unwrap();
    (status, payload)
}

#[tokio::test]
async fn capability_runtime_routes_cover_validate_resolve_and_execute() {
    let package = make_fixture_package();
    let mut capability_host = CapabilityHost::default();
    capability_host.load(package.path()).unwrap();
    let app = app_with_state(AppState::with_capability_host(capability_host));

    let (status, validate_payload) = request_json(
        &app,
        Method::POST,
        "/capabilities/validate-config",
        json!({
            "plugin_id": "fixture_capability@0.1.0",
            "contribution_code": "fixture_action",
            "config_payload": {
                "api_key": "secret",
                "endpoint": "https://api.example.com"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(validate_payload["output"]["sanitized"]["api_key"], "***");

    let (status, dynamic_options_payload) = request_json(
        &app,
        Method::POST,
        "/capabilities/resolve-dynamic-options",
        json!({
            "plugin_id": "fixture_capability@0.1.0",
            "contribution_code": "fixture_action",
            "config_payload": {
                "api_key": "secret"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        dynamic_options_payload["output"]["fields"][0]["options"][0]["label"],
        "Formal"
    );

    let (status, output_schema_payload) = request_json(
        &app,
        Method::POST,
        "/capabilities/resolve-output-schema",
        json!({
            "plugin_id": "fixture_capability@0.1.0",
            "contribution_code": "fixture_action",
            "config_payload": {
                "api_key": "secret"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        output_schema_payload["output"]["schema_version"],
        "1flowbase.capability.output/v1"
    );

    let (status, execute_payload) = request_json(
        &app,
        Method::POST,
        "/capabilities/execute",
        json!({
            "plugin_id": "fixture_capability@0.1.0",
            "contribution_code": "fixture_action",
            "config_payload": {
                "api_key": "secret"
            },
            "input_payload": {
                "prompt": "hello"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        execute_payload["output_payload"]["answer"],
        "echo:fixture_capability"
    );
}
