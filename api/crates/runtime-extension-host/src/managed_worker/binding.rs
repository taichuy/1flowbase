use std::path::PathBuf;

use extension_package_runtime::{PluginExecutionMode, PluginRuntimeLimits};

/// Exact, validated executable selection. The package-level runtime is never consulted here.
#[derive(Debug, Clone)]
pub(crate) struct LoadedManagedBinding {
    pub plugin_id: String,
    pub runtime_executable: PathBuf,
    pub execution_mode: PluginExecutionMode,
    pub limits: PluginRuntimeLimits,
    pub handler: String,
    pub contribution: extension_contracts::extension_bus::ContributionDescriptor,
}
