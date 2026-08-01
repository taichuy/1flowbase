use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedExtensionBootstrapEntry {
    pub category: ExtensionCatalogCategory,
    pub artifact_kind: String,
    pub id: String,
    pub version: String,
    pub checksum: String,
    pub source: String,
    pub artifact_url: String,
    pub installed_path: String,
    pub bundled_path: String,
    pub bootstrap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionBootstrapWarning {
    pub extension_id: String,
    pub version: String,
    pub stage: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionBootstrapDisposition {
    DatabaseInitialized,
    LocalArtifactPresent,
    InstalledFromBundle,
    InstalledFromRemote,
    NotRequested,
    Warned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionBootstrapResult {
    pub extension_id: String,
    pub version: String,
    pub application: TypedExtensionApplication,
    pub disposition: ExtensionBootstrapDisposition,
    pub warning: Option<ExtensionBootstrapWarning>,
}

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository
        + FrontendBlockCatalogRepository,
    H: ProviderRuntimePort,
{
    pub async fn bootstrap_locked_extensions(
        &self,
        actor_user_id: Uuid,
        entries: &[LockedExtensionBootstrapEntry],
    ) -> Vec<ExtensionBootstrapResult> {
        let installations = match self.repository.list_installations().await {
            Ok(installations) => installations,
            Err(error) => {
                return entries
                    .iter()
                    .map(|entry| warned(entry, "inventory", error.to_string()))
                    .collect();
            }
        };

        let mut results = Vec::with_capacity(entries.len());
        for entry in entries {
            let result = self
                .bootstrap_locked_extension(actor_user_id, entry, &installations)
                .await;
            results.push(result);
        }
        results
    }

    async fn bootstrap_locked_extension(
        &self,
        actor_user_id: Uuid,
        entry: &LockedExtensionBootstrapEntry,
        installations: &[domain::PluginInstallationRecord],
    ) -> ExtensionBootstrapResult {
        if installations.iter().any(|installation| {
            installation.plugin_version == entry.version
                && (installation.metadata_json["official_plugin_id"] == entry.id
                    || installation.plugin_id == format!("{}@{}", entry.id, entry.version)
                    || entry
                        .id
                        .rsplit('.')
                        .next()
                        .is_some_and(|code| installation.provider_code == code))
        }) {
            return completed(entry, ExtensionBootstrapDisposition::DatabaseInitialized);
        }

        let installed_path = match safe_relative_path(&self.install_root, &entry.installed_path) {
            Ok(path) => path,
            Err(error) => return warned(entry, "manifest", error.to_string()),
        };
        if installed_path.exists() {
            // A present node artifact is authoritative, including a locally modified or
            // incomplete development artifact. Bootstrap never repairs or replaces it.
            return completed(entry, ExtensionBootstrapDisposition::LocalArtifactPresent);
        }
        if !entry.bootstrap {
            return completed(entry, ExtensionBootstrapDisposition::NotRequested);
        }

        let bundled_path = match safe_relative_path(&self.install_root, &entry.bundled_path) {
            Ok(path) => path,
            Err(error) => return warned(entry, "manifest", error.to_string()),
        };
        if bundled_path.is_file() {
            return match fs::read(&bundled_path) {
                Ok(package_bytes) => self
                    .install_locked_package(actor_user_id, entry, package_bytes, "bundle")
                    .await
                    .map(|_| completed(entry, ExtensionBootstrapDisposition::InstalledFromBundle))
                    .unwrap_or_else(|error| warned(entry, "bundle_install", error.to_string())),
                Err(error) => warned(entry, "bundle_read", error.to_string()),
            };
        }

        match self.download_locked_package(entry).await {
            Ok(package_bytes) => self
                .install_locked_package(actor_user_id, entry, package_bytes, "remote")
                .await
                .map(|_| completed(entry, ExtensionBootstrapDisposition::InstalledFromRemote))
                .unwrap_or_else(|error| warned(entry, "remote_install", error.to_string())),
            Err(error) => warned(entry, "remote_fetch", error.to_string()),
        }
    }

    async fn download_locked_package(
        &self,
        locked: &LockedExtensionBootstrapEntry,
    ) -> Result<Vec<u8>> {
        if locked.source != "official_registry" {
            return Err(ControlPlaneError::InvalidInput("extension_bootstrap_source").into());
        }
        let snapshot = self.official_source.list_official_catalog().await?;
        let entry = snapshot
            .entries
            .into_iter()
            .find(|candidate| candidate.plugin_id == locked.id)
            .ok_or(ControlPlaneError::NotFound("official_plugin"))?;
        if locked.category != ExtensionCatalogCategory::RuntimeExtensions
            || entry.plugin_type != locked.artifact_kind
            || entry.latest_version != locked.version
            || entry.selected_artifact.checksum != locked.checksum
            || entry.selected_artifact.download_url != locked.artifact_url
        {
            return Err(ControlPlaneError::Conflict("extension_bootstrap_lock_mismatch").into());
        }
        validate_official_plugin_compatibility_override(&entry, &self.host_version, None)?;
        Ok(self
            .official_source
            .download_plugin(&entry)
            .await?
            .package_bytes)
    }

    async fn install_locked_package(
        &self,
        actor_user_id: Uuid,
        locked: &LockedExtensionBootstrapEntry,
        package_bytes: Vec<u8>,
        source: &'static str,
    ) -> Result<()> {
        let intake = intake_package_bytes(
            &package_bytes,
            &PackageIntakePolicy {
                source_kind: locked.source.clone(),
                trust_mode: "signature_required".to_string(),
                expected_artifact_sha256: Some(locked.checksum.clone()),
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(format!("{}-{}.1flowbasepkg", locked.id, locked.version)),
            },
        )
        .await?;
        let package_kind = route_plugin_package(&intake.manifest)?;
        if locked.artifact_kind != package_kind.as_plugin_type()
            || intake.manifest.version != locked.version
        {
            return Err(ControlPlaneError::Conflict("extension_bootstrap_package_mismatch").into());
        }
        self.install_intake_result(
            actor_user_id,
            intake,
            Some(package_bytes),
            json!({
                "install_kind": "locked_bootstrap",
                "official_plugin_id": locked.id,
                "bootstrap_source": source,
                "domain_binding_owner": locked.category.application().binding_owner.as_str(),
            }),
        )
        .await?;
        Ok(())
    }
}

impl RoutedPluginPackageKind {
    fn as_plugin_type(self) -> &'static str {
        match self {
            Self::HostExtension => "host_extension",
            Self::ModelProviderRuntime => "model_provider",
            Self::DataSourceRuntime => "data_source",
            Self::CapabilityPlugin => "capability_plugin",
        }
    }
}

fn safe_relative_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(ControlPlaneError::InvalidInput("extension_bootstrap_path").into());
    }
    Ok(root.join(relative))
}

fn completed(
    entry: &LockedExtensionBootstrapEntry,
    disposition: ExtensionBootstrapDisposition,
) -> ExtensionBootstrapResult {
    ExtensionBootstrapResult {
        extension_id: entry.id.clone(),
        version: entry.version.clone(),
        application: entry.category.application(),
        disposition,
        warning: None,
    }
}

fn warned(
    entry: &LockedExtensionBootstrapEntry,
    stage: &'static str,
    message: String,
) -> ExtensionBootstrapResult {
    ExtensionBootstrapResult {
        extension_id: entry.id.clone(),
        version: entry.version.clone(),
        application: entry.category.application(),
        disposition: ExtensionBootstrapDisposition::Warned,
        warning: Some(ExtensionBootstrapWarning {
            extension_id: entry.id.clone(),
            version: entry.version.clone(),
            stage,
            message,
        }),
    }
}
