use super::*;
use std::os::unix::fs::PermissionsExt;

fn package() -> TempProviderPackage {
    named_package("fixture_provider")
}

fn named_package(plugin_id: &str) -> TempProviderPackage {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package(plugin_id, plugin_id, "Fixture");
    package.write("bin/fixture_provider", r#"#!/usr/bin/env python3
import json, os, sys, time
root = os.path.dirname(__file__)
def emit(value):
    print(json.dumps(value), flush=True)
for line in sys.stdin:
    request = json.loads(line)
    if request['method'] == 'invoke':
        d = (request['input'].get('run_context') or {}).get('physical_transport_session', {'task_id':'idle'})
        with open(root + '/dispatches', 'a') as log:
            log.write(json.dumps({'pid': os.getpid(), 'identity': d}) + '\n')
        if d['task_id'] == 'blocked':
            emit({'type':'text_delta','delta':'entered'})
            while not os.path.exists(root + '/release'):
                time.sleep(.01)
        emit({'type':'result','result':{'final_content':str(os.getpid()),'finish_reason':'stop'}})
    elif request['method'] == 'transport_session':
        d = request['input']
        with open(root + '/controls', 'a') as log:
            log.write(str(os.getpid()) + '\n')
        emit({'ok':True,'result':{'generation':d['generation'],'reused':False,
            'physical_state':'closed','connection_age_ms':0,'ttl_remaining_ms':0,
            'close_reason':'requested_close','close_acknowledged':False,
            'closure_evidence':{'source':'provider_local_release','local_released':True,
                'peer_close_acknowledged':False,'no_ack_reason':'unknown',
                'identity':{k:d[k] for k in ['logical_session_id','generation','worker_incarnation']}}}})
    else:
        sys.exit(7)
"#);
    fs::set_permissions(
        package.path().join("bin/fixture_provider"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    package
}

fn input(session: &str, blocked: bool) -> ProviderInvocationInput {
    let mut input = invocation_input("fixture-model");
    input
        .set_transport_session_directive(ProviderTransportSessionDirective {
            logical_session_id: session.into(),
            generation: 7,
            worker_incarnation: None,
            task_id: if blocked { "blocked" } else { "idle" }.into(),
            state: ProviderLogicalSessionState::Active,
            physical_deadline_unix_ms: 4_102_444_800_000,
        })
        .unwrap();
    input
}

fn close(session: &str) -> ProviderTransportSessionCommand {
    ProviderTransportSessionCommand {
        logical_session_id: session.into(),
        generation: 7,
        worker_incarnation: None,
        action: ProviderTransportSessionAction::Close,
        deadline_unix_ms: (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            + 500) as i64,
    }
}

async fn entered(events: &mut tokio::sync::mpsc::Receiver<ProviderStreamEvent>) {
    let event = tokio::time::timeout(Duration::from_secs(3), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(event, ProviderStreamEvent::TextDelta { delta } if delta == "entered"));
}

fn dispatches(package: &TempProviderPackage) -> Vec<Value> {
    fs::read_to_string(package.path().join("bin/dispatches"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

async fn empty_workers(host: &ProviderHost) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if lock_provider_worker_registry(&host.provider_workers)
                .unwrap()
                .session_workers
                .is_empty()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn idle_session_close_is_independent_of_long_other_session_and_affinity_is_stable() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let first = host.invoke_stream(&id, input("idle", false)).await.unwrap();
    let again = host.invoke_stream(&id, input("idle", false)).await.unwrap();
    assert_eq!(first.result.final_content, again.result.final_content);
    let (sender, mut events) = tokio::sync::mpsc::channel(8);
    let running = tokio::spawn(
        host.invoke_stream_with_live_events_operation(&id, input("busy", true), Some(sender), None)
            .unwrap(),
    );
    entered(&mut events).await;
    let result = host
        .transport_session_operation(&id, close("idle"))
        .unwrap()
        .await;
    let before_release = dispatches(&package);
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    running.await.unwrap().unwrap();
    let receipt =
        result.expect("idle Close must complete within its normal deadline while B is blocked");
    assert!(receipt.closure_evidence.unwrap().local_released);
    assert_ne!(before_release[0]["pid"], before_release[2]["pid"]);
    let duplicate = host
        .transport_session_operation(&id, close("idle"))
        .unwrap()
        .await
        .unwrap();
    assert!(duplicate.closure_evidence.unwrap().local_released);
    assert_eq!(
        fs::read_to_string(package.path().join("bin/controls"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(host
        .invoke_stream(&id, input("idle", false))
        .await
        .unwrap_err()
        .to_string()
        .contains("already closed"));
    host.stop_all().await.unwrap();
    empty_workers(&host).await;
}

#[tokio::test]
async fn same_identity_serializes_and_cancelled_queue_does_not_dispatch() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let (sender, mut events) = tokio::sync::mpsc::channel(8);
    let first = tokio::spawn(
        host.invoke_stream_with_live_events_operation(&id, input("one", true), Some(sender), None)
            .unwrap(),
    );
    entered(&mut events).await;
    let queued = host
        .invoke_stream_with_live_events_operation(&id, input("one", false), None, None)
        .unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(50), queued)
        .await
        .is_err());
    assert_eq!(dispatches(&package).len(), 1);
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let first = first.await.unwrap().unwrap();
    let next = host.invoke_stream(&id, input("one", false)).await.unwrap();
    assert_eq!(first.result.final_content, next.result.final_content);
    host.stop_all().await.unwrap();
}

#[tokio::test]
async fn bounded_capacity_wait_cancel_deadline_and_release_reuse() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    lock_provider_worker_registry(&host.provider_workers)
        .unwrap()
        .session_capacity = SessionWorkerCapacity::for_test(1);
    host.invoke_stream(&id, input("one", false)).await.unwrap();
    let waiting = host
        .invoke_stream_with_live_events_operation(&id, input("cancelled", false), None, None)
        .unwrap();
    assert!(tokio::time::timeout(Duration::from_millis(50), waiting)
        .await
        .is_err());
    let mut loaded = host.loaded_package(&id).unwrap().clone();
    loaded.package.manifest.runtime.limits.invoke_timeout_ms = Some(20);
    host.loaded_packages.insert(id.clone(), loaded);
    let error = host
        .invoke_stream(&id, input("deadline", false))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("admission deadline"));
    assert_eq!(dispatches(&package).len(), 1);
    let mut loaded = host.loaded_package(&id).unwrap().clone();
    loaded.package.manifest.runtime.limits.invoke_timeout_ms = Some(3000);
    host.loaded_packages.insert(id.clone(), loaded);
    let waiting = tokio::spawn(
        host.invoke_stream_with_live_events_operation(&id, input("next", false), None, None)
            .unwrap(),
    );
    host.transport_session_operation(&id, close("one"))
        .unwrap()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(dispatches(&package).len(), 2);
    let old_pid = dispatches(&package)[0]["pid"].as_u64().unwrap();
    assert!(
        !pid_alive(old_pid),
        "slot returns only after actual process exit"
    );
    host.stop_all().await.unwrap();
    empty_workers(&host).await;
}

#[tokio::test]
async fn reload_and_unload_reap_all_workers_and_reject_old_package_admission() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    lock_provider_worker_registry(&host.provider_workers)
        .unwrap()
        .session_capacity = SessionWorkerCapacity::for_test(2);
    for session in ["one", "two"] {
        host.invoke_stream(&id, input(session, false))
            .await
            .unwrap();
    }
    let old = host
        .invoke_stream_with_live_events_operation(&id, input("unpolled-old", false), None, None)
        .unwrap();
    let waiting = tokio::spawn(
        host.invoke_stream_with_live_events_operation(&id, input("waiting", false), None, None)
            .unwrap(),
    );
    tokio::task::yield_now().await;
    let pids: Vec<_> = dispatches(&package)
        .iter()
        .map(|v| v["pid"].as_u64().unwrap())
        .collect();
    host.reload(&id).await.unwrap();
    assert!(waiting
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("package changed"));
    assert!(old
        .await
        .unwrap_err()
        .to_string()
        .contains("package changed"));
    for pid in pids {
        assert!(!pid_alive(pid));
    }
    assert!(host.invoke_stream(&id, input("one", false)).await.is_err());
    for session in ["three", "four"] {
        host.invoke_stream(&id, input(session, false))
            .await
            .unwrap();
    }
    host.unload(&id).await.unwrap();
    empty_workers(&host).await;
    for dispatch in dispatches(&package) {
        let pid = dispatch["pid"].as_u64().unwrap();
        assert!(!pid_alive(pid));
    }
}

#[tokio::test]
async fn stop_all_retires_two_busy_workers_with_one_quiesce_budget() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    let mut calls = Vec::new();
    let mut receivers = Vec::new();
    for session in ["busy-a", "busy-b"] {
        let (sender, mut events) = tokio::sync::mpsc::channel(8);
        calls.push(tokio::spawn(
            host.invoke_stream_with_live_events_operation(
                &id,
                input(session, true),
                Some(sender),
                None,
            )
            .unwrap(),
        ));
        entered(&mut events).await;
        receivers.push(events);
    }
    let stopped = tokio::time::timeout(Duration::from_secs(8), host.stop_all()).await;
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    stopped
        .expect("two busy children must share one 5s quiesce budget")
        .unwrap();
    for call in calls {
        assert!(call.await.unwrap().is_err());
    }
    empty_workers(&host).await;
    for dispatch in dispatches(&package) {
        let pid = dispatch["pid"].as_u64().unwrap();
        assert!(!pid_alive(pid));
    }
}

fn pid_alive(pid: u64) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn expired_closed_binding_is_pruned_without_evicting_active_worker() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, input("retired", false))
        .await
        .unwrap();
    host.transport_session_operation(&id, close("retired"))
        .unwrap()
        .await
        .unwrap();
    empty_workers(&host).await;
    host.invoke_stream(&id, input("active", false))
        .await
        .unwrap();
    {
        let mut registry = lock_provider_worker_registry(&host.provider_workers).unwrap();
        for binding in registry.transport_bindings.values_mut() {
            binding.expires_at = std::time::Instant::now() - Duration::from_secs(1);
        }
    }
    host.invoke_stream(&id, input("new", false)).await.unwrap();
    {
        let registry = lock_provider_worker_registry(&host.provider_workers).unwrap();
        assert!(!registry
            .transport_bindings
            .contains_key(&(id.clone(), "retired".into(), 7)));
        assert!(registry
            .transport_bindings
            .contains_key(&(id.clone(), "active".into(), 7)));
    }
    host.stop_all().await.unwrap();
}

#[tokio::test]
async fn reload_and_unload_reject_old_prepared_unbound_stream_without_creating_worker() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, invocation_input("fixture-model"))
        .await
        .unwrap();
    let old = host
        .invoke_stream_operation(&id, invocation_input("fixture-model"))
        .unwrap();
    let old_pid = dispatches(&package)[0]["pid"].as_u64().unwrap();
    host.reload(&id).await.unwrap();
    assert!(!pid_alive(old_pid));
    assert!(old
        .await
        .unwrap_err()
        .to_string()
        .contains("package changed"));
    assert!(
        provider_worker_supervisor_snapshot(&host.provider_workers, &id)
            .unwrap()
            .is_none()
    );
    assert_eq!(dispatches(&package).len(), 1);
    host.invoke_stream(&id, invocation_input("fixture-model"))
        .await
        .unwrap();
    let replacement_pid = dispatches(&package)[1]["pid"].as_u64().unwrap();
    assert_ne!(old_pid, replacement_pid);
    let old = host
        .invoke_stream_operation(&id, invocation_input("fixture-model"))
        .unwrap();
    host.unload(&id).await.unwrap();
    assert!(!pid_alive(replacement_pid));
    assert!(old
        .await
        .unwrap_err()
        .to_string()
        .contains("package changed"));
    assert!(
        provider_worker_supervisor_snapshot(&host.provider_workers, &id)
            .unwrap()
            .is_none()
    );
    assert_eq!(dispatches(&package).len(), 2);
}

#[tokio::test]
async fn stop_all_retires_busy_workers_across_plugins_with_one_host_budget() {
    let packages = [named_package("fixture_a"), named_package("fixture_b")];
    let mut host = ProviderHost::default();
    lock_provider_worker_registry(&host.provider_workers)
        .unwrap()
        .session_capacity = SessionWorkerCapacity::for_test(2);
    let mut calls = Vec::new();
    let mut receivers = Vec::new();
    let mut plugin_ids = Vec::new();
    for package in &packages {
        let id = host
            .load(package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        let (sender, mut events) = tokio::sync::mpsc::channel(8);
        let mut request = input("busy", true);
        request.provider_code = host
            .loaded_package(&id)
            .unwrap()
            .package
            .provider
            .provider_code
            .clone();
        calls.push(tokio::spawn(
            host.invoke_stream_with_live_events_operation(&id, request, Some(sender), None)
                .unwrap(),
        ));
        entered(&mut events).await;
        receivers.push(events);
        plugin_ids.push(id);
    }
    let stopped = tokio::time::timeout(Duration::from_secs(8), host.stop_all()).await;
    for package in &packages {
        fs::write(package.path().join("bin/release"), b"release").unwrap();
    }
    stopped
        .expect("different plugins must share one Host-wide 5s quiesce budget")
        .unwrap();
    for call in calls {
        assert!(call.await.unwrap().is_err());
    }
    assert_eq!(host.loaded_count(), 0);
    empty_workers(&host).await;
    for (package, id) in packages.iter().zip(&plugin_ids) {
        let pid = dispatches(package)[0]["pid"].as_u64().unwrap();
        assert!(!pid_alive(pid));
        let receipt = provider_worker_cleanup_receipt(&host.provider_workers, id)
            .unwrap()
            .unwrap();
        assert!(
            receipt.exited,
            "each plugin retains actual child cleanup evidence"
        );
    }
    // Both Host-wide slots must be reusable after the all-plugin shutdown.
    for package in &packages {
        let id = host
            .load(package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        let mut request = input("new", false);
        request.provider_code = host
            .loaded_package(&id)
            .unwrap()
            .package
            .provider
            .provider_code
            .clone();
        tokio::time::timeout(Duration::from_secs(3), host.invoke_stream(&id, request))
            .await
            .unwrap()
            .unwrap();
    }
    host.stop_all().await.unwrap();
}
