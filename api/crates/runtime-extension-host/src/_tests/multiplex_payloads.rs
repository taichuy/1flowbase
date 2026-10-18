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
