mod managed_event_transport;
mod managed_hook_transport;
mod network_egress_runtime;
mod provider_stdio_streaming;
mod stdio_host_calls;

mod managed_capability_multiplex;

#[cfg(unix)]
pub(crate) fn write_executable_fixture(path: &std::path::Path, bytes: &[u8], mode: u32) {
    use std::{fs, io::Write, os::unix::fs::PermissionsExt};

    let mut writer = fs::File::create(path).unwrap();
    writer.write_all(bytes).unwrap();
    writer
        .set_permissions(fs::Permissions::from_mode(mode))
        .unwrap();
    // Concurrent fork can inherit the writable descriptor until exec, even
    // after this thread closes it. The shared lock waits for those copies too.
    // See rust-lang/rust#114554; keep this fixture barrier outside runtime code.
    writer.lock().unwrap();
    drop(writer);
    let reader = fs::File::open(path).unwrap();
    reader.lock_shared().unwrap();
    drop(reader);
}

#[cfg(unix)]
#[tokio::test]
async fn worker_does_not_inherit_host_jemalloc_policy() {
    const TEST: &str = "_tests::worker_does_not_inherit_host_jemalloc_policy";
    const MARKER: &str = "FLOWBASE_TEST_JEMALLOC_PARENT";
    // Re-enter the test harness with a controlled parent environment; never mutate the shared
    // process environment while other tests are spawning workers.
    if std::env::var_os(MARKER).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(MARKER, "inherited")
            .env("_RJEM_MALLOC_CONF", "thp:never,background_thread:true")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }

    assert!(std::env::var_os("_RJEM_MALLOC_CONF").is_some());
    let root = std::env::temp_dir().join(format!("flowbase-worker-env-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let worker = root.join("worker");
    write_executable_fixture(&worker, br#"#!/bin/sh
cat >/dev/null || exit 1
if [ "${_RJEM_MALLOC_CONF+x}" = x ] || [ "$FLOWBASE_TEST_JEMALLOC_PARENT" != inherited ] || [ -z "$PATH" ]; then
    printf '{"ok":true,"result":false}\n'
else
    printf '{"ok":true,"result":true}\n'
fi
"#, 0o700);
    let result = crate::capability_stdio::call_executable(
        &worker,
        &extension_contracts::CapabilityStdioRequest {
            method: extension_contracts::CapabilityStdioMethod::Execute,
            input: serde_json::Value::Null,
        },
        &Default::default(),
    )
    .await;
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap(), serde_json::Value::Bool(true));
}
