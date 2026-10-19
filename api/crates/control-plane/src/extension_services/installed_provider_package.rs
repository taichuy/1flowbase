use anyhow::Result;
use plugin_framework::{LegacyInstalledManifestEligibility, ProviderPackage};

use crate::errors::ControlPlaneError;

const MISSING_PUBLISHER_NAMESPACE_V1: &str = "missing_publisher_namespace_v1";

/// Loads a persisted model-provider artifact using its complete durable identity.
/// New package intake must continue to use the strict path-only loader.
pub(crate) fn load_installed_provider_package(
    installation: &domain::LocalPluginInstallationRecord,
) -> Result<ProviderPackage> {
    let path = installation
        .local_path()
        .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?;
    let package = match installation.legacy_manifest_compatibility.as_deref() {
        None => ProviderPackage::load_from_dir(path),
        Some(MISSING_PUBLISHER_NAMESPACE_V1) => {
            let fingerprint = installation.artifact.manifest_fingerprint.clone().ok_or(
                ControlPlaneError::Conflict("plugin_manifest_fingerprint_missing"),
            )?;
            ProviderPackage::load_legacy_installed_from_dir(
                path,
                &LegacyInstalledManifestEligibility {
                    expected_publisher_namespace: installation.organization.clone(),
                    expected_versioned_plugin_id: installation.plugin_id.clone(),
                    expected_raw_manifest_fingerprint: fingerprint,
                },
            )
        }
        Some(_) => {
            return Err(
                ControlPlaneError::Conflict("plugin_manifest_compatibility_unsupported").into(),
            );
        }
    };
    package.map_err(map_provider_package_error)
}

fn map_provider_package_error(error: plugin_framework::PluginFrameworkError) -> anyhow::Error {
    use plugin_framework::PluginFrameworkErrorKind;

    match error.kind() {
        PluginFrameworkErrorKind::InvalidAssignment
        | PluginFrameworkErrorKind::InvalidProviderPackage
        | PluginFrameworkErrorKind::InvalidProviderContract
        | PluginFrameworkErrorKind::Serialization => {
            ControlPlaneError::InvalidInput("provider_package").into()
        }
        PluginFrameworkErrorKind::Io | PluginFrameworkErrorKind::RuntimeContract => {
            ControlPlaneError::UpstreamUnavailable("provider_runtime").into()
        }
    }
}
