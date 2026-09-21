use super::*;
use crate::stdio_runtime::{call_executable_streaming, ProviderWorker};
use extension_package_runtime::provider_contract::{
    ProviderRuntimeErrorKind, ProviderStdioMethod, ProviderStdioRequest,
};
use std::os::unix::fs::PermissionsExt;

const PRIMARY: &str = r#"{"type":"error","error":{"kind":"rate_limited","message":"primary-provider-error","provider_summary":"fixture-upstream-429","provider_details":{"status":429,"code":"fixture-quota"}}}"#;
const SECONDARY: &str =
    r#"{"type":"error","error":{"kind":"provider_invalid_response","message":"secondary-error"}}"#;
const FAILED_RESULT: &str = r#"printf '%s\n' '{"type":"finish","reason":"error"}' '{"type":"result","result":{"final_content":"stale-first-result","finish_reason":"error"}}'"#;

fn write_script(package: &TempProviderPackage, script: &str) -> PathBuf {
    package.write("bin/fixture_provider", script);
    let path = package.path().join("bin/fixture_provider");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn error_script(package: &TempProviderPackage, tail: &str) -> PathBuf {
    write_script(
        package,
        &format!(
            r#"#!/usr/bin/env bash
set -eu
count=0
while IFS= read -r payload; do
  case "$payload" in
    *'"method":"invoke"'*)
      count=$((count + 1))
      if [ "$count" -eq 1 ]; then
        printf '%s\n' '{PRIMARY}' '{SECONDARY}'
        {tail}
      else
        printf '%s\n' '{{"type":"text_delta","delta":"second"}}' '{{"type":"result","result":{{"final_content":"second-call","finish_reason":"stop"}}}}'
      fi
      ;;
    *) printf '%s\n' '{{"ok":true,"result":{{}}}}' ;;
  esac
done
"#
        ),
    )
}

fn request() -> ProviderStdioRequest {
    ProviderStdioRequest {
        method: ProviderStdioMethod::Invoke,
        input: json!({}),
    }
}

fn limits() -> PluginRuntimeLimits {
    PluginRuntimeLimits {
        timeout_ms: Some(1_000),
        ..Default::default()
    }
}

fn assert_primary(error: PluginFrameworkError) {
    let PluginFrameworkError::RuntimeContract { error } = error else {
        panic!("expected original typed provider error");
    };
    assert_eq!(error.kind, ProviderRuntimeErrorKind::RateLimited);
    assert_eq!(error.message, "primary-provider-error");
    assert_eq!(
        error.provider_summary.as_deref(),
        Some("fixture-upstream-429")
    );
    assert_eq!(
        error.provider_details,
        Some(json!({"status":429,"code":"fixture-quota"}))
    );
}

#[tokio::test]
async fn managed_first_error_drains_terminal_and_reuses_same_worker_without_residual_frames() {
    let package = TempProviderPackage::new();
    package.write_stateful_provider_package(
        "fixture_provider",
        "fixture_provider",
        "Fixture Provider",
    );
    error_script(&package, FAILED_RESULT);
    let mut host = ProviderHost::default();
    let plugin_id = host
        .load(package.path().to_str().unwrap())
        .unwrap()
        .plugin_id;
    assert_primary(
        host.invoke_stream(&plugin_id, invocation_input("fixture-model"))
            .await
            .unwrap_err(),
    );
    let first = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    assert_eq!(first.state, ProviderWorkerLifecycleState::Active);
    assert!(host
        .provider_worker_cleanup_receipt(&plugin_id)
        .unwrap()
        .is_none());
    let second = host
        .invoke_stream(&plugin_id, invocation_input("fixture-model"))
        .await
        .unwrap();
    let after = host.provider_worker_snapshot(&plugin_id).unwrap().unwrap();
    host.stop_all().await.unwrap();
    assert_eq!(first.pid, after.pid);
    assert_eq!(first.generation, after.generation);
    assert_eq!(second.result.final_content.as_deref(), Some("second-call"));
    assert_eq!(
        second.events,
        vec![ProviderStreamEvent::TextDelta {
            delta: "second".into()
        }]
    );
}

#[tokio::test]
async fn managed_first_error_survives_eof_parse_io_timeout_and_closed_lane() {
    // Invalid UTF-8 exercises a non-typed I/O error, not merely another provider error.
    for tail in [
        "exit 0",
        "printf '%s\n' 'not-json'",
        "printf '\\377\\n'",
        "exec sleep 2",
    ] {
        let package = TempProviderPackage::new();
        let path = error_script(&package, tail);
        let mut worker = ProviderWorker::new(path, limits());
        assert_primary(
            worker
                .call_streaming(&request(), None, None, None)
                .await
                .unwrap_err(),
        );
        assert!(worker.last_cleanup_receipt().unwrap().exited);
        assert!(worker.process_control().is_none());
        // A broken stream may restart, but its buffered frames cannot reach the next call.
        write_script(
            &package,
            r#"#!/usr/bin/env bash
while IFS= read -r payload; do
printf '%s\n' '{"type":"result","result":{"final_content":"replacement","finish_reason":"stop"}}'
done
"#,
        );
        let next = worker
            .call_streaming(&request(), None, None, None)
            .await
            .unwrap();
        worker.stop().await;
        assert_eq!(next.result.final_content.as_deref(), Some("replacement"));
    }
    let package = TempProviderPackage::new();
    let mut worker = ProviderWorker::new(error_script(&package, FAILED_RESULT), limits());
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    assert_primary(
        worker
            .call_streaming(&request(), Some(sender), None, None)
            .await
            .unwrap_err(),
    );
    assert!(worker.last_cleanup_receipt().unwrap().exited);
}

#[tokio::test]
async fn standalone_first_error_survives_result_eof_parse_io_exit_and_closed_lane() {
    for tail in [
        FAILED_RESULT,
        "exit 0",
        "printf '%s\n' 'not-json'",
        "printf '\\377\\n'",
        "printf '%s\n' 'secondary-process-failure' >&2; exit 7",
        "exec sleep 2",
    ] {
        let package = TempProviderPackage::new();
        let path = error_script(&package, tail);
        assert_primary(
            call_executable_streaming(&path, &request(), &limits(), None, None, None, None)
                .await
                .unwrap_err(),
        );
    }
    let package = TempProviderPackage::new();
    let path = error_script(&package, FAILED_RESULT);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    assert_primary(
        call_executable_streaming(&path, &request(), &limits(), Some(sender), None, None, None)
            .await
            .unwrap_err(),
    );
}

#[tokio::test]
async fn secondary_failure_without_provider_error_remains_a_failure() {
    let package = TempProviderPackage::new();
    let path = write_script(
        &package,
        "#!/usr/bin/env bash\nread -r payload\nprintf '\\377\\n'\n",
    );
    let mut worker = ProviderWorker::new(path.clone(), limits());
    let managed = worker
        .call_streaming(&request(), None, None, None)
        .await
        .unwrap_err();
    assert!(!matches!(
        managed,
        PluginFrameworkError::RuntimeContract { .. }
    ));
    let standalone =
        call_executable_streaming(&path, &request(), &limits(), None, None, None, None)
            .await
            .unwrap_err();
    assert!(!matches!(
        standalone,
        PluginFrameworkError::RuntimeContract { .. }
    ));
}
