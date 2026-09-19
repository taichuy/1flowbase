use super::*;
use extension_package_runtime::provider_contract::ProviderStdioMethod;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

struct UnaryFixture(PathBuf);
impl UnaryFixture {
    fn new(body: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "issue2085-unary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            format!("#!/usr/bin/env python3\nimport sys,json,time,os\n{body}\n"),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn supervisor(&self, timeout_ms: u64) -> Arc<ProviderWorkerSupervisor> {
        ProviderWorkerSupervisor::activate(
            self.0.clone(),
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
}
impl Drop for UnaryFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn request() -> ProviderStdioRequest {
    ProviderStdioRequest {
        method: ProviderStdioMethod::TransportSession,
        input: serde_json::json!({}),
    }
}

#[tokio::test]
async fn unary_control_rejections_preserve_same_worker_and_other_session_cursor() {
    let fixture = UnaryFixture::new(
        r#"
cursors={'session-b':'cursor-b'}
for index,line in enumerate(sys.stdin):
 if index < 3:
  message=['command expired','generation mismatch','connection absent'][index]
  print(json.dumps({'ok':False,'error':{'kind':'provider_transport_unavailable','message':message}}),flush=True)
 else:
  print(json.dumps({'ok':True,'result':{'cursor':cursors['session-b'],'pid':os.getpid()}}),flush=True)
"#,
    );
    let supervisor = fixture.supervisor(2_000);
    let before = supervisor.snapshot().unwrap();
    for reason in [
        "command expired",
        "generation mismatch",
        "connection absent",
    ] {
        let error = supervisor.call(&request()).await.unwrap_err();
        assert!(error.to_string().contains(reason), "{error}");
        let after = supervisor.snapshot().unwrap();
        assert_eq!(after.state, ProviderWorkerLifecycleState::Active);
        assert_eq!(after.pid, before.pid);
        assert_eq!(after.generation, before.generation);
        assert!(supervisor.last_cleanup_receipt().unwrap().is_none());
    }
    let resumed = supervisor.call(&request()).await.unwrap();
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
        let fixture = UnaryFixture::new(&format!(
            "for line in sys.stdin:\n print({},flush=True)",
            serde_json::to_string(response).unwrap()
        ));
        let supervisor = fixture.supervisor(2_000);
        assert!(supervisor.call(&request()).await.is_err());
        assert_eq!(
            supervisor.snapshot().unwrap().state,
            ProviderWorkerLifecycleState::Failed
        );
        assert!(supervisor.last_cleanup_receipt().unwrap().unwrap().exited);
    }
}

#[tokio::test]
async fn unary_eof_and_uncorrelated_late_response_do_not_reuse_unsynchronized_worker() {
    for body in ["sys.stdin.readline()\nsys.exit(0)", "sys.stdin.readline()\ntime.sleep(1)\nprint('{\"ok\":true,\"result\":\"late\"}',flush=True)"] {
        let fixture = UnaryFixture::new(body);
        let supervisor = fixture.supervisor(150);
        assert!(supervisor.call(&request()).await.is_err());
        assert_eq!(supervisor.snapshot().unwrap().state, ProviderWorkerLifecycleState::Failed);
        let cleanup = supervisor.last_cleanup_receipt().unwrap().unwrap();
        assert!(cleanup.exited);
        assert!(supervisor.call(&request()).await.is_err(), "retired worker must not consume a late response as the next result");
    }
}
