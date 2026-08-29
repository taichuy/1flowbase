use plugin_framework::{PluginConsumptionKind, PluginManifestV1};

use crate::errors::ControlPlaneError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutedPluginPackageKind {
    HostExtension,
    ModelProviderRuntime,
    DataSourceRuntime,
    NetworkEgressProviderRuntime,
    ProviderDistributionRuleRuntime,
    CapabilityPlugin,
}

impl RoutedPluginPackageKind {
    pub fn as_plugin_type(self) -> &'static str {
        match self {
            Self::HostExtension => "host_extension",
            Self::ModelProviderRuntime => "model_provider",
            Self::DataSourceRuntime => "data_source",
            Self::NetworkEgressProviderRuntime => "network_egress_provider",
            Self::ProviderDistributionRuleRuntime => "provider_distribution_rule",
            Self::CapabilityPlugin => "capability_plugin",
        }
    }
}

pub fn route_plugin_package(
    manifest: &PluginManifestV1,
) -> anyhow::Result<RoutedPluginPackageKind> {
    match manifest.consumption_kind {
        PluginConsumptionKind::HostExtension => Ok(RoutedPluginPackageKind::HostExtension),
        PluginConsumptionKind::CapabilityPlugin => Ok(RoutedPluginPackageKind::CapabilityPlugin),
        PluginConsumptionKind::RuntimeExtension => match manifest.slot_codes.as_slice() {
            [slot] if slot == "model_provider" => Ok(RoutedPluginPackageKind::ModelProviderRuntime),
            [slot] if slot == "data_source" => Ok(RoutedPluginPackageKind::DataSourceRuntime),
            [slot] if slot == "network_egress_provider" => {
                Ok(RoutedPluginPackageKind::NetworkEgressProviderRuntime)
            }
            [slot] if slot == "provider_distribution_rule" => {
                Ok(RoutedPluginPackageKind::ProviderDistributionRuleRuntime)
            }
            _ => Err(ControlPlaneError::InvalidInput("runtime_slot").into()),
        },
    }
}
