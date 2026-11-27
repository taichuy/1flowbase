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
    pub identity: LoadedRuntimeArtifactIdentity,
    #[cfg(test)]
    pub package_root: PathBuf,
    pub runtime_executable: PathBuf,
    pub package: ProviderPackage,
}

#[derive(Debug, Clone)]
pub struct LoadedDataSourcePackage {
    pub identity: LoadedRuntimeArtifactIdentity,
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
    pub identity: LoadedRuntimeArtifactIdentity,
    pub runtime_executable: PathBuf,
    pub manifest: PluginManifestV1,
}

pub struct PackageLoader;

use runtime_core::runtime_backend::{LoadedRuntimeArtifactIdentity, RuntimeArtifactMarkerIdentity};

const ARTIFACT_MARKER_FILE: &str = ".1flowbase-artifact.json";

fn artifact_marker(package_root: &Path) -> FrameworkResult<Option<RuntimeArtifactMarkerIdentity>> {
    let path = package_root.join(ARTIFACT_MARKER_FILE);
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| PluginFrameworkError::serialization(Some(&path), error.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PluginFrameworkError::io(Some(&path), error.to_string())),
    }
}

pub(crate) fn artifact_identity(
    package_root: &Path,
    manifest: &PluginManifestV1,
    manifest_fingerprint: String,
) -> FrameworkResult<LoadedRuntimeArtifactIdentity> {
    Ok(LoadedRuntimeArtifactIdentity {
        manifest_fingerprint,
        plugin_version: manifest.version.clone(),
        marker: artifact_marker(package_root)?,
    })
}

fn verify_manifest_fingerprint(actual: &str, expected: Option<&str>) -> FrameworkResult<()> {
    if expected.is_some_and(|expected| expected != actual) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "runtime manifest fingerprint does not match the bound installation",
        ));
    }
    Ok(())
}

pub(crate) fn cached_identity_matches(
    package_root: &Path,
    identity: &LoadedRuntimeArtifactIdentity,
    expected: Option<&str>,
) -> FrameworkResult<bool> {
    let path = package_root.join("manifest.yaml");
    let raw = fs::read(&path)
        .map_err(|error| PluginFrameworkError::io(Some(&path), error.to_string()))?;
    let actual = extension_package_runtime::canonical_manifest_fingerprint(&raw);
    verify_manifest_fingerprint(&actual, expected)?;
    Ok(
        actual == identity.manifest_fingerprint
            && artifact_marker(package_root)? == identity.marker,
    )
}

pub(crate) fn verify_package_activation(
    plugin_id: &str,
    expected_plugin_id: &str,
    identity: &LoadedRuntimeArtifactIdentity,
    expected_fingerprint: Option<&str>,
) -> FrameworkResult<()> {
    if plugin_id != expected_plugin_id {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "loaded package id {plugin_id} does not match requested {expected_plugin_id}"
        )));
    }
    verify_manifest_fingerprint(&identity.manifest_fingerprint, expected_fingerprint)
}

type ManagedManifestEntry = (
    extension_contracts::extension_bus::ManagedArtifactFingerprint,
    std::sync::Arc<PluginManifestV1>,
);

/// Activation arrives once per contribution with the same verified manifest bytes. Parsing is
/// a pure function of those bytes, so the last result is reused by digest (one entry).
static LAST_MANAGED_MANIFEST: std::sync::Mutex<Option<ManagedManifestEntry>> =
    std::sync::Mutex::new(None);

pub(crate) fn parse_managed_manifest(
    fingerprint: &extension_contracts::extension_bus::ManagedArtifactFingerprint,
    manifest_raw: &str,
) -> FrameworkResult<std::sync::Arc<PluginManifestV1>> {
    parse_managed_manifest_with(&LAST_MANAGED_MANIFEST, fingerprint, manifest_raw)
}

pub(crate) fn parse_managed_manifest_with(
    last: &std::sync::Mutex<Option<ManagedManifestEntry>>,
    fingerprint: &extension_contracts::extension_bus::ManagedArtifactFingerprint,
    manifest_raw: &str,
) -> FrameworkResult<std::sync::Arc<PluginManifestV1>> {
    let cached = last
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .filter(|(digest, _)| digest == fingerprint)
        .map(|(_, manifest)| std::sync::Arc::clone(manifest));
    if let Some(manifest) = cached {
        return Ok(manifest);
    }
    let manifest = std::sync::Arc::new(extension_package_runtime::parse_plugin_manifest(
        manifest_raw,
    )?);
    *last
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some((fingerprint.clone(), std::sync::Arc::clone(&manifest)));
    Ok(manifest)
}

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
        let manifest =
            parse_managed_manifest(request.identity.artifact_fingerprint(), &manifest_raw)?;
        if manifest.versioned_plugin_id()? != request.plugin_id {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed artifact plugin identity does not match activation",
            ));
        }
        if artifact_marker(&package_root)?.is_some_and(|marker| {
            marker.plugin_id != request.plugin_id
                || marker.version != manifest.version
                || marker.manifest_fingerprint.as_deref()
                    != Some(request.identity.artifact_fingerprint().as_str())
        }) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "managed artifact marker identity does not match activation",
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
        let (executable_fingerprint, _) = fs::File::open(&runtime_executable)
            .and_then(ManagedArtifactFingerprint::from_reader)
            .map_err(|error| {
                PluginFrameworkError::io(Some(&runtime_executable), error.to_string())
            })?;
        Ok(crate::managed_worker::LoadedManagedBinding {
            plugin_id: request.plugin_id.clone(),
            publisher_namespace: manifest.publisher_namespace.clone(),
            plugin_code: manifest.plugin_code()?.to_string(),
            plugin_version: manifest.version.clone(),
            protocol: binding.runtime.protocol.clone(),
            executable_fingerprint,
            runtime_executable,
            execution_mode: binding.execution_mode,
            limits: binding.runtime.limits.clone(),
            handler: binding.handler.clone(),
            worker_retention: binding
                .runtime
                .capabilities
                .iter()
                .any(|value| value == extension_contracts::RUNTIME_WORKER_RETENTION_V1),
            worker_idle_timeout_ms: manifest.managed_service.as_ref().and_then(|service| {
                (service.worker_lifetime.is_shared()
                    && service.operations.iter().any(|op| {
                        op.contribution_id == request.identity.contribution_id().as_str()
                    }))
                .then_some(service.worker_idle_timeout_ms.unwrap_or(1_800_000))
            }),
            actor_workspace_scope: manifest.managed_service.as_ref().is_some_and(|service| {
                service.worker_lifetime
                    == extension_package_runtime::ManagedServiceWorkerLifetime::ActorWorkspaceScope
                    && service
                        .operations
                        .iter()
                        .any(|op| op.contribution_id == request.identity.contribution_id().as_str())
            }),
            interface_protocol: binding.interface_protocol,
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
            identity: artifact_identity(
                &package_root,
                &manifest,
                extension_package_runtime::canonical_manifest_fingerprint(manifest_raw.as_bytes()),
            )?,
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
            identity: artifact_identity(
                &package_root,
                &package.manifest,
                package.manifest_fingerprint.clone(),
            )?,
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
            identity: artifact_identity(
                &package_root,
                &package.manifest,
                package.manifest_fingerprint.clone(),
            )?,
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
            identity: artifact_identity(
                &package_root,
                &manifest,
                extension_package_runtime::canonical_manifest_fingerprint(manifest_raw.as_bytes()),
            )?,
            package_root,
            runtime_executable,
            manifest,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LoadedCapabilityPackage {
    pub identity: LoadedRuntimeArtifactIdentity,
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
