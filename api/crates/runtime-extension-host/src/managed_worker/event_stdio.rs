use std::process::Stdio;

use extension_contracts::{
    ManagedEventHostFrame, ManagedEventOutcome, ManagedEventWorkerFrame,
    MANAGED_EVENT_MAX_FRAME_BYTES,
};
use extension_package_runtime::FrameworkResult;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

use super::{invalid, LoadedManagedBinding};

/// Isolated additive wire family. Provider/capability stdout parsing is intentionally not involved.
pub(super) async fn exchange(
    binding: &LoadedManagedBinding,
    frame: &ManagedEventHostFrame,
    payload: Vec<u8>,
    lease: std::sync::Arc<crate::plugin_scope::PluginScopeLease>,
) -> FrameworkResult<ManagedEventOutcome> {
    let mut command = Command::new(&binding.runtime_executable);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
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
        .map_err(|_| invalid("managed event worker could not start"))?;
    let mut owner = super::process::ManagedChild::new(child, lease);
    let child = owner.child();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| invalid("managed event stdin unavailable"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid("managed event stdout unavailable"))?;
    let write = async move {
        stdin.write_all(&payload).await?;
        stdin.shutdown().await
    };
    let read = async move {
        let mut bytes = Vec::new();
        stdout
            .take((MANAGED_EVENT_MAX_FRAME_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > MANAGED_EVENT_MAX_FRAME_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "managed event response exceeds frame limit",
            ));
        }
        Ok::<_, std::io::Error>(bytes)
    };
    let (_, bytes, status) = tokio::try_join!(write, read, child.wait())
        .map_err(|_| invalid("managed event worker exchange failed"))?;
    if !status.success() {
        return Err(invalid("managed event worker exited unsuccessfully"));
    }
    let response: ManagedEventWorkerFrame = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("managed event response is malformed"))?;
    response
        .validate_for(frame)
        .map_err(|error| invalid(&error.to_string()))?;
    Ok(response.result)
}
