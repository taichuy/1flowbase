use std::path::PathBuf;

use plugin_framework::{provider_contract::ProviderStdioRequest, PluginRuntimeLimits};

fn fixture_script(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/_fixtures/provider_stdio")
        .join(name)
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
    let script = fixture_script("success.sh");

    let output = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &limits(),
        None,
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

    let script = fixture_script("default_budget.sh");

    let output = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &default_limits(),
        None,
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
async fn provider_stdio_v2_rejects_bad_json_line() {
    let script = fixture_script("bad_json.sh");

    let error = plugin_runner::stdio_runtime::call_executable_streaming(
        &script,
        &invoke_request(),
        &limits(),
        None,
        None,
        None,
    )
    .await
    .unwrap_err();

    assert!(error.to_string().contains("invalid provider ndjson"));
}

#[tokio::test]
async fn provider_worker_stdio_reuses_process_across_streaming_invocations() {
    let script = fixture_script("worker_reuse.sh");
    let mut worker = plugin_runner::stdio_runtime::ProviderWorker::new(script, limits());

    let first = worker
        .call_streaming(&invoke_request(), None, None, None)
        .await
        .expect("first worker invoke should succeed");
    let second = worker
        .call_streaming(&invoke_request(), None, None, None)
        .await
        .expect("second worker invoke should reuse process");

    assert_eq!(first.result.final_content.as_deref(), Some("turn-1"));
    assert_eq!(second.result.final_content.as_deref(), Some("turn-2"));
}
