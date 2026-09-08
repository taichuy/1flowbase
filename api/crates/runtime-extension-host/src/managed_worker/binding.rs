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
    pub contribution: extension_contracts::extension_bus::ContributionDescriptor,
}

impl LoadedManagedBinding {
    /// Runs on the host's blocking pool before admission; the installed path is never resolved
    /// again through a current/latest installation. Removed or replaced bytes fail closed.
    /// The artifact owner must keep published files immutable: this check is not synchronization
    /// with an unrelated filesystem writer racing the subsequent process spawn.
    pub(crate) fn verify_executable(&self) -> extension_package_runtime::FrameworkResult<()> {
        let bytes = std::fs::read(&self.runtime_executable)
            .map_err(|_| super::invalid("frozen managed executable is unavailable"))?;
        if extension_contracts::ManagedArtifactFingerprint::from_bytes(&bytes)
            != self.executable_fingerprint
        {
            return Err(super::invalid("frozen managed executable bytes changed"));
        }
        Ok(())
    }
}
