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
    ManagedContributions,
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
            Self::ManagedContributions => "managed_contributions",
        }
    }
}

/// Public catalog classification, separate from the internal installation route.
/// Managed packages retain their declared consumption kind; legacy providers retain
/// their existing slot-specific catalog types.
pub fn catalog_plugin_type(manifest: &PluginManifestV1) -> anyhow::Result<&'static str> {
    let route = route_plugin_package(manifest)?;
    Ok(match route {
        RoutedPluginPackageKind::ManagedContributions => manifest.consumption_kind.as_str(),
        route => route.as_plugin_type(),
    })
}

pub fn route_plugin_package(
    manifest: &PluginManifestV1,
) -> anyhow::Result<RoutedPluginPackageKind> {
    if manifest.manifest_version == 2 {
        if manifest.managed.is_none() {
            return Err(ControlPlaneError::InvalidInput("managed_manifest").into());
        }
        return Ok(
            if manifest.consumption_kind == PluginConsumptionKind::HostExtension {
                RoutedPluginPackageKind::HostExtension
            } else {
                RoutedPluginPackageKind::ManagedContributions
            },
        );
    }
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
