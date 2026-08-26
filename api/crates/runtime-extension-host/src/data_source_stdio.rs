use std::{path::Path, process::Stdio, time::Duration};

use plugin_framework::{
    data_source_contract::{DataSourceStdioError, DataSourceStdioRequest, DataSourceStdioResponse},
    error::{FrameworkResult, PluginFrameworkError},
    provider_contract::ProviderRuntimeError,
    PluginRuntimeLimits,
};
use serde_json::Value;
use tokio::{io::AsyncWriteExt, process::Command};

pub async fn call_executable(
    executable_path: &Path,
    request: &DataSourceStdioRequest,
    limits: &PluginRuntimeLimits,
) -> FrameworkResult<Value> {
    let mut command = Command::new(executable_path);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    apply_memory_limit(&mut command, limits.memory_bytes)?;

    let mut child = command
        .spawn()
        .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(request)
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
        stdin.write_all(&payload).await.map_err(|error| {
            map_dispatched_transport_error(request, executable_path, error.to_string())
        })?;
    }

    let output = tokio::time::timeout(
        Duration::from_millis(limits.timeout_ms.unwrap_or(30_000)),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        map_dispatched_transport_error(
            request,
            executable_path,
            "data source runtime timed out".to_string(),
        )
    })?
    .map_err(|error| map_dispatched_transport_error(request, executable_path, error.to_string()))?;

    parse_stdio_response(executable_path, &output.stdout, &output.stderr)
}

fn parse_stdio_response(
    executable_path: &Path,
    stdout: &[u8],
    stderr: &[u8],
) -> FrameworkResult<Value> {
    let stdout = String::from_utf8_lossy(stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(stderr).trim().to_string();
    if stdout.is_empty() {
        return Err(PluginFrameworkError::runtime(
            ProviderRuntimeError::normalize(
                "data_source_runtime",
                if stderr.is_empty() {
                    "data source runtime returned empty output"
                } else {
                    stderr.as_str()
                },
                None,
            ),
        ));
    }

    let envelope = serde_json::from_str::<DataSourceStdioResponse>(&stdout).map_err(|error| {
        PluginFrameworkError::serialization(Some(executable_path), error.to_string())
    })?;

    if envelope.ok {
        return Ok(envelope.result);
    }

    let error = envelope.error.unwrap_or_else(|| DataSourceStdioError {
        code: None,
        message: if stderr.is_empty() {
            "data source runtime execution failed".to_string()
        } else {
            stderr.clone()
        },
        detail: None,
        provider_summary: None,
    });
    let code = error.code.as_deref().unwrap_or("data_source_runtime");
    let mut runtime_error =
        ProviderRuntimeError::normalize(code, error.message, error.provider_summary.as_deref());
    if error.code.is_some() || error.detail.is_some() {
        runtime_error = runtime_error.with_provider_details(serde_json::json!({
            "code": error.code,
            "detail": error.detail,
        }));
    }
    Err(PluginFrameworkError::runtime(runtime_error))
}

fn map_dispatched_transport_error(
    request: &DataSourceStdioRequest,
    executable_path: &Path,
    message: String,
) -> PluginFrameworkError {
    if request.method != plugin_framework::data_source_contract::DataSourceStdioMethod::ExecuteSql {
        return PluginFrameworkError::io(Some(executable_path), message);
    }
    PluginFrameworkError::runtime(
        ProviderRuntimeError::new(
            plugin_framework::provider_contract::ProviderRuntimeErrorKind::ProviderTransportUnavailable,
            message,
        )
        .with_provider_summary("outcome_unknown")
        .with_provider_details(serde_json::json!({ "code": "outcome_unknown" })),
    )
}

fn apply_memory_limit(command: &mut Command, memory_bytes: Option<u64>) -> FrameworkResult<()> {
    #[cfg(unix)]
    {
        if let Some(limit) = memory_bytes {
            unsafe {
                command.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: limit as libc::rlim_t,
                        rlim_max: limit as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (command, memory_bytes);
    }

    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use plugin_framework::{
        data_source_contract::{DataSourceStdioMethod, DataSourceStdioRequest},
        PluginRuntimeLimits,
    };
    use serde_json::json;
    use tokio::time::{sleep, Duration};

    use super::call_executable;

    struct TempRuntime {
        root: PathBuf,
    }

    impl TempRuntime {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/test-fixtures")
                .join(format!("data-source-stdio-timeout-{nonce}"));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn script_path(&self) -> PathBuf {
            self.root.join("runtime.sh")
        }

        fn pid_path(&self) -> PathBuf {
            self.root.join("runtime.pid")
        }
    }

    impl Drop for TempRuntime {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn write_sleeping_runtime(temp: &TempRuntime) {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/_fixtures/timeout_runtime.sh");
        fs::hard_link(fixture, temp.script_path()).unwrap();
    }

    fn process_exists(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    async fn read_pid(path: &Path) -> i32 {
        for _ in 0..20 {
            if let Ok(raw) = fs::read_to_string(path) {
                return raw.trim().parse::<i32>().unwrap();
            }
            sleep(Duration::from_millis(10)).await;
        }
        panic!("runtime did not write pid file");
    }

    #[tokio::test]
    async fn timeout_terminates_data_source_child_process() {
        let temp = TempRuntime::new();
        write_sleeping_runtime(&temp);
        let request = DataSourceStdioRequest {
            method: DataSourceStdioMethod::ValidateConfig,
            input: json!({}),
        };
        let limits = PluginRuntimeLimits {
            // Leave enough time for a loaded CI runner to schedule the shell and write its PID.
            timeout_ms: Some(1_000),
            ..Default::default()
        };

        let error = call_executable(&temp.script_path(), &request, &limits)
            .await
            .expect_err("sleeping runtime should time out");
        let pid = read_pid(&temp.pid_path()).await;
        sleep(Duration::from_millis(100)).await;

        assert!(error.to_string().contains("timed out"));
        assert!(
            !process_exists(pid),
            "timed out data source child process should be terminated"
        );
    }
}
