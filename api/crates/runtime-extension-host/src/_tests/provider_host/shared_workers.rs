use super::*;
use std::os::unix::fs::PermissionsExt;

fn package() -> TempProviderPackage {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package("shared_fixture", "shared_fixture", "Shared fixture");
    let manifest = fs::read_to_string(package.path().join("manifest.yaml")).unwrap();
    package.write(
        "manifest.yaml",
        &manifest.replace("stdio_json_worker", "stdio_json_multiplex_v1"),
    );
    package.write("bin/fixture_provider", r#"#!/usr/bin/env python3
import json, os, sys, threading, time
root = os.path.dirname(__file__)
lock = threading.Lock()
cancelled = set()
def emit(value):
    with lock:
        print(json.dumps(dict(protocol='stdio_json_multiplex_v1', **value)), flush=True)
def work(frame):
    call_id, request = frame['call_id'], frame['request']
    if request['method'] == 'transport_session':
        d = request['input']
        receipt = dict(generation=d['generation'], reused=False, physical_state='closed', connection_age_ms=0,
            ttl_remaining_ms=0, close_reason='requested_close', close_acknowledged=False,
            closure_evidence=dict(source='provider_local_release',local_released=True,peer_close_acknowledged=False,
                no_ack_reason='unknown',identity={k:d[k] for k in ['logical_session_id','generation','worker_incarnation']}))
        emit(dict(kind='response',call_id=call_id,response=dict(ok=True,result=receipt)))
        return
    d = request['input'].get('run_context',{}).get('physical_transport_session',{})
    with lock:
        with open(root+'/dispatches','a') as log:
            log.write(json.dumps(dict(pid=os.getpid(),session=d.get('logical_session_id'),incarnation=d.get('worker_incarnation')))+'\n')
    emit(dict(kind='event',call_id=call_id,event=dict(type='text_delta',delta='entered')))
    while not os.path.exists(root+'/release'):
        if call_id in cancelled: return
        time.sleep(.005)
    emit(dict(kind='response',call_id=call_id,response=dict(final_content=str(os.getpid()),finish_reason='stop')))
for line in sys.stdin:
    frame=json.loads(line)
    if frame['kind']=='call': threading.Thread(target=work,args=(frame,),daemon=True).start()
    elif frame['kind']=='cancel':
        cancelled.add(frame['call_id'])
        emit(dict(kind='cancelled',call_id=frame['call_id']))
"#);
    fs::set_permissions(
        package.path().join("bin/fixture_provider"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    package
}

fn input(session: &str, generation: u64) -> ProviderInvocationInput {
    let mut input = invocation_input("fixture-model");
    input
        .set_transport_session_directive(ProviderTransportSessionDirective {
            logical_session_id: session.into(),
            generation,
            worker_incarnation: None,
            task_id: format!("{session}-{generation}"),
            state: ProviderLogicalSessionState::Active,
            physical_deadline_unix_ms: 4_102_444_800_000,
        })
        .unwrap();
    input
}

async fn entered(events: &mut tokio::sync::mpsc::Receiver<ProviderStreamEvent>) {
    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(event, ProviderStreamEvent::TextDelta { delta } if delta == "entered"));
}

#[tokio::test]
async fn ten_sessions_enter_one_process_before_any_result_and_close_only_one_owner() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let mut running = Vec::new();
    for index in 0..10 {
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        running.push(tokio::spawn(
            host.invoke_stream_with_live_events_operation(
                &id,
                input(&format!("session-{index}"), 7),
                Some(tx),
                None,
            )
            .unwrap(),
        ));
        entered(&mut rx).await;
    }
    let rows: Vec<Value> = fs::read_to_string(package.path().join("bin/dispatches"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len(), 10);
    assert!(rows
        .iter()
        .all(|row| row["pid"] == rows[0]["pid"] && row["incarnation"] == rows[0]["incarnation"]));
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    for task in running {
        task.await.unwrap().unwrap();
    }
    let receipt = host
        .transport_session_operation(
            &id,
            ProviderTransportSessionCommand {
                logical_session_id: "session-0".into(),
                generation: 7,
                worker_incarnation: None,
                action: ProviderTransportSessionAction::Close,
                deadline_unix_ms: (SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    + 2000) as i64,
            },
        )
        .unwrap()
        .await
        .unwrap();
    assert!(receipt.closure_evidence.unwrap().local_released);
    assert!(host
        .invoke_stream(&id, input("session-0", 7))
        .await
        .is_err());
    let neighbour = host
        .invoke_stream(&id, input("session-1", 7))
        .await
        .unwrap();
    let successor = host
        .invoke_stream(&id, input("session-0", 8))
        .await
        .unwrap();
    assert_eq!(
        neighbour.result.final_content,
        successor.result.final_content
    );
    host.stop_all().await.unwrap();
}

#[tokio::test]
async fn same_logical_owner_queues_while_an_independent_owner_runs() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let first = tokio::spawn(
        host.invoke_stream_with_live_events_operation(&id, input("one", 7), Some(tx), None)
            .unwrap(),
    );
    entered(&mut rx).await;
    let duplicate = tokio::spawn(host.invoke_stream_operation(&id, input("one", 7)).unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let other = tokio::spawn(
        host.invoke_stream_with_live_events_operation(&id, input("two", 7), Some(tx), None)
            .unwrap(),
    );
    entered(&mut rx).await;
    assert_eq!(
        fs::read_to_string(package.path().join("bin/dispatches"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    duplicate.abort();
    let _ = duplicate.await;
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    first.await.unwrap().unwrap();
    other.await.unwrap().unwrap();
    assert_eq!(
        fs::read_to_string(package.path().join("bin/dispatches"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    host.stop_all().await.unwrap();
}
