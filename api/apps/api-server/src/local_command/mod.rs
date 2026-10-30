//! API-host process owner. No remote target, session, PTY, or command retry.
use control_plane::local_command::{LocalCommandExecutor, LocalCommandFuture, LocalCommandPolicy};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

#[cfg(unix)]
mod unix;

pub(crate) struct HostCommandExecutor;

impl LocalCommandExecutor for HostCommandExecutor {
    fn execute(&self, command: String, policy: LocalCommandPolicy) -> LocalCommandFuture<'_> {
        Box::pin(async move {
            #[cfg(unix)]
            {
                // The task owns kill/wait and both pipes even if its caller disappears.
                tokio::spawn(unix::execute(command, policy)).await?
            }
            #[cfg(not(unix))]
            {
                let _ = (command, policy);
                anyhow::bail!("local command execution requires a Unix process-group owner")
            }
        })
    }
}

pub(crate) fn executor() -> Arc<dyn LocalCommandExecutor> {
    Arc::new(HostCommandExecutor)
}

pub(crate) fn policy_from_env(
    map: &BTreeMap<String, String>,
) -> anyhow::Result<Option<LocalCommandPolicy>> {
    const TIMEOUT: &str = "API_LOCAL_COMMAND_TIMEOUT_MS";
    const OUTPUT: &str = "API_LOCAL_COMMAND_MAX_OUTPUT_BYTES";
    match (map.get(TIMEOUT), map.get(OUTPUT)) {
        (None, None) => Ok(None),
        (Some(timeout), Some(output)) => {
            anyhow::ensure!(
                cfg!(unix),
                "local command execution is not supported on this platform"
            );
            let timeout_ms: u64 = timeout
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid {TIMEOUT}"))?;
            let max_output_bytes: usize = output
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid {OUTPUT}"))?;
            let timeout = Duration::from_millis(timeout_ms);
            anyhow::ensure!(
                timeout_ms > 0 && std::time::Instant::now().checked_add(timeout).is_some(),
                "invalid {TIMEOUT}"
            );
            anyhow::ensure!(
                max_output_bytes > 0 && max_output_bytes <= isize::MAX as usize,
                "invalid {OUTPUT}"
            );
            Ok(Some(LocalCommandPolicy {
                timeout,
                max_output_bytes,
            }))
        }
        _ => anyhow::bail!("configure both {TIMEOUT} and {OUTPUT} to enable local commands"),
    }
}

#[cfg(test)]
#[path = "_tests/mod.rs"]
mod tests;
