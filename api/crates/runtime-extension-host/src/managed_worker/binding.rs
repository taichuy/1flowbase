use std::path::PathBuf;

use extension_package_runtime::{PluginExecutionMode, PluginRuntimeLimits};

/// Exact, validated executable selection. The package-level runtime is never consulted here.
#[derive(Debug, Clone)]
pub(crate) struct LoadedManagedBinding {
    pub plugin_id: String,
    pub runtime_executable: PathBuf,
    pub executable_fingerprint: extension_contracts::ManagedArtifactFingerprint,
    pub execution_mode: PluginExecutionMode,
    pub limits: PluginRuntimeLimits,
    pub handler: String,
    pub interface_protocol: Option<extension_contracts::ManagedInterfaceProtocol>,
    pub contribution: extension_contracts::extension_bus::ContributionDescriptor,
}

impl LoadedManagedBinding {
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
