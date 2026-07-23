use std::{path::Path, process::Stdio, time::Duration};

use plugin_framework::{
    error::{FrameworkResult, PluginFrameworkError},
    provider_contract::ProviderRuntimeError,
    PluginRuntimeLimits,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{io::AsyncWriteExt, process::Command};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStdioMethod {
    ValidateConfig,
    ResolveDynamicOptions,
    ResolveOutputSchema,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStdioRequest {
    pub method: CapabilityStdioMethod,
    #[serde(default)]
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStdioResponse {
    pub ok: bool,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<CapabilityStdioError>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityStdioError {
    pub message: String,
    #[serde(default)]
    pub provider_summary: Option<String>,
}

pub async fn call_executable(
    executable_path: &Path,
    request: &CapabilityStdioRequest,
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
        stdin
            .write_all(&payload)
            .await
            .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;
    }

    let output = tokio::time::timeout(
        Duration::from_millis(limits.timeout_ms.unwrap_or(30_000)),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        PluginFrameworkError::runtime(ProviderRuntimeError::normalize(
            "capability_runtime",
            "capability runtime timed out",
            None,
        ))
    })?
    .map_err(|error| PluginFrameworkError::io(Some(executable_path), error.to_string()))?;

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
                "capability_runtime",
                if stderr.is_empty() {
                    "capability runtime returned empty output"
                } else {
                    stderr.as_str()
                },
                None,
            ),
        ));
    }

    let envelope = serde_json::from_str::<CapabilityStdioResponse>(&stdout).map_err(|error| {
        PluginFrameworkError::serialization(Some(executable_path), error.to_string())
    })?;

    if envelope.ok {
        return Ok(envelope.result);
    }

    let error = envelope.error.unwrap_or_else(|| CapabilityStdioError {
        message: if stderr.is_empty() {
            "capability runtime execution failed".to_string()
        } else {
            stderr.clone()
        },
        provider_summary: None,
    });

    Err(PluginFrameworkError::runtime(
        ProviderRuntimeError::normalize(
            "capability_runtime",
            error.message,
            error.provider_summary.as_deref(),
        ),
    ))
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
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use plugin_framework::PluginRuntimeLimits;
    use serde_json::json;
    use tokio::time::{sleep, Duration};

    use super::{call_executable, CapabilityStdioMethod, CapabilityStdioRequest};

    struct TempRuntime {
        root: PathBuf,
    }

    impl TempRuntime {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("capability-stdio-timeout-{nonce}"));
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
        fs::write(
            temp.script_path(),
            format!(
                "#!/usr/bin/env bash\nprintf '%s' $$ > '{}'\nsleep 5\n",
                temp.pid_path().display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(temp.script_path()).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(temp.script_path(), permissions).unwrap();
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
    async fn timeout_terminates_capability_child_process() {
        let temp = TempRuntime::new();
        write_sleeping_runtime(&temp);
        let request = CapabilityStdioRequest {
            method: CapabilityStdioMethod::ValidateConfig,
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
            "timed out capability child process should be terminated"
        );
    }
}
