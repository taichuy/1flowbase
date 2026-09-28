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
        while os.path.exists(root+'/delay_cancel'): time.sleep(.005)
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

pub(super) async fn wait_reaped(host: &ProviderHost, id: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if host.provider_worker_snapshot(id).unwrap().is_none() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        host.provider_worker_cleanup_receipt(id)
            .unwrap()
            .unwrap()
            .exited
    );
}

#[tokio::test]
async fn idle_worker_reaps_without_followup_traffic_and_cold_starts_new_pid() {
    let package = package();
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::from_millis(30));
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let first = host
        .invoke_stream(&id, invocation_input("fixture-model"))
        .await
        .unwrap();
    wait_reaped(&host, &id).await;
    let second = host
        .invoke_stream(&id, invocation_input("fixture-model"))
        .await
        .unwrap();
    assert_ne!(first.result.final_content, second.result.final_content);
    assert_eq!(
        host.provider_worker_snapshot(&id)
            .unwrap()
            .unwrap()
            .generation,
        2
    );
    host.stop_all().await.unwrap();
}

#[tokio::test]
async fn unreleased_transport_blocks_zero_grace_and_deselection_until_close() {
    let package = package();
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::ZERO);
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, input("owner", 7)).await.unwrap();
    host.reconcile_worker_demand(&id, 2, Some(false)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let original = host.provider_worker_snapshot(&id).unwrap().unwrap();
    assert_eq!(original.state, ProviderWorkerLifecycleState::Active);
    assert!(host
        .transport_worker_exit_evidence(&id, "owner", 7)
        .await
        .unwrap()
        .is_none());
    host.transport_session_operation(
        &id,
        ProviderTransportSessionCommand {
            logical_session_id: "owner".into(),
            generation: 7,
            worker_incarnation: None,
            action: ProviderTransportSessionAction::Close,
            deadline_unix_ms: now_unix_ms_for_test() + 2000,
        },
    )
    .unwrap()
    .await
    .unwrap();
    wait_reaped(&host, &id).await;
}

fn now_unix_ms_for_test() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test]
async fn activity_reservation_blocks_idle_even_when_demand_disappears() {
    let package = package();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::ZERO);
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let task = tokio::spawn(
        host.invoke_stream_with_live_events_operation(
            &id,
            invocation_input("fixture-model"),
            Some(tx),
            None,
        )
        .unwrap(),
    );
    entered(&mut rx).await;
    host.reconcile_worker_demand(&id, 10, Some(false)).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        host.provider_worker_snapshot(&id)
            .unwrap()
            .unwrap()
            .in_flight
            > 0
    );
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    task.await.unwrap().unwrap();
    wait_reaped(&host, &id).await;
}

#[tokio::test]
async fn stale_demand_revision_cannot_skip_current_grace() {
    let package = package();
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::from_millis(250));
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.reconcile_worker_demand(&id, 10, Some(true)).unwrap();
    host.reconcile_worker_demand(&id, 9, Some(false)).unwrap();
    host.invoke_stream(&id, invocation_input("fixture-model"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        host.provider_worker_snapshot(&id).unwrap().unwrap().state,
        ProviderWorkerLifecycleState::Active
    );
    wait_reaped(&host, &id).await;
}

#[tokio::test]
async fn cancelled_caller_retains_activity_until_carrier_acknowledges() {
    let package = package();
    fs::write(package.path().join("bin/delay_cancel"), b"delay").unwrap();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::ZERO);
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let (tx, mut rx) = tokio::sync::mpsc::channel(2);
    let task = tokio::spawn(
        host.invoke_stream_with_live_events_operation(
            &id,
            invocation_input("fixture-model"),
            Some(tx),
            None,
        )
        .unwrap(),
    );
    entered(&mut rx).await;
    let pid = host.provider_worker_snapshot(&id).unwrap().unwrap().pid;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let snapshot = host.provider_worker_snapshot(&id).unwrap().unwrap();
    assert_eq!(snapshot.pid, pid);
    assert_eq!(snapshot.state, ProviderWorkerLifecycleState::Active);
    assert!(snapshot.in_flight > 0);
    fs::remove_file(package.path().join("bin/delay_cancel")).unwrap();
    wait_reaped(&host, &id).await;
}

#[tokio::test]
async fn selection_reserves_before_zero_grace_timer_and_replacement_waits_for_reap() {
    let package = package();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::ZERO);
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let reserved = provider_worker_handle(
        &host.provider_workers,
        id.clone(),
        host.loaded_package(&id).unwrap(),
    )
    .unwrap();
    let original = reserved.worker.clone();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        original.snapshot().unwrap().state,
        ProviderWorkerLifecycleState::Active
    );
    assert!(original.snapshot().unwrap().in_flight > 0);
    original.begin_quiesce().unwrap();
    assert!(provider_worker_handle(
        &host.provider_workers,
        id.clone(),
        host.loaded_package(&id).unwrap()
    )
    .is_err());
    assert!(original.last_cleanup_receipt().unwrap().is_none());
    drop(reserved);
    super::super::session_workers::cleanup_batch(
        host.provider_workers.clone(),
        id.clone(),
        vec![original.clone()],
    )
    .await
    .unwrap();
    assert!(original.last_cleanup_receipt().unwrap().unwrap().exited);
    let successor = provider_worker_handle(
        &host.provider_workers,
        id.clone(),
        host.loaded_package(&id).unwrap(),
    )
    .unwrap();
    assert_eq!(
        successor.incarnation().unwrap(),
        original.incarnation().unwrap() + 1
    );
    assert_ne!(
        successor.snapshot().unwrap().pid,
        original.snapshot().unwrap().pid
    );
    drop(successor);
    wait_reaped(&host, &id).await;
}

#[tokio::test]
async fn spontaneous_idle_exit_observed_without_request_and_old_binding_stays_exact() {
    let package = package();
    let executable = package.path().join("bin/fixture_provider");
    let source = fs::read_to_string(&executable).unwrap();
    fs::write(
        &executable,
        source.replace(
            "for line in sys.stdin:",
            r#"def exit_watcher():
    while not os.path.exists(root+'/exit'): time.sleep(.005)
    os._exit(0)
threading.Thread(target=exit_watcher,daemon=True).start()
for line in sys.stdin:"#,
        ),
    )
    .unwrap();
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::from_secs(30));
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, input("original", 7)).await.unwrap();
    let original = host.provider_worker_snapshot(&id).unwrap().unwrap();
    assert!(host
        .transport_worker_exit_evidence(&id, "original", 7)
        .await
        .unwrap()
        .is_none());
    fs::write(package.path().join("bin/exit"), b"exit").unwrap();
    wait_reaped(&host, &id).await;
    let evidence = host
        .transport_worker_exit_evidence(&id, "original", 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        evidence.source,
        ProviderTransportClosureSource::ConfirmedWorkerExit
    );
    assert_eq!(evidence.identity.worker_incarnation, original.generation);
    fs::remove_file(package.path().join("bin/exit")).unwrap();
    host.invoke_stream(&id, input("successor", 1))
        .await
        .unwrap();
    let successor = host.provider_worker_snapshot(&id).unwrap().unwrap();
    assert_ne!(successor.pid, original.pid);
    assert_eq!(successor.generation, original.generation + 1);
    let late = host
        .transport_worker_exit_evidence(&id, "original", 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(late.identity, evidence.identity);
    assert!(host
        .transport_worker_exit_evidence(&id, "successor", 1)
        .await
        .unwrap()
        .is_none());
    host.stop_all().await.unwrap();
}

#[tokio::test]
async fn demand_notifications_do_not_extend_existing_idle_deadline() {
    let package = package();
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let mut host = ProviderHost::with_worker_idle_grace(Duration::from_millis(400));
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, invocation_input("fixture-model"))
        .await
        .unwrap();
    for revision in 1..=4 {
        tokio::time::sleep(Duration::from_millis(70)).await;
        host.reconcile_worker_demand(&id, revision, Some(true))
            .unwrap();
    }
    tokio::time::timeout(Duration::from_millis(200), wait_reaped(&host, &id))
        .await
        .unwrap();
}
