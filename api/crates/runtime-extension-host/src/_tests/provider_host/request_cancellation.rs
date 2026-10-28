use super::*;
use crate::{RuntimeArtifactResolver, RuntimeExtensionHost};
use runtime_core::runtime_backend::{
    ProviderRuntimePort, RuntimeArtifactReference, RuntimeBackendError, RuntimeCancelOutcome,
    RuntimeExecutionPort, RuntimeExecutionRequest, RuntimeObservationPort,
    RuntimePackageActivation, RuntimeRequestId, RuntimeStreamSinks, RuntimeTargetId,
};

struct Resolver(PathBuf);
#[async_trait::async_trait]
impl RuntimeArtifactResolver for Resolver {
    async fn resolve(&self, _: &RuntimeArtifactReference) -> Result<PathBuf, RuntimeBackendError> {
        Ok(self.0.clone())
    }
}

const WORKER: &str = r#"#!/usr/bin/env python3
import json, os, sys, threading, time
root = os.path.dirname(__file__)
output = threading.Lock()
jobs = {}
def mark(name, value='ready'):
    with open(os.path.join(root, name), 'w') as file: file.write(str(value))
def exists(name): return os.path.exists(os.path.join(root, name))
def emit(value):
    with output: print(json.dumps(value), flush=True)
def run(request, call_id=None, cancelled=None):
    name = request['input']['model']
    mark('pid-'+name, os.getpid())
    while not exists('release-'+name):
        if cancelled is not None and cancelled.is_set():
            mark('stopped-'+name)
            return
        time.sleep(.005)
    mark('finished-'+name)
    result = dict(final_content=name, finish_reason='stop')
    if call_id is None: emit(dict(type='result', result=result))
    else: emit(dict(protocol='stdio_json_multiplex_v1', kind='response', call_id=call_id, response=result))
for line in sys.stdin:
    frame = json.loads(line)
    if 'kind' not in frame: run(frame)
    elif frame['kind'] == 'call':
        stopped = threading.Event()
        task = threading.Thread(target=run, args=(frame['request'], frame['call_id'], stopped), daemon=True)
        jobs[frame['call_id']] = (stopped, task, frame['request']['input']['model'])
        task.start()
    elif frame['kind'] == 'cancel':
        stopped, task, name = jobs.pop(frame['call_id'])
        stopped.set()
        task.join()
        mark('cancel-seen-'+name)
        while not exists('allow-cancel-'+name): time.sleep(.005)
        emit(dict(protocol='stdio_json_multiplex_v1', kind='cancelled', call_id=frame['call_id']))
"#;

struct Fixture {
    package: TempProviderPackage,
    host: Arc<RuntimeExtensionHost>,
}
impl Fixture {
    async fn new(mode: &str) -> Self {
        let package = TempProviderPackage::new();
        if mode != "one-shot" {
            let manifest = fs::read_to_string(package.path().join("manifest.yaml")).unwrap();
            package.write(
                "manifest.yaml",
                &manifest
                    .replace(
                        "execution_mode: process_per_call",
                        "execution_mode: stateful_provider_worker",
                    )
                    .replace(
                        "protocol: stdio_json\n",
                        if mode == "multiplex" {
                            "protocol: stdio_json_multiplex_v1\n"
                        } else {
                            "protocol: stdio_json_worker\n"
                        },
                    ),
            );
        }
        crate::_tests::write_executable_fixture(
            &package.path().join("bin/fixture_provider"),
            WORKER.as_bytes(),
            0o755,
        );
        let host = Arc::new(
            RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                Arc::new(Resolver(package.path().to_path_buf())),
            )
            .unwrap(),
        );
        host.activate_provider(RuntimePackageActivation {
            plugin_id: "fixture_provider@0.1.0".into(),
            artifact: RuntimeArtifactReference::new("fixture").unwrap(),
            source_identity: None,
            legacy_eligibility: None,
        })
        .await
        .unwrap();
        host.mark_ready().unwrap();
        Self { package, host }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.package.path().join("bin").join(name)
    }
    fn release(&self, name: &str) {
        fs::write(self.path(name), b"release").unwrap();
    }
    async fn marker(&self, name: &str) -> String {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(value) = fs::read_to_string(self.path(name)) {
                    if !value.is_empty() {
                        return value;
                    }
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("fixture must reach the real carrier boundary")
    }
    fn start(
        &self,
        name: &str,
    ) -> tokio::task::JoinHandle<
        Result<runtime_core::runtime_backend::RuntimeExecutionOutcome, RuntimeBackendError>,
    > {
        let mut input = invocation_input(name);
        input.provider_instance_id = name.into();
        let request = RuntimeExecutionRequest {
            request_id: RuntimeRequestId::new(name).unwrap(),
            target: RuntimeTargetId::new("fixture_provider@0.1.0").unwrap(),
            input,
            principal: None,
        };
        let host = self.host.clone();
        tokio::spawn(async move {
            host.execute_stream(request, RuntimeStreamSinks::default())
                .await
        })
    }
    async fn cancel(&self, name: &str) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
        self.host
            .cancel(&RuntimeRequestId::new(name).unwrap())
            .await
    }
}

fn alive(pid: &str) -> bool {
    Path::new(&format!("/proc/{}", pid.trim())).exists()
}

#[tokio::test]
async fn communication_explicit_cancel_reaps_one_shot_after_waiter_drop_and_preserves_other_request(
) {
    let fixture = Fixture::new("one-shot").await;
    let a = fixture.start("a");
    let b = fixture.start("b");
    let a_pid = fixture.marker("pid-a").await;
    let b_pid = fixture.marker("pid-b").await;
    a.abort();
    assert!(a.await.unwrap_err().is_cancelled());
    assert!(
        alive(&a_pid),
        "ordinary waiter drop must not cancel business execution"
    );
    assert!(fixture
        .host
        .snapshot()
        .await
        .unwrap()
        .active_request_ids
        .contains(&"a".into()));

    assert_eq!(
        fixture.cancel("a").await.unwrap(),
        RuntimeCancelOutcome::Cancelled
    );
    assert!(
        !alive(&a_pid),
        "cancel success must already include actual child reap"
    );
    assert!(!fixture.path("release-a").exists());
    assert!(!fixture.path("finished-a").exists());
    assert!(
        alive(&b_pid),
        "another request's process is not a cancellation target"
    );
    fixture.release("release-b");
    assert_eq!(
        b.await.unwrap().unwrap().result.final_content.as_deref(),
        Some("b")
    );
    assert_eq!(
        fixture.cancel("a").await.unwrap(),
        RuntimeCancelOutcome::NotFound
    );
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn communication_multiplex_cancel_waits_for_call_ack_and_preserves_shared_peer() {
    let fixture = Fixture::new("multiplex").await;
    let a = fixture.start("a");
    let b = fixture.start("b");
    let pid = fixture.marker("pid-a").await;
    assert_eq!(pid, fixture.marker("pid-b").await);
    let host = fixture.host.clone();
    let mut cancel =
        tokio::spawn(async move { host.cancel(&RuntimeRequestId::new("a").unwrap()).await });
    fixture.marker("cancel-seen-a").await;
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut cancel)
            .await
            .is_err(),
        "no cancellation success before the carrier acknowledges resource release"
    );
    assert!(alive(&pid));
    fixture.release("allow-cancel-a");
    assert_eq!(
        cancel.await.unwrap().unwrap(),
        RuntimeCancelOutcome::Cancelled
    );
    assert!(matches!(
        a.await.unwrap(),
        Err(RuntimeBackendError::Cancelled(_))
    ));
    assert!(!b.is_finished());
    assert!(
        alive(&pid),
        "shared carrier must survive cancellation of one call"
    );
    fixture.release("release-b");
    assert_eq!(
        b.await.unwrap().unwrap().result.final_content.as_deref(),
        Some("b")
    );
    fixture.host.stop().await.unwrap();
}

#[tokio::test]
async fn communication_serial_cancel_reports_bounded_join_failure_without_killing_queued_peer() {
    let fixture = Fixture::new("serial").await;
    let a = fixture.start("a");
    let pid = fixture.marker("pid-a").await;
    let b = fixture.start("b");
    let error = tokio::time::timeout(Duration::from_secs(7), fixture.cancel("a"))
        .await
        .expect("explicit failure is bounded by the host shutdown budget")
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("could not join the owned carrier"));
    assert!(alive(&pid));
    assert!(!b.is_finished());
    assert!(
        !fixture.path("pid-b").exists(),
        "queued serial work must not be dispatched or killed"
    );
    assert!(fixture
        .host
        .snapshot()
        .await
        .unwrap()
        .active_request_ids
        .contains(&"a".into()));
    fixture.release("release-a");
    assert!(matches!(
        a.await.unwrap(),
        Err(RuntimeBackendError::Cancelled(_))
    ));
    assert_eq!(pid, fixture.marker("pid-b").await);
    fixture.release("release-b");
    assert_eq!(
        b.await.unwrap().unwrap().result.final_content.as_deref(),
        Some("b")
    );
    fixture.host.stop().await.unwrap();
}
