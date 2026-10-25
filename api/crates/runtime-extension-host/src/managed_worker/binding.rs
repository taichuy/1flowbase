use std::path::PathBuf;

use extension_package_runtime::{PluginExecutionMode, PluginRuntimeLimits};

/// Exact, validated executable selection. The package-level runtime is never consulted here.
#[derive(Debug, Clone)]
pub(crate) struct LoadedManagedBinding {
    pub plugin_id: String,
    pub publisher_namespace: String,
    pub plugin_code: String,
    pub plugin_version: String,
    pub protocol: String,
    pub runtime_executable: PathBuf,
    pub executable_fingerprint: extension_contracts::ManagedArtifactFingerprint,
    pub execution_mode: PluginExecutionMode,
    pub limits: PluginRuntimeLimits,
    pub handler: String,
    pub worker_idle_timeout_ms: Option<u64>,
    pub worker_retention: bool,
    pub actor_workspace_scope: bool,
    pub interface_protocol: Option<extension_contracts::ManagedInterfaceProtocol>,
    pub contribution: extension_contracts::extension_bus::ContributionDescriptor,
}

impl LoadedManagedBinding {
    pub(super) fn call_context(
        &self,
        principal: &runtime_core::runtime_backend::RuntimeExecutionPrincipal,
    ) -> extension_package_runtime::FrameworkResult<
        Option<extension_contracts::ManagedServiceCallContext>,
    > {
        if !self.actor_workspace_scope {
            return Ok(None);
        }
        let caller = principal.caller.as_ref().ok_or_else(|| {
            super::invalid("actor_workspace_scope service requires a captured caller")
        })?;
        let actor_id = caller.actor.user_id.to_string();
        if principal.actor_id.as_deref() != Some(actor_id.as_str()) {
            return Err(super::invalid(
                "managed service caller actor does not match execution actor",
            ));
        }
        Ok(Some(extension_contracts::ManagedServiceCallContext {
            actor_id,
            workspace_id: caller.actor.current_workspace_id.to_string(),
            deadline_unix_ms: principal.deadline_unix_ms,
        }))
    }

    /// Runs on the host's blocking pool before admission; the installed path is never resolved
    /// again through a current/latest installation. Removed or replaced bytes fail closed.
    /// The artifact owner must keep published files immutable: this check is not synchronization
    /// with an unrelated filesystem writer racing the subsequent process spawn.
    pub(crate) fn verify_executable(&self) -> extension_package_runtime::FrameworkResult<()> {
        let started = std::time::Instant::now();
        tracing::debug!(
            contribution_id = self.contribution.contribution_id.as_str(),
            status = "started",
            "managed executable verification"
        );
        let bytes = match std::fs::read(&self.runtime_executable) {
            Ok(bytes) => bytes,
            Err(_) => {
                tracing::debug!(
                    contribution_id = self.contribution.contribution_id.as_str(),
                    elapsed_us = started.elapsed().as_micros() as u64,
                    status = "read_failed",
                    "managed executable verification"
                );
                return Err(super::invalid("frozen managed executable is unavailable"));
            }
        };
        let matches = extension_contracts::ManagedArtifactFingerprint::from_bytes(&bytes)
            == self.executable_fingerprint;
        tracing::debug!(
            contribution_id = self.contribution.contribution_id.as_str(),
            bytes = bytes.len(),
            elapsed_us = started.elapsed().as_micros() as u64,
            status = if matches {
                "verified"
            } else {
                "fingerprint_mismatch"
            },
            "managed executable verification"
        );
        if !matches {
            return Err(super::invalid("frozen managed executable bytes changed"));
        }
        Ok(())
    }
}
