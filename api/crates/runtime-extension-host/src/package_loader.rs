use std::{
    fs,
    path::{Path, PathBuf},
};

use extension_package_runtime::{
    capability_kind::PluginConsumptionKind,
    data_source_package::DataSourcePackage,
    error::{FrameworkResult, PluginFrameworkError},
    manifest_v1::{PluginExecutionMode, PluginManifestV1},
    provider_package::ProviderPackage,
    LegacyInstalledManifestEligibility,
};

#[derive(Debug, Clone)]
pub struct LoadedProviderPackage {
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "retained only for package loader fixture identity assertions"
    )]
    pub package_root: PathBuf,
    pub runtime_executable: PathBuf,
    pub package: ProviderPackage,
}

#[derive(Debug, Clone)]
pub struct LoadedDataSourcePackage {
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "retained only for package loader fixture identity assertions"
    )]
    pub package_root: PathBuf,
    pub runtime_executable: PathBuf,
    pub package: DataSourcePackage,
}

#[derive(Debug, Clone)]
pub struct LoadedProviderDistributionPackage {
    pub runtime_executable: PathBuf,
    pub manifest: PluginManifestV1,
}

pub struct PackageLoader;

impl PackageLoader {
    pub(crate) fn load_managed(
        package_root: impl AsRef<Path>,
        request: &runtime_core::runtime_backend::RuntimeManagedActivation,
    ) -> FrameworkResult<crate::managed_worker::LoadedManagedBinding> {
        use extension_contracts::extension_bus::ManagedArtifactFingerprint;
        let package_root = fs::canonicalize(package_root.as_ref()).map_err(|error| {
            PluginFrameworkError::io(Some(package_root.as_ref()), error.to_string())
        })?;
        let manifest_path = package_root.join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| PluginFrameworkError::io(Some(&manifest_path), error.to_string()))?;
        if &ManagedArtifactFingerprint::from_bytes(manifest_raw.as_bytes())
            != request.identity.artifact_fingerprint()
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed artifact fingerprint does not match the bound installation",
            ));
        }
        let manifest = extension_package_runtime::parse_plugin_manifest(&manifest_raw)?;
        if manifest.versioned_plugin_id()? != request.plugin_id {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed artifact plugin identity does not match activation",
            ));
        }
        let managed = manifest.managed.as_ref().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_package(
                "managed activation requires an explicit managed manifest",
            )
        })?;
        if &managed.execution_binding_fingerprint(request.identity.contribution_id())?
            != request.identity.binding_fingerprint()
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed contribution binding fingerprint does not match activation",
            ));
        }
        let binding = managed
            .execution_bindings
            .iter()
            .find(|binding| &binding.contribution_id == request.identity.contribution_id())
            .ok_or_else(|| {
                PluginFrameworkError::invalid_provider_package(
                    "managed contribution has no execution binding",
                )
            })?;
        if !matches!(
            binding.execution_mode,
            PluginExecutionMode::ProcessPerCall | PluginExecutionMode::DeclarativeOnly
        ) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed execution mode does not have an installed runtime adapter",
            ));
        }
        let runtime_executable = fs::canonicalize(package_root.join(&binding.runtime.entry))
            .map_err(|error| PluginFrameworkError::io(Some(&package_root), error.to_string()))?;
        if !runtime_executable.starts_with(&package_root) || !runtime_executable.is_file() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed execution entry must be a file inside the installed artifact",
            ));
        }
        Ok(crate::managed_worker::LoadedManagedBinding {
            plugin_id: request.plugin_id.clone(),
            runtime_executable,
            execution_mode: binding.execution_mode,
            limits: binding.runtime.limits.clone(),
            handler: binding.handler.clone(),
            contribution: managed
                .module
                .contributions
                .iter()
                .find(|contribution| {
                    &contribution.contribution_id == request.identity.contribution_id()
                })
                .ok_or_else(|| {
                    PluginFrameworkError::invalid_provider_package(
                        "managed contribution declaration missing",
                    )
                })?
                .clone(),
        })
    }

    pub fn load_provider_distribution(
        package_root: impl AsRef<Path>,
    ) -> FrameworkResult<LoadedProviderDistributionPackage> {
        let package_root = fs::canonicalize(package_root.as_ref()).map_err(|error| {
            PluginFrameworkError::invalid_provider_package(format!(
                "cannot resolve package root: {error}"
            ))
        })?;
        let manifest_path = package_root.join("manifest.yaml");
        let manifest_raw = fs::read_to_string(&manifest_path)
            .map_err(|error| PluginFrameworkError::io(Some(&manifest_path), error.to_string()))?;
        let manifest = extension_package_runtime::parse_plugin_manifest(&manifest_raw)?;
        if manifest.managed.is_some() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed packages require contribution-bound runtime activation",
            ));
        }
        if manifest.provider_distribution_rules.len() != 1 {
            return Err(PluginFrameworkError::invalid_provider_package(
                "provider distribution package must declare exactly one rule",
            ));
        }
        let runtime_executable = package_root.join(&manifest.runtime.entry);
        if !runtime_executable.is_file() {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "provider distribution runtime entry does not exist: {}",
                runtime_executable.display()
            )));
        }
        Ok(LoadedProviderDistributionPackage {
            runtime_executable,
            manifest,
        })
    }

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
            #[cfg(test)]
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
            #[cfg(test)]
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
            Some(eligibility) => extension_package_runtime::parse_legacy_installed_plugin_manifest(
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
            #[cfg(test)]
            package_root,
            runtime_executable,
            manifest,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LoadedCapabilityPackage {
    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "retained only for package loader fixture identity assertions"
    )]
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
    let manifest = extension_package_runtime::parse_plugin_manifest(raw)?;
    validate_capability_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_capability_manifest(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    if manifest.managed.is_some() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "managed packages require contribution-bound runtime activation",
        ));
    }

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
