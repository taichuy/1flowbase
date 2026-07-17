use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use plugin_runner::app;
use serde_json::{json, Value};
use tower::ServiceExt;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempProviderPackage {
    root: PathBuf,
}

impl TempProviderPackage {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "plugin-runner-tests-{}-{nonce}-{sequence}",
            std::process::id()
        ));
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

impl Drop for TempProviderPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn write_fixture_runtime_with_invoke_lines(
    package: &TempProviderPackage,
    dynamic_label: &str,
    invoke_lines: Vec<String>,
) {
    let validate_output = json!({
        "ok": true,
        "result": {
            "ok": true,
            "sanitized": {
                "base_url": "https://api.example.com",
                "api_key": "***"
            }
        }
    })
    .to_string();
    let list_models_output = json!({
        "ok": true,
        "result": [
            {
                "model_id": "fixture_dynamic",
                "display_name": dynamic_label,
                "source": "dynamic",
                "supports_streaming": true,
                "supports_tool_call": true,
                "supports_multimodal": false,
                "context_window": 64000,
                "max_output_tokens": 4096,
                "provider_metadata": {
                    "tier": "dynamic"
                }
            }
        ]
    })
    .to_string();
    let balance_output = json!({
        "ok": true,
        "result": {
            "is_available": true,
            "balance_infos": [
                {
                    "currency": "CNY",
                    "total_balance": "110.00",
                    "granted_balance": "10.00",
                    "topped_up_balance": "100.00"
                }
            ],
            "provider_metadata": {
                "provider": "deepseek"
            }
        }
    })
    .to_string();
    let error_output = json!({
        "ok": false,
        "error": {
            "kind": "provider_invalid_response",
            "message": "unknown method",
            "provider_summary": null
        }
    })
    .to_string();
    let validate_output = shell_quote(&validate_output);
    let list_models_output = shell_quote(&list_models_output);
    let balance_output = shell_quote(&balance_output);
    let error_output = shell_quote(&error_output);
    let invoke_output = invoke_lines
        .iter()
        .map(|line| format!("    printf '%s\\n' {}\n", shell_quote(line)))
        .collect::<String>();

    package.write(
        "bin/fixture_provider",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${{payload}}" in
  *'"method":"validate"'*)
    printf '%s' {validate_output}
    ;;
  *'"method":"list_models"'*)
    printf '%s' {list_models_output}
    ;;
  *'"method":"balance"'*)
    printf '%s' {balance_output}
    ;;
  *'"method":"invoke"'*)
{invoke_output}
    ;;
  *)
    printf '%s' {error_output}
    exit 1
    ;;
esac
"#
        ),
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn write_fixture_runtime(package: &TempProviderPackage, dynamic_label: &str) {
    write_fixture_runtime_with_invoke_lines(
        package,
        dynamic_label,
        vec![
            json!({
                "type": "text_delta",
                "delta": "echo:fixture_dynamic"
            })
            .to_string(),
            json!({
                "type": "tool_call_commit",
                "call": {
                    "id": "tool-1",
                    "name": "search_docs",
                    "arguments": {
                        "query": "provider host"
                    }
                }
            })
            .to_string(),
            json!({
                "type": "mcp_call_commit",
                "call": {
                    "id": "mcp-1",
                    "server": "docs",
                    "method": "search",
                    "arguments": {
                        "query": "provider host"
                    }
                }
            })
            .to_string(),
            json!({
                "type": "usage_snapshot",
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 7,
                    "total_tokens": 12
                }
            })
            .to_string(),
            json!({
                "type": "finish",
                "reason": "stop"
            })
            .to_string(),
            json!({
                "type": "result",
                "result": {
                "final_content": "echo:fixture_dynamic",
                "tool_calls": [
                    {
                        "id": "tool-1",
                        "name": "search_docs",
                        "arguments": {
                            "query": "provider host"
                        }
                    }
                ],
                "mcp_calls": [
                    {
                        "id": "mcp-1",
                        "server": "docs",
                        "method": "search",
                        "arguments": {
                            "query": "provider host"
                        }
                    }
                ],
                "usage": {
                    "input_tokens": 5,
                    "output_tokens": 7,
                    "total_tokens": 12
                },
                "finish_reason": "stop",
                "provider_metadata": {
                    "provider_code": "fixture_provider"
                }
            }
            })
            .to_string(),
        ],
    );
}

fn write_legacy_invoke_runtime(package: &TempProviderPackage) {
    write_fixture_runtime_with_invoke_lines(
        package,
        "Fixture Dynamic",
        vec![json!({
            "ok": true,
            "result": {
                "output_text": "legacy text"
            }
        })
        .to_string()],
    );
}

fn write_slow_invoke_runtime(package: &TempProviderPackage) {
    package.write(
        "bin/fixture_provider",
        r#"#!/usr/bin/env bash
set -euo pipefail

payload="$(cat)"
case "${payload}" in
  *'"method":"validate"'*)
    printf '%s' '{"ok":true,"result":{"ok":true,"sanitized":{"api_key":"***"}}}'
    ;;
  *'"method":"list_models"'*)
    printf '%s' '{"ok":true,"result":[]}'
    ;;
  *'"method":"invoke"'*)
    printf '%s\n' '{"type":"text_delta","delta":"slow"}'
    sleep 1
    printf '%s\n' '{"type":"finish","reason":"stop"}'
    printf '%s\n' '{"type":"result","result":{"final_content":"slow","finish_reason":"stop","provider_metadata":{"transport":"responses_websocket"}}}'
    ;;
  *)
    printf '%s' '{"ok":false,"error":{"kind":"provider_invalid_response","message":"unknown method","provider_summary":null}}'
    exit 1
    ;;
esac
"#,
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn make_fixture_package() -> TempProviderPackage {
    let package = TempProviderPackage::new();
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
vendor: taichuy
display_name: Fixture Provider
description: Fixture Provider
icon: icon.svg
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_provider
  limits:
    memory_bytes: 134217728
    timeout_ms: 5000
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
supports_model_fetch_without_credentials: true
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: true
"#,
    );
    write_fixture_runtime(&package, "Fixture Dynamic");
    package.write("models/llm/_position.yaml", "items:\n  - fixture_static\n");
    package.write(
        "models/llm/fixture_static.yaml",
        r#"model: fixture_static
label: Fixture Static
family: llm
capabilities:
  - stream
context_window: 32000
max_output_tokens: 2048
"#,
    );
    package.write(
        "i18n/en_US.json",
        r#"{
  "plugin": { "label": "Fixture Provider" },
  "provider": { "label": "Fixture Provider" }
}
"#,
    );
    package
}

fn make_stateful_fixture_package() -> TempProviderPackage {
    let package = TempProviderPackage::new();
    package.write(
        "manifest.yaml",
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
vendor: taichuy
display_name: Fixture Provider
description: Fixture Provider
icon: icon.svg
source_kind: uploaded
trust_level: unverified
consumption_kind: runtime_extension
execution_mode: stateful_provider_worker
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json_worker
  entry: bin/fixture_provider
  limits:
    memory_bytes: 134217728
    timeout_ms: 5000
"#,
    );
    package.write(
        "provider/fixture_provider.yaml",
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
model_discovery: static
config_schema:
  - key: api_key
    type: secret
    required: true
"#,
    );
    package.write(
        "bin/fixture_provider",
        r#"#!/usr/bin/env bash
set -euo pipefail
count=0
while IFS= read -r payload; do
  case "${payload}" in
    *'"method":"validate"'*)
      printf '%s\n' '{"ok":true,"result":{"ok":true,"sanitized":{"api_key":"***"}}}'
      ;;
    *'"method":"list_models"'*)
      printf '%s\n' '{"ok":true,"result":[]}'
      ;;
    *'"method":"invoke"'*)
      count=$((count + 1))
      printf '%s\n' "{\"type\":\"text_delta\",\"delta\":\"worker-turn-${count}\"}"
      printf '%s\n' "{\"type\":\"finish\",\"reason\":\"stop\"}"
      printf '%s\n' "{\"type\":\"result\",\"result\":{\"final_content\":\"worker-turn-${count}\",\"finish_reason\":\"stop\",\"provider_metadata\":{\"worker_turn\":${count}}}}"
      ;;
    *)
      printf '%s\n' '{"ok":false,"error":{"kind":"provider_invalid_response","message":"unknown method"}}'
      ;;
  esac
done
"#,
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
    package.write(
        "i18n/en_US.json",
        r#"{
  "plugin": { "label": "Fixture Provider" },
  "provider": { "label": "Fixture Provider" }
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
                .method(method.clone())
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice(&body).unwrap_or_else(|error| {
        panic!(
            "{method} {uri} returned a non-JSON response ({status}): {error}; body={}",
            String::from_utf8_lossy(&body)
        )
    });
    (status, payload)
}

async fn request_empty(app: &Router, method: Method, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload = serde_json::from_slice(&body).unwrap();
    (status, payload)
}

async fn wait_for_single_active_stream(app: &Router) -> Value {
    for _ in 0..20 {
        let (status, payload) = request_empty(app, Method::GET, "/providers/active-streams").await;
        assert_eq!(status, StatusCode::OK);
        if payload["streams"]
            .as_array()
            .is_some_and(|streams| streams.len() == 1)
        {
            return payload;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!("active stream snapshot did not expose the running stream");
}

fn find_model<'a>(models: &'a [Value], model_id: &str) -> &'a Value {
    models
        .iter()
        .find(|model| model["model_id"] == model_id)
        .unwrap()
}

#[tokio::test]
async fn provider_runner_exposes_and_cleans_active_stream_snapshot() {
    let package = make_fixture_package();
    write_slow_invoke_runtime(&package);
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let plugin_id = load_payload["plugin_id"].as_str().unwrap().to_string();

    let invoke_app = app.clone();
    let invoke_body = json!({
        "plugin_id": plugin_id,
        "input": {
            "contract_version": "1flowbase.provider/v2",
            "provider_instance_id": "instance-active",
            "provider_code": "fixture_provider",
            "protocol": "openai_compatible",
            "model": "fixture_dynamic",
            "provider_config": {
                "transport_mode": "responses_websocket"
            },
            "messages": [
                {
                    "role": "user",
                    "content": "hello",
                }
            ]
        }
    });
    let invoke_task = tokio::spawn(async move {
        request_json(
            &invoke_app,
            Method::POST,
            "/providers/invoke-stream",
            invoke_body,
        )
        .await
    });

    let active_payload = wait_for_single_active_stream(&app).await;
    let stream = &active_payload["streams"][0];
    assert!(!stream["invocation_id"].as_str().unwrap().is_empty());
    assert_eq!(stream["plugin_id"], load_payload["plugin_id"]);
    assert_eq!(stream["provider_instance_id"], "instance-active");
    assert_eq!(stream["transport"], "responses_websocket");
    assert_eq!(stream["status"], "running");
    assert!(stream["duration_ms"].as_u64().is_some());
    assert!(!stream["started_at"].as_str().unwrap().is_empty());
    assert!(!stream["last_event_at"].as_str().unwrap().is_empty());

    let (status, invoke_payload) = invoke_task.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invoke_payload["result"]["final_content"], "slow");

    let (status, empty_payload) =
        request_empty(&app, Method::GET, "/providers/active-streams").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty_payload["streams"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn provider_runner_reload_is_not_blocked_by_running_invocation() {
    let package = make_fixture_package();
    write_slow_invoke_runtime(&package);
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let plugin_id = load_payload["plugin_id"].as_str().unwrap().to_string();

    let invoke_app = app.clone();
    let invoke_plugin_id = plugin_id.clone();
    let invoke_task = tokio::spawn(async move {
        request_json(
            &invoke_app,
            Method::POST,
            "/providers/invoke-stream",
            json!({
                "plugin_id": invoke_plugin_id,
                "input": {
                    "contract_version": "1flowbase.provider/v2",
                    "provider_instance_id": "instance-reload",
                    "provider_code": "fixture_provider",
                    "protocol": "openai_compatible",
                    "model": "fixture_dynamic",
                    "messages": [
                        {
                            "role": "user",
                            "content": "hello",
                        }
                    ]
                }
            }),
        )
        .await
    });
    wait_for_single_active_stream(&app).await;

    let reload = tokio::time::timeout(
        Duration::from_millis(200),
        request_json(
            &app,
            Method::POST,
            "/providers/reload",
            json!({
                "plugin_id": plugin_id,
            }),
        ),
    )
    .await
    .expect("provider reload should not wait for an external provider invocation to finish");
    assert_eq!(reload.0, StatusCode::OK);

    let (status, invoke_payload) = invoke_task.await.unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invoke_payload["result"]["final_content"], "slow");
}

#[tokio::test]
async fn provider_runner_cleans_active_stream_snapshot_after_error() {
    let package = make_fixture_package();
    write_legacy_invoke_runtime(&package);
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = request_json(
        &app,
        Method::POST,
        "/providers/invoke-stream",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "input": {
                "contract_version": "1flowbase.provider/v2",
                "provider_instance_id": "instance-error",
                "provider_code": "fixture_provider",
                "protocol": "openai_compatible",
                "model": "fixture_dynamic",
                "messages": [
                    {
                        "role": "user",
                        "content": "hello",
                    }
                ]
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);

    let (status, payload) = request_empty(&app, Method::GET, "/providers/active-streams").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(payload["streams"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn stateful_provider_runtime_route_reuses_worker_process_between_invokes() {
    let package = make_stateful_fixture_package();
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let invoke_body = json!({
        "plugin_id": load_payload["plugin_id"],
        "input": {
            "contract_version": "1flowbase.provider/v2",
            "provider_instance_id": "instance-1",
            "provider_code": "fixture_provider",
            "protocol": "openai_compatible",
            "model": "fixture_static",
            "messages": [
                {
                    "role": "user",
                    "content": "hello",
                }
            ]
        }
    });

    let (status, first_payload) = request_json(
        &app,
        Method::POST,
        "/providers/invoke-stream",
        invoke_body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, second_payload) =
        request_json(&app, Method::POST, "/providers/invoke-stream", invoke_body).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(first_payload["result"]["final_content"], "worker-turn-1");
    assert_eq!(second_payload["result"]["final_content"], "worker-turn-2");
    assert_eq!(
        second_payload["result"]["provider_metadata"]["worker_turn"],
        2
    );
}

#[tokio::test]
async fn provider_runtime_routes_cover_load_reload_validate_list_models_and_invoke_stream() {
    let package = make_fixture_package();
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(load_payload["provider_code"], "fixture_provider");
    assert_eq!(load_payload["model_discovery_mode"], "hybrid");
    let plugin_id = load_payload["plugin_id"].as_str().unwrap().to_string();

    let (status, validate_payload) = request_json(
        &app,
        Method::POST,
        "/providers/validate",
        json!({
            "plugin_id": plugin_id,
            "provider_config": {
                "base_url": "https://api.example.com",
                "api_key": "secret",
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(validate_payload["output"]["sanitized"]["api_key"], "***");

    let (status, models_payload) = request_json(
        &app,
        Method::POST,
        "/providers/list-models",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "provider_config": {
                "api_key": "secret",
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let models = models_payload["models"].as_array().unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(find_model(models, "fixture_static")["source"], "static");
    assert_eq!(find_model(models, "fixture_dynamic")["source"], "dynamic");
    assert_eq!(
        find_model(models, "fixture_dynamic")["display_name"],
        "Fixture Dynamic"
    );

    let (status, invoke_payload) = request_json(
        &app,
        Method::POST,
        "/providers/invoke-stream",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "input": {
                "contract_version": "1flowbase.provider/v2",
                "provider_instance_id": "instance-1",
                "provider_code": "fixture_provider",
                "protocol": "openai_compatible",
                "model": "fixture_dynamic",
                "messages": [
                    {
                        "role": "user",
                        "content": "hello",
                    }
                ]
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let events = invoke_payload["events"].as_array().unwrap();
    assert_eq!(events[0]["type"], "text_delta");
    assert_eq!(events[1]["type"], "tool_call_commit");
    assert_eq!(events[2]["type"], "mcp_call_commit");
    assert_eq!(events[3]["type"], "usage_snapshot");
    assert_eq!(events[4]["type"], "finish");
    assert!(invoke_payload.get("output_text").is_none());
    assert_eq!(invoke_payload["result"]["finish_reason"], "stop");
    assert_eq!(invoke_payload["result"]["usage"]["total_tokens"], 12);

    write_fixture_runtime(&package, "Reloaded Dynamic");

    let (status, _) = request_json(
        &app,
        Method::POST,
        "/providers/reload",
        json!({
            "plugin_id": load_payload["plugin_id"],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, reloaded_models_payload) = request_json(
        &app,
        Method::POST,
        "/providers/list-models",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "provider_config": {
                "api_key": "secret",
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let reloaded_models = reloaded_models_payload["models"].as_array().unwrap();
    assert_eq!(
        find_model(reloaded_models, "fixture_dynamic")["display_name"],
        "Reloaded Dynamic"
    );
}

#[tokio::test]
async fn provider_runner_exposes_balance() {
    let package = make_fixture_package();
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, balance_payload) = request_json(
        &app,
        Method::POST,
        "/providers/balance",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "provider_config": {
                "api_key": "secret",
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(balance_payload["balance"]["is_available"], true);
    assert_eq!(
        balance_payload["balance"]["balance_infos"][0]["currency"],
        "CNY"
    );
    assert_eq!(
        balance_payload["balance"]["balance_infos"][0]["total_balance"],
        "110.00"
    );
    assert_eq!(
        balance_payload["balance"]["provider_metadata"]["provider"],
        "deepseek"
    );
    assert!(!balance_payload.to_string().contains("secret"));
}

#[tokio::test]
async fn provider_runtime_routes_rejects_legacy_invoke_payload() {
    let package = make_fixture_package();
    write_legacy_invoke_runtime(&package);
    let app = app();

    let (status, load_payload) = request_json(
        &app,
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, payload) = request_json(
        &app,
        Method::POST,
        "/providers/invoke-stream",
        json!({
            "plugin_id": load_payload["plugin_id"],
            "input": {
                "contract_version": "1flowbase.provider/v2",
                "provider_instance_id": "instance-1",
                "provider_code": "fixture_provider",
                "protocol": "openai_compatible",
                "model": "fixture_dynamic",
                "messages": [
                    {
                        "role": "user",
                        "content": "hello",
                    }
                ]
            }
        }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(payload["message"]
        .as_str()
        .unwrap()
        .contains("invalid provider ndjson"));
}

#[tokio::test]
async fn provider_load_rejects_source_tree_root() {
    let package = make_fixture_package();
    package.write("demo/index.html", "<!doctype html>");
    package.write("scripts/demo.runner.example.json", "{}");

    let (status, payload) = request_json(
        &app(),
        Method::POST,
        "/providers/load",
        json!({
            "package_root": package.path(),
        }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(payload["message"].as_str().unwrap().contains("source tree"));
}
