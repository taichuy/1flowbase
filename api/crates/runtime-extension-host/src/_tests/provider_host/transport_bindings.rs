use super::*;
use std::os::unix::fs::PermissionsExt;

fn package() -> TempProviderPackage {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package("fixture_provider", "fixture_provider", "Fixture");
    package.write("bin/fixture_provider", r#"#!/usr/bin/env bash
set -eu
while IFS= read -r payload; do
  case "$payload" in
    *'"method":"invoke"'*)
      printf '%s\n' "$payload" >> "$(dirname "$0")/invokes"
      printf '%s\n' '{"type":"result","result":{"final_content":"ok","finish_reason":"stop"}}'
      ;;
    *'"method":"transport_session"'*)
      printf '%s\n' "$payload" >> "$(dirname "$0")/controls"
      printf '%s\n' '{"ok":true,"result":{"generation":7,"reused":false,"physical_state":"closed","connection_age_ms":0,"ttl_remaining_ms":0,"close_reason":"requested_close","close_acknowledged":null,"closure_evidence":{"source":"confirmed_worker_exit","no_ack_reason":"unknown","identity":{"logical_session_id":"bound","generation":7,"worker_incarnation":1},"local_released":true,"peer_close_acknowledged":null}}}'
      ;;
    *) exit 7 ;;
  esac
done
"#);
    fs::set_permissions(
        package.path().join("bin/fixture_provider"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    package
}

fn input() -> ProviderInvocationInput {
    let mut input = invocation_input("fixture-model");
    input
        .set_transport_session_directive(ProviderTransportSessionDirective {
            logical_session_id: "bound".into(),
            generation: 7,
            worker_incarnation: Some(999),
            task_id: "first".into(),
            state: ProviderLogicalSessionState::Active,
            physical_deadline_unix_ms: 4_102_444_800_000,
        })
        .unwrap();
    input
}

fn command() -> ProviderTransportSessionCommand {
    ProviderTransportSessionCommand {
        logical_session_id: "bound".into(),
        generation: 7,
        worker_incarnation: None,
        action: ProviderTransportSessionAction::Close,
        deadline_unix_ms: 4_102_444_800_000,
    }
}

#[tokio::test]
async fn dispatch_binds_real_worker_and_rejects_provider_forged_exit() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, input()).await.unwrap();
    let invocation: Value = serde_json::from_str(
        fs::read_to_string(package.path().join("bin/invokes"))
            .unwrap()
            .trim(),
    )
    .unwrap();
    assert_eq!(
        invocation["input"]["run_context"]["physical_transport_session"]["worker_incarnation"],
        json!(1)
    );
    let error = host
        .transport_session_operation(&id, command())
        .unwrap()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("identity or source rejected"));
    let snapshot = provider_worker_supervisor_snapshot(&host.provider_workers, &id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.state, ProviderWorkerLifecycleState::Active);
}

#[tokio::test]
async fn confirmed_old_worker_exit_is_not_dispatched_to_replacement() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.invoke_stream(&id, input()).await.unwrap();
    let old = provider_worker_handle(
        &host.provider_workers,
        id.clone(),
        host.loaded_package(&id).unwrap(),
    )
    .unwrap();
    assert!(old
        .call(&ProviderStdioRequest {
            method: ProviderStdioMethod::Validate,
            input: json!({})
        })
        .await
        .is_err());
    assert!(old.last_cleanup_receipt().unwrap().unwrap().exited);
    let replacement = provider_worker_handle(
        &host.provider_workers,
        id.clone(),
        host.loaded_package(&id).unwrap(),
    )
    .unwrap();
    assert_eq!(replacement.incarnation().unwrap(), 2);
    let receipt = host
        .transport_session_operation(&id, command())
        .unwrap()
        .await
        .unwrap();
    let evidence = receipt.closure_evidence.unwrap();
    assert_eq!(
        evidence.source,
        ProviderTransportClosureSource::ConfirmedWorkerExit
    );
    assert_eq!(evidence.identity.worker_incarnation, 1);
    assert!(evidence.local_released);
    assert_eq!(evidence.peer_close_acknowledged, None);
    assert!(!package.path().join("bin/controls").exists());
    assert!(host
        .invoke_stream(&id, input())
        .await
        .unwrap_err()
        .to_string()
        .contains("previous worker"));
}

#[tokio::test]
async fn missing_or_wrong_binding_never_routes_control_to_current_worker() {
    let package = package();
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    assert!(host
        .transport_session_operation(&id, command())
        .unwrap()
        .await
        .is_err());
    host.invoke_stream(&id, input()).await.unwrap();
    let mut wrong = command();
    wrong.worker_incarnation = Some(2);
    assert!(host
        .transport_session_operation(&id, wrong)
        .unwrap()
        .await
        .is_err());
    assert!(!package.path().join("bin/controls").exists());
}

fn released_receipt() -> Value {
    json!({"generation":7,"reused":true,"physical_state":"closed","connection_age_ms":42,
        "ttl_remaining_ms":0,"close_reason":"transport_fault","close_acknowledged":false,
        "closure_evidence":{"source":"provider_local_release","no_ack_reason":"transport_error",
            "identity":{"logical_session_id":"bound","generation":7,"worker_incarnation":1},
            "local_released":true,"peer_close_acknowledged":false}})
}

fn failing_package(receipt: Option<Value>) -> (TempProviderPackage, ProviderRuntimeError) {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package("fixture_provider", "fixture_provider", "Fixture");
    let mut details = json!({"status":502,"original_diagnostic":"preserved"});
    if let Some(receipt) = receipt {
        details["1flowbase_physical_transport_session"] = receipt;
    }
    let primary = ProviderRuntimeError {
        kind: ProviderRuntimeErrorKind::ProviderTransportUnavailable,
        message: "original-websocket-failure".into(),
        provider_summary: Some("original-summary".into()),
        provider_details: Some(details),
    };
    let frame = json!({"type":"error","error":primary}).to_string();
    package.write("bin/fixture_provider", &format!(r#"#!/usr/bin/env bash
set -eu
count=0
dir="$(dirname "$0")"
while IFS= read -r payload; do
  case "$payload" in
    *'"method":"invoke"'*)
      count=$((count + 1))
      printf '%s\n' "$payload" >> "$dir/invokes"
      if [ "$count" -eq 1 ]; then
        printf '%s\n' '{frame}' '{{"type":"result","result":{{"finish_reason":"error"}}}}'
      else
        printf '%s\n' '{{"type":"text_delta","delta":"busy"}}'
        while [ ! -f "$dir/release" ]; do sleep 0.01; done
        printf '%s\n' '{{"type":"result","result":{{"final_content":"other-session-completed","finish_reason":"stop"}}}}'
      fi
      ;;
    *'"method":"transport_session"'*)
      printf '%s\n' "$payload" >> "$dir/controls"
      printf '%s\n' '{{"ok":true,"result":{{"generation":7,"reused":false,"physical_state":"faulted","connection_age_ms":0,"ttl_remaining_ms":0,"close_reason":"requested_close"}}}}'
      ;;
    *) exit 7 ;;
  esac
done
"#));
    fs::set_permissions(
        package.path().join("bin/fixture_provider"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    (package, primary)
}

fn assert_original_error(error: PluginFrameworkError, expected: &ProviderRuntimeError) {
    let PluginFrameworkError::RuntimeContract { error } = error else {
        panic!("original typed provider error must be preserved");
    };
    assert_eq!(&*error, expected);
}

#[tokio::test]
async fn final_failure_proof_closes_without_queueing_behind_busy_worker() {
    let (package, primary) = failing_package(Some(released_receipt()));
    let mut host = ProviderHost::default();
    let id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    assert_original_error(
        host.invoke_stream(&id, input()).await.unwrap_err(),
        &primary,
    );
    let mut wrong_worker = command();
    wrong_worker.worker_incarnation = Some(99);
    assert!(host
        .transport_session_operation(&id, wrong_worker)
        .unwrap()
        .await
        .is_err());
    let host = Arc::new(host);
    let (sender, mut events) = tokio::sync::mpsc::channel(8);
    let running_host = Arc::clone(&host);
    let running_id = id.clone();
    let mut other = input();
    let mut directive = other.transport_session_directive().unwrap().unwrap();
    directive.logical_session_id = "other-session".into();
    other.set_transport_session_directive(directive).unwrap();
    let running = tokio::spawn(async move {
        running_host
            .invoke_stream_with_live_events(&running_id, other, Some(sender), None)
            .await
    });
    let ready = tokio::time::timeout(Duration::from_secs(2), events.recv()).await;
    // Even a failing assertion releases our deterministic worker gate first.
    let observed_busy =
        matches!(ready, Ok(Some(ProviderStreamEvent::TextDelta { ref delta })) if delta == "busy");
    let mut close = command();
    close.deadline_unix_ms = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + 100) as i64;
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        host.transport_session_operation(&id, close).unwrap(),
    )
    .await;
    let controls_queued = package.path().join("bin/controls").exists();
    fs::write(package.path().join("bin/release"), b"release").unwrap();
    let completed = running.await.unwrap().unwrap();
    let readmission = host.invoke_stream(&id, input()).await;
    host.stop_all().await.unwrap();
    assert!(
        observed_busy,
        "unrelated invocation must own the worker before Close"
    );
    assert_eq!(
        completed.result.final_content.as_deref(),
        Some("other-session-completed")
    );
    let receipt = result
        .expect("Close proof must not wait for the carrier")
        .unwrap();
    assert_eq!(serde_json::to_value(receipt).unwrap(), released_receipt());
    assert!(
        !controls_queued,
        "proven closure must not write a redundant stdio command"
    );
    assert!(readmission
        .unwrap_err()
        .to_string()
        .contains("already closed"));
    assert_eq!(
        fs::read_to_string(package.path().join("bin/invokes"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

#[tokio::test]
async fn invalid_or_absent_failure_proof_preserves_error_and_normal_close() {
    for mutation in 0..10 {
        let mut receipt = released_receipt();
        match mutation {
            0 => {
                receipt["closure_evidence"]["source"] = json!("confirmed_worker_exit");
                receipt["close_acknowledged"] = Value::Null;
                receipt["closure_evidence"]["peer_close_acknowledged"] = Value::Null;
            }
            1 => receipt["closure_evidence"]["identity"]["logical_session_id"] = json!("foreign"),
            2 => receipt["closure_evidence"]["identity"]["worker_incarnation"] = json!(99),
            3 => {
                receipt["generation"] = json!(8);
                receipt["closure_evidence"]["identity"]["generation"] = json!(8);
            }
            4 => receipt["physical_state"] = json!("ready"),
            5 => receipt["ttl_remaining_ms"] = json!(10),
            6 => receipt["closure_evidence"]["local_released"] = json!(false),
            7 => receipt = json!({"malformed":true}),
            8 => {
                receipt.as_object_mut().unwrap().remove("closure_evidence");
            }
            9 => {}
            _ => unreachable!(),
        }
        let (package, primary) = failing_package((mutation != 9).then_some(receipt));
        let mut host = ProviderHost::default();
        let id = host
            .load(package.path().to_str().unwrap())
            .unwrap()
            .plugin_id;
        assert_original_error(
            host.invoke_stream(&id, input()).await.unwrap_err(),
            &primary,
        );
        let receipt = host
            .transport_session_operation(&id, command())
            .unwrap()
            .await
            .unwrap();
        assert!(receipt.closure_evidence.is_none(), "mutation {mutation}");
        assert!(
            package.path().join("bin/controls").exists(),
            "mutation {mutation}"
        );
        fs::write(package.path().join("bin/release"), b"release").unwrap();
        assert!(
            host.invoke_stream(&id, input()).await.is_ok(),
            "unproved generation remains admitted"
        );
        host.stop_all().await.unwrap();
    }
}
