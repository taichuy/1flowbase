use plugin_framework::{PluginConsumptionKind, PluginManifestV1};

use crate::errors::ControlPlaneError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutedPluginPackageKind {
    HostExtension,
    ModelProviderRuntime,
    DataSourceRuntime,
    NetworkEgressProviderRuntime,
    CapabilityPlugin,
}

pub fn route_plugin_package(
    manifest: &PluginManifestV1,
) -> anyhow::Result<RoutedPluginPackageKind> {
    match manifest.consumption_kind {
        PluginConsumptionKind::HostExtension => Ok(RoutedPluginPackageKind::HostExtension),
        PluginConsumptionKind::CapabilityPlugin => Ok(RoutedPluginPackageKind::CapabilityPlugin),
        PluginConsumptionKind::RuntimeExtension => {
            if manifest
                .slot_codes
                .iter()
                .any(|slot| slot == "model_provider")
            {
                return Ok(RoutedPluginPackageKind::ModelProviderRuntime);
            }
            if manifest.slot_codes.iter().any(|slot| slot == "data_source") {
                return Ok(RoutedPluginPackageKind::DataSourceRuntime);
            }
            if manifest
                .slot_codes
                .iter()
                .any(|slot| slot == "network_egress_provider")
            {
                return Ok(RoutedPluginPackageKind::NetworkEgressProviderRuntime);
            }
            Err(ControlPlaneError::InvalidInput("runtime_slot").into())
        }
    }
}
