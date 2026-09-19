use super::*;
use extension_package_runtime::provider_contract::ProviderStdioMethod;
use std::path::PathBuf;

fn supervisor(timeout_ms: u64) -> Arc<ProviderWorkerSupervisor> {
    ProviderWorkerSupervisor::activate(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/_fixtures/provider_stdio/unary_worker.py"),
        PluginRuntimeLimits {
            timeout_ms: Some(timeout_ms),
            invoke_timeout_ms: Some(timeout_ms),
            first_token_timeout_ms: None,
            stream_idle_timeout_ms: None,
            memory_bytes: None,
        },
        17,
    )
    .unwrap()
}
fn request(mode: &str) -> ProviderStdioRequest {
    ProviderStdioRequest {
        method: ProviderStdioMethod::TransportSession,
        input: serde_json::json!({"mode": mode}),
    }
}

#[tokio::test]
async fn unary_control_rejections_preserve_same_worker_and_other_session_cursor() {
    let supervisor = supervisor(2_000);
    let before = supervisor.snapshot().unwrap();
    for reason in [
        "command expired",
        "generation mismatch",
        "connection absent",
    ] {
        let error = supervisor.call(&request("rejections")).await.unwrap_err();
        assert!(error.to_string().contains(reason), "{error}");
        let after = supervisor.snapshot().unwrap();
        assert_eq!(after.state, ProviderWorkerLifecycleState::Active);
        assert_eq!(after.pid, before.pid);
        assert_eq!(after.generation, before.generation);
        assert!(supervisor.last_cleanup_receipt().unwrap().is_none());
    }
    let resumed = supervisor.call(&request("rejections")).await.unwrap();
    assert_eq!(resumed["cursor"], "cursor-b");
    assert_eq!(resumed["pid"], before.pid.unwrap());
    supervisor.begin_quiesce().unwrap();
    let cleanup = supervisor
        .finish_quiesce(Duration::from_secs(1), ProviderWorkerCleanupReason::Drained)
        .await
        .unwrap();
    assert!(cleanup.exited);
}

#[tokio::test]
async fn unary_broken_or_inconsistent_frames_retire_worker_with_exit_evidence() {
    for response in [
        "not-json",
        r#"{"ok":false}"#,
        r#"{"ok":true,"error":{"kind":"provider_transport_unavailable","message":"bad"}}"#,
    ] {
        let supervisor = supervisor(2_000);
        let mut input = request("raw");
        input.input["response"] = serde_json::json!(response);
        assert!(supervisor.call(&input).await.is_err());
        assert_eq!(
            supervisor.snapshot().unwrap().state,
            ProviderWorkerLifecycleState::Failed
        );
        assert!(supervisor.last_cleanup_receipt().unwrap().unwrap().exited);
    }
}

#[tokio::test]
async fn unary_eof_and_uncorrelated_late_response_do_not_reuse_unsynchronized_worker() {
    for mode in ["eof", "late"] {
        let supervisor = supervisor(150);
        assert!(supervisor.call(&request(mode)).await.is_err());
        assert_eq!(
            supervisor.snapshot().unwrap().state,
            ProviderWorkerLifecycleState::Failed
        );
        let cleanup = supervisor.last_cleanup_receipt().unwrap().unwrap();
        assert!(cleanup.exited);
        assert!(
            supervisor.call(&request(mode)).await.is_err(),
            "retired worker must not consume a late response as the next result"
        );
    }
}
