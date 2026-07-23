use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use plugin_framework::{
    error::PluginFrameworkError, provider_contract::ProviderStdioRequest, PluginRuntimeLimits,
};

fn write_script(name: &str, body: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("provider-stdio-v2-{name}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    let script = root.join("provider.sh");
    fs::write(&script, body).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
    }

    script
}

fn invoke_request() -> ProviderStdioRequest {
    ProviderStdioRequest {
        method: plugin_framework::provider_contract::ProviderStdioMethod::Invoke,
        input: serde_json::json!({ "model": "fixture" }),
    }
}

fn limits() -> PluginRuntimeLimits {
    PluginRuntimeLimits {
        timeout_ms: Some(2_000),
        invoke_timeout_ms: None,
        first_token_timeout_ms: None,
        stream_idle_timeout_ms: None,
        memory_bytes: None,
    }
}

fn default_limits() -> PluginRuntimeLimits {
    PluginRuntimeLimits {
        timeout_ms: None,
        invoke_timeout_ms: None,
        first_token_timeout_ms: None,
        stream_idle_timeout_ms: None,
        memory_bytes: None,
    }
}

#[tokio::test]
async fn provider_stdio_v2_reads_ndjson_stream_until_result() {
    let script = write_script(
        "success",
        r#"#!/usr/bin/env bash
read _request
printf '%s\n' '{"type":"text_delta","delta":"hel"}'
printf '%s\n' '{"type":"text_delta","delta":"lo"}'
printf '%s\n' '{"type":"usage_snapshot","usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3}}'
printf '%s\n' '{"type":"finish","reason":"stop"}'
printf '%s\n' '{"type":"result","result":{"final_content":"hello","usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3},"finish_reason":"stop"}}'
"#,
    );

    let output = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &limits(),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(output.events.len(), 4);
    assert_eq!(output.result.final_content.as_deref(), Some("hello"));
}

#[tokio::test]
async fn provider_stdio_default_invocation_budget_is_300_seconds() {
    assert_eq!(
        plugin_runner::stdio_runtime::DEFAULT_PROVIDER_INVOCATION_TIMEOUT_MS,
        300_000
    );

    let script = write_script(
        "default-budget",
        r#"#!/usr/bin/env bash
read _request
printf '%s\n' '{"type":"result","result":{"final_content":"within-default-budget","finish_reason":"stop"}}'
"#,
    );

    let output = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &default_limits(),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        output.result.final_content.as_deref(),
        Some("within-default-budget")
    );
}

#[tokio::test]
async fn provider_stdio_timeout_error_classifies_wall_clock_budget() {
    let script = write_script(
        "wall-clock-timeout",
        r#"#!/usr/bin/env bash
read _request
sleep 0.5
printf '%s\n' '{"type":"result","result":{"final_content":"too-late","finish_reason":"stop"}}'
"#,
    );
    let short_limits = PluginRuntimeLimits {
        timeout_ms: Some(100),
        invoke_timeout_ms: None,
        first_token_timeout_ms: None,
        stream_idle_timeout_ms: None,
        memory_bytes: None,
    };

    let error = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &short_limits,
        None,
        None,
    )
    .await
    .unwrap_err();

    let PluginFrameworkError::RuntimeContract { error } = error else {
        panic!("expected provider runtime contract error");
    };
    assert!(error.message.contains("provider runtime timed out"));
    assert!(error
        .provider_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("timeout_kind=wall_clock")));
}

#[tokio::test]
async fn provider_stdio_timeout_error_classifies_first_token_budget() {
    let script = write_script(
        "first-token-timeout",
        r#"#!/usr/bin/env bash
read _request
sleep 0.5
printf '%s\n' '{"type":"text_delta","delta":"late"}'
printf '%s\n' '{"type":"result","result":{"final_content":"late","finish_reason":"stop"}}'
"#,
    );
    let short_limits = PluginRuntimeLimits {
        timeout_ms: Some(2_000),
        invoke_timeout_ms: None,
        first_token_timeout_ms: Some(100),
        stream_idle_timeout_ms: None,
        memory_bytes: None,
    };

    let error = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &short_limits,
        None,
        None,
    )
    .await
    .unwrap_err();

    let PluginFrameworkError::RuntimeContract { error } = error else {
        panic!("expected provider runtime contract error");
    };
    assert!(error
        .provider_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("timeout_kind=first_token")));
}

#[tokio::test]
async fn provider_stdio_timeout_error_classifies_stream_idle_budget() {
    let script = write_script(
        "stream-idle-timeout",
        r#"#!/usr/bin/env bash
read _request
printf '%s\n' '{"type":"text_delta","delta":"first"}'
sleep 0.5
printf '%s\n' '{"type":"result","result":{"final_content":"first","finish_reason":"stop"}}'
"#,
    );
    let short_limits = PluginRuntimeLimits {
        timeout_ms: Some(2_000),
        invoke_timeout_ms: None,
        first_token_timeout_ms: None,
        stream_idle_timeout_ms: Some(100),
        memory_bytes: None,
    };

    let error = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &short_limits,
        None,
        None,
    )
    .await
    .unwrap_err();

    let PluginFrameworkError::RuntimeContract { error } = error else {
        panic!("expected provider runtime contract error");
    };
    assert!(error
        .provider_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("timeout_kind=stream_idle")));
}

#[tokio::test]
async fn provider_stdio_v2_rejects_bad_json_line() {
    let script = write_script(
        "bad-json",
        r#"#!/usr/bin/env bash
read _request
printf '%s\n' '{not-json'
"#,
    );

    let error = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &limits(),
        None,
        None,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("invalid provider ndjson"));
}

#[tokio::test]
async fn provider_worker_stdio_reuses_process_across_streaming_invocations() {
    let script = write_script(
        "worker-reuse",
        r#"#!/usr/bin/env bash
set -euo pipefail
count=0
while IFS= read -r _request; do
  count=$((count + 1))
  printf '%s\n' "{\"type\":\"text_delta\",\"delta\":\"turn-${count}\"}"
  printf '%s\n' "{\"type\":\"result\",\"result\":{\"final_content\":\"turn-${count}\",\"finish_reason\":\"stop\"}}"
done
"#,
    );
    let mut worker = plugin_runner::stdio_runtime::ProviderWorker::new(script, limits());

    let first = worker
        .call_streaming(&invoke_request(), None, None)
        .await
        .expect("first worker invoke should succeed");
    let second = worker
        .call_streaming(&invoke_request(), None, None)
        .await
        .expect("second worker invoke should reuse process");

    assert_eq!(first.result.final_content.as_deref(), Some("turn-1"));
    assert_eq!(second.result.final_content.as_deref(), Some("turn-2"));
}
