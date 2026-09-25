use super::*;

#[tokio::test]
async fn expired_execution_deadline_retires_the_slow_stateful_worker() {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package(
        "fixture_provider",
        "fixture_provider",
        "Fixture Provider",
    );
    package.write(
        "bin/fixture_provider",
        r#"#!/usr/bin/env bash
set -euo pipefail
while IFS= read -r payload; do
  case "${payload}" in
    *'"method":"invoke"'*)
      printf '%s\n' '{"type":"text_delta","delta":"started"}'
      sleep 2
      printf '%s\n' '{"type":"result","result":{"final_content":"late","finish_reason":"stop"}}'
      ;;
    *) printf '%s\n' '{"ok":true,"result":{}}' ;;
  esac
done
"#,
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = package.path().join("bin/fixture_provider");
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    host.validate(&plugin_id, json!({}))
        .await
        .expect("warm the worker before measuring its invoke deadline");
    let now_ms =
        i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000).unwrap();
    let principal = runtime_core::runtime_backend::RuntimeExecutionPrincipal {
        workspace_id: "fixture-workspace".into(),
        actor_id: None,
        deadline_unix_ms: now_ms + 500,
    };
    let invocation = host
        .invoke_stream_with_host_calls_operation(
            &plugin_id,
            invocation_input("fixture-model"),
            None,
            None,
            None,
            (Some(principal), None),
        )
        .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(3), invocation)
        .await
        .expect("execution deadline must stop the call")
        .unwrap_err();
    assert!(error.to_string().contains("timed out"), "{error}");
    let worker = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    let cleanup = host
        .provider_worker_cleanup_receipt(&plugin_id)
        .unwrap()
        .unwrap();
    assert_eq!(worker.state, ProviderWorkerLifecycleState::Failed);
    assert_eq!(cleanup.prior_pid, worker.pid);
    assert!(cleanup.exited, "timed out worker must have exited");
    assert!(cleanup.cleanup_error.is_none());
}
