use super::*;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Fixture, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "multiplex-payload-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let executable = root.join("worker.py");
    std::fs::write(&executable, r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    frame = json.loads(line)
    cid = frame['call_id']
    def send(kind, **fields):
        print(json.dumps(dict(protocol='stdio_json_multiplex_v1',kind=kind,call_id=cid,**fields),ensure_ascii=False),flush=True)
    if frame['kind'] == 'cancel':
        send('cancelled')
        continue
    request = frame['request']
    if request.get('events'):
        for index in range(40):
            send('event',event=dict(type='text_delta',delta=str(index)+':'+'x'*(2*1024*1024)))
    send('response',response=request.get('payload', {'done':True}))
"#).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    (Fixture(root), executable)
}

#[tokio::test]
async fn multiplex_large_request_result_and_cumulative_events_survive_real_stdio() {
    let (_fixture, executable) = fixture();
    let limits = PluginRuntimeLimits {
        timeout_ms: Some(120_000),
        ..Default::default()
    };
    let worker =
        Arc::new(MultiplexProviderWorker::activate(executable, limits.clone(), 1).unwrap());
    let task_worker = worker.clone();
    let result = tokio::spawn(async move {
        let worker = task_worker;
        tokio::time::timeout(Duration::from_secs(120), async {
            let payload = format!("{}完整🙂", "x".repeat(64 * 1024 * 1024 + 1));
            let (response, events) = worker
                .call_json(json!({"payload":payload}), &limits, None, None)
                .await
                .unwrap();
            assert!(events.is_empty());
            assert_eq!(response.as_str(), Some(payload.as_str()));
            drop(response);
            drop(payload);
            let (response, events) = worker
                .call_json(json!({"events":true}), &limits, None, None)
                .await
                .unwrap();
            assert_eq!(response, json!({"done":true}));
            assert_eq!(events.len(), 40);
            for (index, event) in events.iter().enumerate() {
                let ProviderStreamEvent::TextDelta { delta } = event else {
                    panic!("unexpected event");
                };
                let prefix = format!("{index}:");
                assert!(delta.starts_with(&prefix));
                assert_eq!(delta.len(), prefix.len() + 2 * 1024 * 1024);
            }
            let (neighbor, _) = worker
                .call_json(json!({"payload":"neighbor"}), &limits, None, None)
                .await
                .unwrap();
            assert_eq!(neighbor, "neighbor");
        })
        .await
    })
    .await;
    let cleanup = worker.process_control().terminate().await;
    assert!(cleanup.exited);
    result.unwrap().unwrap();
}

const LARGE_CALLBACK_BYTES: usize = 64 * 1024 * 1024 + 1;

struct LargeCredential;
impl extension_contracts::PluginCredentialPort for LargeCredential {
    fn execute<'a>(
        &'a self,
        _: &'a extension_contracts::PluginCredentialBinding,
        _: &'a extension_contracts::PluginCredentialRequest,
    ) -> extension_contracts::PluginCredentialFuture<'a> {
        Box::pin(async {
            Ok(extension_contracts::PluginCredentialResponse {
                value: Some(json!(format!("{}完整🙂", "x".repeat(LARGE_CALLBACK_BYTES)))),
            })
        })
    }
}

// Drive the real reader actor with real child stdout, but retain the transport
// queues. Intercepting CallbackDone lets the test order the saturated callback
// before a registration fence without relying on sleeps or scheduler timing.
struct CallbackReader {
    _fixture: Fixture,
    stdin: ChildStdin,
    commands: mpsc::Sender<CommandMessage>,
    callback_commands: mpsc::Receiver<CommandMessage>,
    writes: mpsc::UnboundedReceiver<WriteFrame>,
    controls: mpsc::Receiver<ControlFrame>,
    budget: Arc<Semaphore>,
    control: ProviderWorkerProcessControl,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for CallbackReader {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl CallbackReader {
    fn new() -> Self {
        let (fixture, executable) = fixture();
        std::fs::write(
            &executable,
            "#!/usr/bin/env python3\nimport sys\nfor line in sys.stdin:\n    sys.stdout.write(line)\n    sys.stdout.flush()\n",
        )
        .unwrap();
        let process = spawn_worker_process(&executable, &PluginRuntimeLimits::default()).unwrap();
        let (commands, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (callback_sender, callback_commands) = mpsc::channel(QUEUE_CAPACITY);
        let (writes, write_receiver) = mpsc::unbounded_channel();
        let (controls, control_receiver) = mpsc::channel(QUEUE_CAPACITY);
        let budget = Arc::new(Semaphore::new(WRITER_BYTES));
        let control = process.control;
        let reader = tokio::spawn(run_reader(
            process.stdout.into_inner().into_inner(),
            receiver,
            ReaderContext {
                retention_enabled: false,
                retained_resources: Arc::new(AtomicU64::new(0)),
                command_sender: callback_sender,
                writes,
                controls,
                writer_budget: budget.clone(),
                control: control.clone(),
                failed: Arc::new(AtomicBool::new(false)),
            },
        ));
        Self {
            _fixture: fixture,
            stdin: process.stdin,
            commands,
            callback_commands,
            writes: write_receiver,
            controls: control_receiver,
            budget,
            control,
            reader,
        }
    }

    async fn register(
        &mut self,
        permit: OwnedSemaphorePermit,
        context: Option<ProviderHostCallContext>,
    ) -> (String, mpsc::UnboundedReceiver<Delivered>) {
        let (events, receiver) = mpsc::unbounded_channel();
        let (registered, registration) = oneshot::channel();
        self.commands
            .send(CommandMessage::Call {
                request: json!({}),
                events,
                host_calls: context.map(Box::new),
                registered,
                permit,
                lease: None,
            })
            .await
            .unwrap();
        let id = registration.await.unwrap();
        let written = self.writes.recv().await.unwrap();
        assert_eq!(written.call_id.as_deref(), Some(id.as_str()));
        drop(written);
        self.commands
            .send(CommandMessage::Written(id.clone()))
            .await
            .unwrap();
        (id, receiver)
    }

    async fn assert_writer_recovered(&mut self) {
        // An un-aborted FIFO waiter would acquire the window first and retain
        // it in the undrained control queue, preventing this acquisition.
        let recovered = tokio::time::timeout(
            Duration::from_secs(5),
            self.budget.clone().acquire_many_owned(WRITER_BYTES as u32),
        )
        .await
        .expect("call cleanup must release the pending callback writer")
        .unwrap();
        assert!(
            self.controls.try_recv().is_err(),
            "stale callback result survived call cleanup"
        );
        drop(recovered);
    }

    async fn emit(&mut self, frame: Value) {
        let mut bytes = serde_json::to_vec(&frame).unwrap();
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await.unwrap();
        self.stdin.flush().await.unwrap();
    }
}

async fn callback_wait_case(cancel: bool, terminal_drop: bool) {
    let mut fixture = CallbackReader::new();
    let control = fixture.control.clone();
    let result = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(120), async {
            let mut context = credential_tests::context(Arc::new(credential_tests::Credentials(
                std::sync::Mutex::new(vec![]),
            )));
            let (binding, port) = context.plugin_credentials.as_mut().unwrap();
            binding.deadline_unix_ms = i64::MAX;
            *port = Arc::new(LargeCredential);
            let permit = fixture.budget.clone().acquire_owned().await.unwrap();
            let (id, mut events) = fixture.register(permit, Some(context)).await;
            // Reserve the neighbour admission before saturating the window.
            let neighbor_permit = fixture.budget.clone().acquire_owned().await.unwrap();
            let mut saturation = Some(fixture
                .budget
                .clone()
                .acquire_many_owned((WRITER_BYTES - 1) as u32)
                .await
                .unwrap());
            fixture
                .emit(json!({"protocol":"stdio_json_multiplex_v1", "kind":"callback",
                    "call_id":id, "callback_id":"1", "service":"plugin_credential_v1",
                    "request":{"operation":"get", "credential_id":"large"}}))
                .await;
            let done = fixture.callback_commands.recv().await.unwrap();
            assert!(matches!(&done, CommandMessage::CallbackDone { response, .. }
                if response["result"]["value"].as_str().unwrap().len() > LARGE_CALLBACK_BYTES));
            fixture.commands.send(done).await.unwrap();
            // FIFO registration is a fence: CallbackDone has been handled, and
            // its full-window permit cannot be granted while saturation lives.
            let (neighbor, mut neighbor_events) = fixture.register(neighbor_permit, None).await;
            // The neighbour frame released one byte. The full-window callback
            // waiter must reserve it before cleanup is exercised, so even a
            // callback task which has not yet been polled cannot escape detection.
            tokio::time::timeout(Duration::from_secs(5), async {
                while fixture.budget.available_permits() != 0 {
                    tokio::task::yield_now().await;
                }
            }).await.expect("callback must be waiting on the writer window");
            assert!(fixture.controls.try_recv().is_err());
            fixture
                .emit(json!({"protocol":"stdio_json_multiplex_v1", "kind":"response",
                    "call_id":neighbor, "response":"reader-progress"}))
                .await;
            assert!(matches!(neighbor_events.recv().await.unwrap().message.unwrap(),
                MultiplexWorkerMessage::Response { response, .. } if response == "reader-progress"));

            if cancel {
                fixture.commands.send(CommandMessage::Cancel { id: id.clone() }).await.unwrap();
                let cancelled = fixture.controls.recv().await.unwrap();
                let frame: MultiplexEnvelope<MultiplexHostMessage> =
                    serde_json::from_slice(&cancelled.bytes).unwrap();
                assert!(matches!(frame.message, MultiplexHostMessage::Cancel { call_id } if call_id == id));
                drop(cancelled);
                // Check cancellation cleanup before the worker acknowledges it;
                // otherwise ActiveCall::drop could hide a broken request_cancel.
                drop(saturation.take());
                fixture.assert_writer_recovered().await;
                fixture.emit(json!({"protocol":"stdio_json_multiplex_v1", "kind":"cancelled", "call_id":id})).await;
            } else if terminal_drop {
                // A terminal worker result removes ActiveCall and must abort
                // the call-owned callback writer even without explicit cancel.
                fixture.emit(json!({"protocol":"stdio_json_multiplex_v1", "kind":"response", "call_id":id, "response":"early-terminal"})).await;
            } else {
                drop(saturation.take());
                let delivered = fixture.controls.recv().await.unwrap();
                assert!(delivered.bytes.len() > LARGE_CALLBACK_BYTES);
                let frame: MultiplexEnvelope<MultiplexHostMessage> =
                    serde_json::from_slice(&delivered.bytes).unwrap();
                let MultiplexHostMessage::CallbackResult { call_id, callback_id, response } = frame.message else {
                    panic!("expected callback result");
                };
                assert_eq!(call_id, id);
                assert_eq!(callback_id, "1");
                let value = response["result"]["value"].as_str().unwrap();
                assert_eq!(value.len(), LARGE_CALLBACK_BYTES + "完整🙂".len());
                assert!(value.as_bytes()[..LARGE_CALLBACK_BYTES].iter().all(|byte| *byte == b'x'));
                assert!(value.ends_with("完整🙂"));
                drop(delivered);
                fixture.emit(json!({"protocol":"stdio_json_multiplex_v1", "kind":"response", "call_id":id, "response":"callback-delivered"})).await;
                assert!(matches!(events.recv().await.unwrap().message.unwrap(),
                    MultiplexWorkerMessage::Response { response, .. } if response == "callback-delivered"));
                return;
            }
            let terminal = events.recv().await.unwrap().message.unwrap();
            assert!(if cancel {
                matches!(terminal, MultiplexWorkerMessage::Cancelled { .. })
            } else {
                matches!(terminal, MultiplexWorkerMessage::Response { response, .. } if response == "early-terminal")
            });
            if !cancel {
                drop(saturation.take());
                fixture.assert_writer_recovered().await;
            }
            let permit = fixture.budget.clone().acquire_owned().await.unwrap();
            let (neighbor, mut events) = fixture.register(permit, None).await;
            fixture.emit(json!({"protocol":"stdio_json_multiplex_v1", "kind":"response", "call_id":neighbor, "response":"after-cleanup"})).await;
            assert!(matches!(events.recv().await.unwrap().message.unwrap(),
                MultiplexWorkerMessage::Response { response, .. } if response == "after-cleanup"));
        })
        .await
    })
    .await;
    let cleanup = control.terminate().await;
    assert!(cleanup.exited);
    result.unwrap().unwrap();
}

#[tokio::test]
async fn large_callback_wait_preserves_reader_progress_and_delivers_complete_result() {
    callback_wait_case(false, false).await;
}

#[tokio::test]
async fn cancelled_large_callback_wait_releases_writer_and_preserves_neighbor() {
    callback_wait_case(true, false).await;
}

#[tokio::test]
async fn terminal_call_drop_releases_large_callback_wait_and_preserves_neighbor() {
    callback_wait_case(false, true).await;
}
