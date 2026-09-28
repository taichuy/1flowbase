use super::*;
use extension_package_runtime::provider_contract::ProviderStdioMethod;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

struct Fixture {
    root: PathBuf,
    supervisor: Arc<ProviderWorkerSupervisor>,
}
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("2153-serial-cancel-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/_fixtures/provider_stdio/cancellation_worker.py");
        let supervisor = ProviderWorkerSupervisor::activate(executable, Self::limits(), 1).unwrap();
        Self { root, supervisor }
    }
    fn limits() -> PluginRuntimeLimits {
        PluginRuntimeLimits {
            timeout_ms: Some(5_000),
            ..PluginRuntimeLimits::default()
        }
    }
    fn request(&self, method: ProviderStdioMethod, mode: &str) -> ProviderStdioRequest {
        ProviderStdioRequest {
            method,
            input: serde_json::json!({
                "mode": mode, "marker": self.root.join("dispatches"), "release": self.root.join("release")
            }),
        }
    }
    fn dispatched(&self) -> String {
        std::fs::read_to_string(self.root.join("dispatches")).unwrap_or_default()
    }
    async fn stop(&self) {
        self.supervisor.begin_quiesce().unwrap();
        assert!(
            self.supervisor
                .finish_quiesce(Duration::from_secs(1), ProviderWorkerCleanupReason::Drained)
                .await
                .unwrap()
                .exited
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn context() -> crate::stdio_runtime::StreamingCallContext {
    crate::stdio_runtime::StreamingCallContext {
        required_live_events: None,
        diagnostic_live_events: None,
        protocol_observation: None,
        event_observer: None,
        host_calls: None,
    }
}
struct Resource(Arc<AtomicBool>);
impl Drop for Resource {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn all_serial_queued_call_families_cancel_without_dispatch() {
    let fixture = Fixture::new();
    for family in 0..3 {
        let guard = fixture.supervisor.worker.lock().await;
        let supervisor = Arc::clone(&fixture.supervisor);
        let request = fixture.request(
            if family == 2 {
                ProviderStdioMethod::Invoke
            } else {
                ProviderStdioMethod::Validate
            },
            "cancelled",
        );
        let dropped = Arc::new(AtomicBool::new(false));
        let resource = Box::new(Resource(Arc::clone(&dropped)));
        let task = tokio::spawn(async move {
            match family {
                0 => {
                    let _ = supervisor.call_admitted(&request, resource).await;
                }
                1 => {
                    let _resource = resource;
                    let deadline = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        + 5_000) as i64;
                    let _ = supervisor
                        .call_with_deadline(&request, &Fixture::limits(), deadline)
                        .await;
                }
                _ => {
                    let _ = supervisor
                        .call_streaming_admitted(&request, &Fixture::limits(), context(), resource)
                        .await;
                }
            }
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while fixture.supervisor.snapshot().unwrap().in_flight != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(
            dropped.load(Ordering::SeqCst),
            "queued resource must release with its caller"
        );
        assert_eq!(fixture.supervisor.snapshot().unwrap().in_flight, 0);
        drop(guard);
        // A response barrier traverses the same serial carrier. Any detached
        // queued request would have written its marker before this result.
        fixture
            .supervisor
            .call(&fixture.request(ProviderStdioMethod::Validate, "barrier"))
            .await
            .unwrap();
        assert!(!fixture.dispatched().contains("cancelled"));
        assert_eq!(
            fixture.supervisor.snapshot().unwrap().state,
            ProviderWorkerLifecycleState::Active
        );
    }
    assert_eq!(fixture.dispatched().lines().count(), 3);
    fixture.stop().await;
}

#[tokio::test]
async fn dispatched_serial_caller_abort_preserves_carrier_and_resource_until_result() {
    let fixture = Fixture::new();
    let supervisor = Arc::clone(&fixture.supervisor);
    let request = fixture.request(ProviderStdioMethod::Invoke, "wait");
    let dropped = Arc::new(AtomicBool::new(false));
    let resource = Box::new(Resource(Arc::clone(&dropped)));
    let task = tokio::spawn(async move {
        supervisor
            .call_streaming_admitted(&request, &Fixture::limits(), context(), resource)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !fixture.dispatched().contains("wait") {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!dropped.load(Ordering::SeqCst));
    assert_eq!(fixture.supervisor.snapshot().unwrap().in_flight, 1);
    std::fs::write(fixture.root.join("release"), b"release").unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !dropped.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    fixture
        .supervisor
        .call(&fixture.request(ProviderStdioMethod::Validate, "barrier"))
        .await
        .unwrap();
    assert_eq!(fixture.dispatched(), "wait\nbarrier\n");
    assert_eq!(fixture.supervisor.snapshot().unwrap().in_flight, 0);
    fixture.stop().await;
}
