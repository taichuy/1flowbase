use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginConsumptionKind {
    HostExtension,
    RuntimeExtension,
    CapabilityPlugin,
}

impl PluginConsumptionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HostExtension => "host_extension",
            Self::RuntimeExtension => "runtime_extension",
            Self::CapabilityPlugin => "capability_plugin",
        }
    }
}
