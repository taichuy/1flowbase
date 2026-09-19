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
