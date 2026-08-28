use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use extension_contracts::{
    PluginDataBinding, PluginDataOperationResult, PluginDataPermission, PluginDataPort,
    PluginDataRequest, PluginDataResponse,
};
use extension_package_runtime::{
    provider_contract::{ProviderStdioMethod, ProviderStdioRequest},
    PluginRuntimeLimits,
};

use crate::stdio_runtime::{ProviderHostCallContext, ProviderWorker};

#[derive(Default)]
struct CapturingPluginDataPort {
    bindings: tokio::sync::Mutex<Vec<PluginDataBinding>>,
    completed: AtomicUsize,
}

impl PluginDataPort for CapturingPluginDataPort {
    fn execute<'a>(
        &'a self,
        binding: &'a PluginDataBinding,
        _request: &'a PluginDataRequest,
    ) -> extension_contracts::PluginDataFuture<'a> {
        Box::pin(async move {
            self.bindings.lock().await.push(binding.clone());
            tokio::time::sleep(Duration::from_millis(20)).await;
            self.completed.fetch_add(1, Ordering::Relaxed);
            Ok(PluginDataResponse {
                results: vec![PluginDataOperationResult::Count { count: 1 }],
                replayed: false,
            })
        })
    }
}

fn host_call_worker() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/_fixtures/provider_stdio/host_call_worker.sh")
}

fn now_unix_ms() -> i64 {
    i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .unwrap_or(i64::MAX)
}

fn host_call_context(port: Arc<CapturingPluginDataPort>) -> ProviderHostCallContext {
    ProviderHostCallContext {
        binding: PluginDataBinding {
            publisher_namespace: "trusted".to_string(),
            plugin_code: "fixture".to_string(),
            plugin_version: "1.0.0".to_string(),
            storage_binding: "main".to_string(),
            workspace_id: "00000000-0000-0000-0000-000000000001".to_string(),
            actor_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
            provider_instance_id: "provider-1".to_string(),
            permissions: BTreeSet::from([PluginDataPermission::Read, PluginDataPermission::Write]),
            deadline_unix_ms: now_unix_ms() + 10_000,
        },
        plugin_data: port,
    }
}

fn host_call_request(mode: &str) -> ProviderStdioRequest {
    ProviderStdioRequest {
        method: ProviderStdioMethod::Invoke,
        input: serde_json::json!({ "mode": mode }),
    }
}

#[tokio::test]
async fn pdp_003_009_host_calls_use_trusted_binding_and_correlated_results() {
    let port = Arc::new(CapturingPluginDataPort::default());
    let mut worker = ProviderWorker::new(host_call_worker(), PluginRuntimeLimits::default());
    let output = worker
        .call_streaming_with_limits_and_host_calls(
            &host_call_request("normal"),
            &PluginRuntimeLimits {
                timeout_ms: Some(2_000),
                ..Default::default()
            },
            None,
            None,
            None,
            Some(host_call_context(Arc::clone(&port))),
        )
        .await
        .unwrap();
    assert_eq!(output.result.final_content.as_deref(), Some("host-call-ok"));
    let bindings = port.bindings.lock().await;
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].publisher_namespace, "trusted");
    assert_eq!(
        bindings[0].workspace_id,
        "00000000-0000-0000-0000-000000000001"
    );
}

#[tokio::test]
async fn pdp_009_duplicate_and_unknown_call_ids_fail_closed() {
    for mode in ["duplicate", "unknown_cancel"] {
        let port = Arc::new(CapturingPluginDataPort::default());
        let mut worker = ProviderWorker::new(host_call_worker(), PluginRuntimeLimits::default());
        let error = worker
            .call_streaming_with_limits_and_host_calls(
                &host_call_request(mode),
                &PluginRuntimeLimits {
                    timeout_ms: Some(2_000),
                    ..Default::default()
                },
                None,
                None,
                None,
                Some(host_call_context(port)),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("host call id"), "{error}");
    }
}

#[tokio::test]
async fn pdp_008_cancel_deadline_and_worker_crash_clear_active_host_calls() {
    for mode in ["cancel", "crash"] {
        let port = Arc::new(CapturingPluginDataPort::default());
        let mut worker = ProviderWorker::new(host_call_worker(), PluginRuntimeLimits::default());
        let result = worker
            .call_streaming_with_limits_and_host_calls(
                &host_call_request(mode),
                &PluginRuntimeLimits {
                    timeout_ms: Some(2_000),
                    ..Default::default()
                },
                None,
                None,
                None,
                Some(host_call_context(Arc::clone(&port))),
            )
            .await;
        assert_eq!(result.is_ok(), mode == "cancel");
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(port.completed.load(Ordering::Relaxed), 0, "{mode}");
    }

    let port = Arc::new(CapturingPluginDataPort::default());
    let mut context = host_call_context(Arc::clone(&port));
    context.binding.deadline_unix_ms = now_unix_ms() - 1;
    let mut worker = ProviderWorker::new(host_call_worker(), PluginRuntimeLimits::default());
    assert!(worker
        .call_streaming_with_limits_and_host_calls(
            &host_call_request("normal"),
            &PluginRuntimeLimits {
                timeout_ms: Some(2_000),
                ..Default::default()
            },
            None,
            None,
            None,
            Some(context),
        )
        .await
        .is_ok());
    assert!(port.bindings.lock().await.is_empty());
}
