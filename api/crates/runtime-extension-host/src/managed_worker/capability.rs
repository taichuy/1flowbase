use super::{invalid, process::ManagedChild, LoadedManagedBinding};
use crate::{
    capability_stdio::{parse_stdio_response, CapabilityStdioRequest},
    plugin_scope::PluginScopeLease,
};
use extension_package_runtime::FrameworkResult;
use serde_json::Value;
use std::{process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
pub(super) async fn exchange(
    binding: &LoadedManagedBinding,
    request: &CapabilityStdioRequest,
    lease: Arc<PluginScopeLease>,
) -> FrameworkResult<Value> {
    let payload =
        serde_json::to_vec(request).map_err(|_| invalid("managed capability cannot be encoded"))?;
    if payload.len() > MAX_OUTPUT_BYTES as usize {
        return Err(invalid("managed capability request exceeds frame limit"));
    }
    let mut command = Command::new(&binding.runtime_executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    if let Some(bytes) = binding.limits.memory_bytes {
        unsafe {
            command.pre_exec(move || {
                let limit = libc::rlimit {
                    rlim_cur: bytes as libc::rlim_t,
                    rlim_max: bytes as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let child = command
        .spawn()
        .map_err(|_| invalid("managed capability worker could not start"))?;
    let mut owner = ManagedChild::new(child, lease);
    let child = owner.child();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| invalid("managed capability stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid("managed capability stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| invalid("managed capability stderr unavailable"))?;
    let write = async move {
        stdin.write_all(&payload).await?;
        stdin.shutdown().await
    };
    async fn read(stream: impl tokio::io::AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        stream
            .take(MAX_OUTPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > MAX_OUTPUT_BYTES as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "managed capability output exceeds frame limit",
            ));
        }
        Ok(bytes)
    }
    let (_, stdout, stderr, status) =
        tokio::try_join!(write, read(stdout), read(stderr), child.wait())
            .map_err(|_| invalid("managed capability exchange failed or exceeded output limit"))?;
    if !status.success() {
        return Err(invalid("managed capability worker exited unsuccessfully"));
    }
    parse_stdio_response(&binding.runtime_executable, &stdout, &stderr)
}
