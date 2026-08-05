use std::{
    fs,
    path::{Path, PathBuf},
};

use plugin_framework::{
    capability_kind::PluginConsumptionKind,
    data_source_package::DataSourcePackage,
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::{PluginExecutionMode, PluginManifestV1},
    provider_package::ProviderPackage,
    LegacyInstalledManifestEligibility,
};

#[derive(Debug, Clone)]
pub struct LoadedProviderPackage {
    pub package_root: PathBuf,
    pub runtime_executable: PathBuf,
    pub package: ProviderPackage,
}

#[derive(Debug, Clone)]
pub struct LoadedDataSourcePackage {
    pub package_root: PathBuf,
    pub runtime_executable: PathBuf,
    pub package: DataSourcePackage,
}

pub struct PackageLoader;

impl PackageLoader {
    pub fn load(package_root: impl AsRef<Path>) -> FrameworkResult<LoadedProviderPackage> {
        Self::load_provider_package(package_root, None)
    }

    pub fn load_legacy_installed(
        package_root: impl AsRef<Path>,
        eligibility: &LegacyInstalledManifestEligibility,
    ) -> FrameworkResult<LoadedProviderPackage> {
        Self::load_provider_package(package_root, Some(eligibility))
    }

    fn load_provider_package(
        package_root: impl AsRef<Path>,
        eligibility: Option<&LegacyInstalledManifestEligibility>,
    ) -> FrameworkResult<LoadedProviderPackage> {
        let package_root = fs::canonicalize(package_root.as_ref()).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "cannot resolve package root: {error}"
            ))
        })?;

        if Self::looks_like_source_tree(&package_root) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "provider package root looks like a source tree; load an installed or unpacked artifact instead",
            ));
        }

        let package = match eligibility {
            Some(eligibility) => {
                ProviderPackage::load_legacy_installed_from_dir(&package_root, eligibility)?
            }
            None => ProviderPackage::load_from_dir(&package_root)?,
        };
        let runtime_executable = package.runtime_entry();
        if !runtime_executable.is_file() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "provider runtime entry does not exist: {}",
                runtime_executable.display()
            )));
        }

        Ok(LoadedProviderPackage {
            package_root,
            runtime_executable,
            package,
        })
    }

    pub fn load_data_source(
        package_root: impl AsRef<Path>,
    ) -> FrameworkResult<LoadedDataSourcePackage> {
        Self::load_data_source_package(package_root, None)
    }

    pub fn load_legacy_installed_data_source(
        package_root: impl AsRef<Path>,
        eligibility: &LegacyInstalledManifestEligibility,
    ) -> FrameworkResult<LoadedDataSourcePackage> {
        Self::load_data_source_package(package_root, Some(eligibility))
    }

    fn load_data_source_package(
        package_root: impl AsRef<Path>,
        eligibility: Option<&LegacyInstalledManifestEligibility>,
    ) -> FrameworkResult<LoadedDataSourcePackage> {
        let package_root = fs::canonicalize(package_root.as_ref()).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "cannot resolve package root: {error}"
            ))
        })?;

        if Self::looks_like_source_tree(&package_root) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "data source package root looks like a source tree; load an installed or unpacked artifact instead",
            ));
        }

        let package = match eligibility {
            Some(eligibility) => {
                DataSourcePackage::load_legacy_installed_from_dir(&package_root, eligibility)?
            }
            None => DataSourcePackage::load_from_dir(&package_root)?,
        };
        let runtime_executable = package.runtime_entry();
        if !runtime_executable.is_file() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "data source runtime entry does not exist: {}",
                runtime_executable.display()
            )));
        }

        Ok(LoadedDataSourcePackage {
            package_root,
            runtime_executable,
            package,
        })
    }

    fn looks_like_source_tree(package_root: &Path) -> bool {
        package_root.join("demo").exists() || package_root.join("scripts").exists()
    }

    pub fn load_capability(
        package_root: impl AsRef<Path>,
    ) -> FrameworkResult<LoadedCapabilityPackage> {
        Self::load_capability_package(package_root, None)
    }

    pub fn load_legacy_installed_capability(
        package_root: impl AsRef<Path>,
        eligibility: &LegacyInstalledManifestEligibility,
    ) -> FrameworkResult<LoadedCapabilityPackage> {
        Self::load_capability_package(package_root, Some(eligibility))
    }

    fn load_capability_package(
        package_root: impl AsRef<Path>,
        eligibility: Option<&LegacyInstalledManifestEligibility>,
    ) -> FrameworkResult<LoadedCapabilityPackage> {
        let package_root = fs::canonicalize(package_root.as_ref()).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "cannot resolve package root: {error}"
            ))
        })?;

        if Self::looks_like_source_tree(&package_root) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "capability package root looks like a source tree; load an installed or unpacked artifact instead",
            ));
        }

        let manifest_path = package_root.join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| PluginFrameworkError::io(Some(&manifest_path), error.to_string()))?;
        let manifest = match eligibility {
            Some(eligibility) => plugin_framework::parse_legacy_installed_plugin_manifest(
                &manifest_raw,
                eligibility,
            )?,
            None => parse_capability_manifest(&manifest_raw)?,
        };
        if eligibility.is_some() {
            validate_capability_manifest(&manifest)?;
        }
        let runtime_executable = package_root.join(&manifest.runtime.entry);
        if !runtime_executable.is_file() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "capability runtime entry does not exist: {}",
                runtime_executable.display()
            )));
        }

        Ok(LoadedCapabilityPackage {
            package_root,
            runtime_executable,
            manifest,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LoadedCapabilityPackage {
    pub package_root: PathBuf,
    pub runtime_executable: PathBuf,
    pub manifest: PluginManifestV1,
}

impl LoadedCapabilityPackage {
    pub fn identifier(&self) -> String {
        self.manifest
            .versioned_plugin_id()
            .expect("capability package manifest identity is validated")
    }
}

fn parse_capability_manifest(raw: &str) -> FrameworkResult<PluginManifestV1> {
    let manifest = plugin_framework::parse_plugin_manifest(raw)?;
    validate_capability_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_capability_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    if manifest.consumption_kind != PluginConsumptionKind::CapabilityPlugin {
        return Err(PluginFrameworkError::invalid_provider_package(
            "capability package must declare consumption_kind=capability_plugin",
        ));
    }
    if manifest.execution_mode != PluginExecutionMode::ProcessPerCall {
        return Err(PluginFrameworkError::invalid_provider_package(
            "capability package must declare execution_mode=process_per_call",
        ));
    }
    if manifest.runtime.protocol != "stdio_json" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "capability package must declare runtime.protocol=stdio_json",
        ));
    }

    Ok(())
}
