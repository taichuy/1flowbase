use super::*;
use super::{
    filesystem::copy_installation_artifact,
    install::{load_plugin_manifest, map_framework_error},
};
use serde::{Deserialize, Serialize};

const ARTIFACT_MARKER_FILE: &str = ".1flowbase-artifact.json";

#[derive(Debug, Clone)]
pub struct RefreshCurrentNodePluginArtifactCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct InstallCurrentNodePluginArtifactCommand {
    pub actor_user_id: Uuid,
    pub installation_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
struct PluginArtifactMarker {
    plugin_id: String,
    version: String,
    checksum: Option<String>,
    manifest_fingerprint: Option<String>,
    #[serde(default)]
    source_kind: Option<String>,
    #[serde(default)]
    trust_level: Option<String>,
    #[serde(default)]
    signature_status: Option<String>,
    #[serde(default)]
    signature_algorithm: Option<String>,
    #[serde(default)]
    signing_key_id: Option<String>,
}

#[derive(Debug)]
struct ScannedPluginArtifact {
    local_version: Option<String>,
    local_checksum: Option<String>,
    installed_path: Option<String>,
    artifact_status: domain::PluginArtifactInstanceStatus,
    last_error: Option<String>,
}

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository + PluginRepository,
    H: ProviderRuntimePort,
{
    pub async fn rebuild_inventory_from_local_receipts(
        &self,
        actor_user_id: Uuid,
    ) -> Result<usize> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_root_actor(&actor)?;
        let existing = self
            .repository
            .list_installations()
            .await?
            .into_iter()
            .map(|installation| (installation.plugin_id, installation.plugin_version))
            .collect::<HashSet<_>>();
        let installed_root = self.install_root.join("installed");
        let mut rebuilt = 0usize;
        let Ok(families) = fs::read_dir(&installed_root) else {
            return Ok(0);
        };
        for family in families.filter_map(|entry| entry.ok()) {
            let Ok(versions) = fs::read_dir(family.path()) else {
                continue;
            };
            for version in versions.filter_map(|entry| entry.ok()) {
                let install_path = version.path();
                if !install_path.is_dir() {
                    continue;
                }
                let Ok(receipt) = read_artifact_marker(&install_path) else {
                    continue;
                };
                if existing.contains(&(receipt.plugin_id.clone(), receipt.version.clone())) {
                    continue;
                }
                let Ok(manifest) = load_plugin_manifest(&install_path) else {
                    continue;
                };
                let package_id = manifest
                    .versioned_plugin_id()
                    .map_err(map_framework_error)?;
                if package_id != receipt.plugin_id || manifest.version != receipt.version {
                    continue;
                }
                let package_kind = route_plugin_package(&manifest)?;
                let plugin_type = match package_kind {
                    RoutedPluginPackageKind::HostExtension => "host_extension",
                    RoutedPluginPackageKind::ModelProviderRuntime => "model_provider",
                    RoutedPluginPackageKind::DataSourceRuntime => "data_source",
                    RoutedPluginPackageKind::CapabilityPlugin => "capability_plugin",
                };
                let desired_state = if package_kind == RoutedPluginPackageKind::HostExtension {
                    domain::PluginDesiredState::PendingRestart
                } else {
                    domain::PluginDesiredState::Disabled
                };
                let availability_status = if package_kind == RoutedPluginPackageKind::HostExtension
                {
                    domain::PluginAvailabilityStatus::PendingRestart
                } else {
                    domain::PluginAvailabilityStatus::Disabled
                };
                let signature_is_valid = matches!(
                    receipt.signature_status.as_deref(),
                    Some("verified" | "builtin")
                );
                let installation = self
                    .repository
                    .upsert_installation(&UpsertPluginInstallationInput {
                        installation_id: Uuid::now_v7(),
                        provider_code: manifest
                            .plugin_code()
                            .map_err(map_framework_error)?
                            .to_string(),
                        plugin_id: receipt.plugin_id,
                        plugin_version: receipt.version,
                        contract_version: manifest.contract_version,
                        protocol: manifest.runtime.protocol,
                        display_name: manifest.display_name,
                        source_kind: receipt
                            .source_kind
                            .unwrap_or_else(|| manifest.source_kind.clone()),
                        trust_level: receipt
                            .trust_level
                            .unwrap_or_else(|| manifest.trust_level.clone()),
                        verification_status: if signature_is_valid {
                            domain::PluginVerificationStatus::Valid
                        } else {
                            domain::PluginVerificationStatus::Invalid
                        },
                        desired_state,
                        artifact_status: domain::PluginArtifactStatus::Ready,
                        runtime_status: domain::PluginRuntimeStatus::Inactive,
                        availability_status,
                        package_path: None,
                        installed_path: install_path.display().to_string(),
                        checksum: receipt.checksum,
                        manifest_fingerprint: receipt.manifest_fingerprint,
                        signature_status: receipt.signature_status,
                        signature_algorithm: receipt.signature_algorithm,
                        signing_key_id: receipt.signing_key_id,
                        last_load_error: None,
                        metadata_json: json!({
                            "install_kind": "local_receipt_rebuild",
                            "plugin_type": plugin_type,
                        }),
                        actor_user_id,
                    })
                    .await?;
                self.refresh_current_node_artifact_snapshot(&installation)
                    .await?;
                rebuilt = rebuilt.saturating_add(1);
            }
        }
        Ok(rebuilt)
    }

    pub async fn refresh_current_node_artifact(
        &self,
        command: RefreshCurrentNodePluginArtifactCommand,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&installation)?;

        self.refresh_current_node_artifact_snapshot(&installation)
            .await
    }

    pub async fn install_current_node_artifact(
        &self,
        command: InstallCurrentNodePluginArtifactCommand,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.configure.all")
            .await?;
        let installation = self
            .repository
            .get_installation(command.installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        self.ensure_model_provider_target(&installation)?;

        // Installed files are the node truth. Refreshing or repairing metadata must never
        // replace a present local development artifact with a registry download.
        let local_artifact = self
            .refresh_current_node_artifact_snapshot(&installation)
            .await?;
        if let Some(warning) = local_artifact.last_error.as_deref() {
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "plugin_installation",
                    Some(installation.id),
                    "plugin.local_artifact_warning",
                    json!({
                        "warning": warning,
                        "node_id": self.node_id,
                        "remote_io": false,
                    }),
                ))
                .await?;
        }
        if local_artifact.installed_path.is_some() && local_artifact.artifact_status.is_ready() {
            return Ok(local_artifact);
        }

        let package_bytes = self
            .package_bytes_for_current_node_install(&installation)
            .await?;
        let intake = intake_package_bytes(
            &package_bytes,
            &PackageIntakePolicy {
                source_kind: installation.source_kind.clone(),
                trust_mode: "allow_unsigned".to_string(),
                expected_artifact_sha256: installation.checksum.clone(),
                trusted_public_keys: self.official_source.trusted_public_keys(),
                original_filename: Some(format!(
                    "{}-{}.1flowbasepkg",
                    installation.provider_code, installation.plugin_version
                )),
            },
        )
        .await?;
        let package_root = intake.extracted_root.clone();
        let install_result = async {
            let manifest = load_plugin_manifest(&package_root)?;
            let package_id = manifest
                .versioned_plugin_id()
                .map_err(map_framework_error)?;
            if package_id != installation.plugin_id
                || manifest.version != installation.plugin_version
            {
                return Err(ControlPlaneError::Conflict("plugin_artifact_mismatched").into());
            }
            let install_path =
                expected_current_node_artifact_path(&self.install_root, &installation);
            copy_installation_artifact(&package_root, &install_path)?;
            let manifest_fingerprint =
                compute_manifest_fingerprint(&install_path.join("manifest.yaml"))
                    .await
                    .map_err(map_framework_error)?;
            if let Some(expected) = installation.manifest_fingerprint.as_deref() {
                if expected != manifest_fingerprint {
                    return Err(ControlPlaneError::Conflict("plugin_artifact_mismatched").into());
                }
            }
            let checksum = intake
                .checksum
                .as_deref()
                .or(installation.checksum.as_deref());
            write_artifact_marker(
                &install_path,
                &installation.plugin_id,
                &installation.plugin_version,
                checksum,
                Some(&manifest_fingerprint),
                Some(&installation.source_kind),
                Some(&installation.trust_level),
                installation.signature_status.as_deref(),
                installation.signature_algorithm.as_deref(),
                installation.signing_key_id.as_deref(),
            )?;
            self.refresh_current_node_artifact_snapshot(&installation)
                .await
        }
        .await;
        let _ = fs::remove_dir_all(&package_root);
        install_result
    }

    pub(super) async fn refresh_current_node_artifact_snapshot(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        refresh_current_node_plugin_artifact_instance(
            &self.repository,
            &self.node_id,
            &self.install_root,
            installation,
        )
        .await
    }

    pub(super) async fn record_ready_current_node_artifact(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        self.repository
            .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
                node_id: self.node_id.clone(),
                installation_id: installation.id,
                local_version: Some(installation.plugin_version.clone()),
                local_checksum: installation.checksum.clone(),
                installed_path: Some(installation.installed_path.clone()),
                artifact_status: domain::PluginArtifactInstanceStatus::Ready,
                runtime_status: domain::PluginRuntimeStatus::Inactive,
                checked_at: OffsetDateTime::now_utc(),
                last_error: None,
            })
            .await
    }

    pub(super) async fn ready_current_node_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::PluginInstallationRecord> {
        let installation = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let artifact = self
            .refresh_current_node_artifact_snapshot(&installation)
            .await?;
        if !artifact.artifact_status.is_ready() {
            return Err(ControlPlaneError::Conflict(error_code_for_artifact_status(
                artifact.artifact_status,
            ))
            .into());
        }
        let installed_path = artifact
            .installed_path
            .ok_or(ControlPlaneError::Conflict("plugin_artifact_missing"))?;
        let mut local_installation = installation;
        local_installation.installed_path = installed_path;
        local_installation.artifact_status = domain::PluginArtifactStatus::Ready;
        local_installation.runtime_status = artifact.runtime_status;
        Ok(local_installation)
    }

    pub(super) async fn mark_current_node_runtime_status(
        &self,
        installation: &domain::PluginInstallationRecord,
        runtime_status: domain::PluginRuntimeStatus,
        last_error: Option<String>,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        mark_current_node_plugin_runtime_status(
            &self.repository,
            &self.node_id,
            installation,
            runtime_status,
            last_error,
        )
        .await
    }

    async fn package_bytes_for_current_node_install(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<Vec<u8>> {
        if let Some(package_path) = installation.package_path.as_deref() {
            let path = Path::new(package_path);
            if path.is_file() {
                return fs::read(path).with_context(|| {
                    format!(
                        "failed to read plugin package archive at {}",
                        path.display()
                    )
                });
            }
        }

        let official_snapshot = self.official_source.list_official_catalog().await?;
        if let Some(entry) = official_snapshot.entries.into_iter().find(|entry| {
            entry.provider_code == installation.provider_code
                && entry.latest_version == installation.plugin_version
        }) {
            return Ok(self
                .official_source
                .download_plugin(&entry)
                .await?
                .package_bytes);
        }

        Err(ControlPlaneError::Conflict("plugin_artifact_package_unavailable").into())
    }
}

pub async fn refresh_current_node_plugin_artifact_instance<R>(
    repository: &R,
    node_id: &str,
    install_root: &Path,
    installation: &domain::PluginInstallationRecord,
) -> Result<domain::PluginArtifactInstanceRecord>
where
    R: PluginRepository + ?Sized,
{
    let scanned = scan_current_node_artifact_at(install_root, installation).await;
    let existing = repository
        .get_artifact_instance(node_id, installation.id)
        .await?;
    let runtime_status = existing
        .filter(|existing| {
            existing.artifact_status.is_ready()
                && scanned.artifact_status.is_ready()
                && existing.local_version == scanned.local_version
                && existing.local_checksum == scanned.local_checksum
                && existing.installed_path == scanned.installed_path
        })
        .map(|existing| existing.runtime_status)
        .unwrap_or(domain::PluginRuntimeStatus::Inactive);
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: node_id.to_string(),
            installation_id: installation.id,
            local_version: scanned.local_version,
            local_checksum: scanned.local_checksum,
            installed_path: scanned.installed_path,
            artifact_status: scanned.artifact_status,
            runtime_status,
            checked_at: OffsetDateTime::now_utc(),
            last_error: scanned.last_error,
        })
        .await
}

pub async fn ready_current_node_plugin_installation<R>(
    repository: &R,
    node_id: &str,
    install_root: &Path,
    installation_id: Uuid,
) -> Result<domain::PluginInstallationRecord>
where
    R: PluginRepository + ?Sized,
{
    let installation = repository
        .get_installation(installation_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    if let Some(existing) = repository
        .get_artifact_instance(node_id, installation_id)
        .await?
    {
        if load_failed_snapshot_matches_installation(&existing, &installation, install_root) {
            return Err(ControlPlaneError::Conflict("plugin_runtime_load_failed").into());
        }
    }
    let artifact = refresh_current_node_plugin_artifact_instance(
        repository,
        node_id,
        install_root,
        &installation,
    )
    .await?;
    if !artifact.artifact_status.is_ready() {
        return Err(ControlPlaneError::Conflict(error_code_for_artifact_status(
            artifact.artifact_status,
        ))
        .into());
    }
    let installed_path = artifact
        .installed_path
        .ok_or(ControlPlaneError::Conflict("plugin_artifact_missing"))?;
    let mut local_installation = installation;
    local_installation.installed_path = installed_path;
    local_installation.artifact_status = domain::PluginArtifactStatus::Ready;
    local_installation.runtime_status = artifact.runtime_status;
    Ok(local_installation)
}

pub async fn mark_current_node_plugin_runtime_status<R>(
    repository: &R,
    node_id: &str,
    installation: &domain::PluginInstallationRecord,
    runtime_status: domain::PluginRuntimeStatus,
    last_error: Option<String>,
) -> Result<domain::PluginArtifactInstanceRecord>
where
    R: PluginRepository + ?Sized,
{
    let existing = repository
        .get_artifact_instance(node_id, installation.id)
        .await?;
    let artifact_status = match runtime_status {
        domain::PluginRuntimeStatus::LoadFailed => domain::PluginArtifactInstanceStatus::LoadFailed,
        _ => domain::PluginArtifactInstanceStatus::Ready,
    };
    let (local_version, local_checksum, installed_path) = existing
        .map(|record| {
            (
                record.local_version,
                record.local_checksum,
                record.installed_path,
            )
        })
        .unwrap_or_else(|| {
            (
                Some(installation.plugin_version.clone()),
                installation.checksum.clone(),
                Some(installation.installed_path.clone()),
            )
        });
    repository
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: node_id.to_string(),
            installation_id: installation.id,
            local_version,
            local_checksum,
            installed_path,
            artifact_status,
            runtime_status,
            checked_at: OffsetDateTime::now_utc(),
            last_error,
        })
        .await
}

fn load_failed_snapshot_matches_installation(
    artifact: &domain::PluginArtifactInstanceRecord,
    installation: &domain::PluginInstallationRecord,
    install_root: &Path,
) -> bool {
    if artifact.artifact_status != domain::PluginArtifactInstanceStatus::LoadFailed {
        return false;
    }
    if artifact.local_version.as_deref() != Some(installation.plugin_version.as_str()) {
        return false;
    }
    if let Some(expected_checksum) = installation.checksum.as_deref() {
        if artifact.local_checksum.as_deref() != Some(expected_checksum) {
            return false;
        }
    }
    let expected_path = expected_current_node_artifact_path(install_root, installation)
        .display()
        .to_string();
    artifact.installed_path.as_deref() == Some(expected_path.as_str())
}

async fn scan_current_node_artifact_at(
    install_root: &Path,
    installation: &domain::PluginInstallationRecord,
) -> ScannedPluginArtifact {
    let expected_path = expected_current_node_artifact_path(install_root, installation);
    if expected_path.is_dir() {
        return inspect_artifact_path(installation, &expected_path).await;
    }

    if let Some(stale_path) = find_any_local_version_path(install_root, installation) {
        let mut scanned = inspect_artifact_path(installation, &stale_path).await;
        if scanned.artifact_status.is_ready() {
            scanned.artifact_status = domain::PluginArtifactInstanceStatus::Outdated;
            scanned.last_error = Some("local_version_outdated".to_string());
        }
        return scanned;
    }

    ScannedPluginArtifact {
        local_version: None,
        local_checksum: None,
        installed_path: None,
        artifact_status: domain::PluginArtifactInstanceStatus::Missing,
        last_error: Some("artifact_missing".to_string()),
    }
}

fn expected_current_node_artifact_path(
    install_root: &Path,
    installation: &domain::PluginInstallationRecord,
) -> PathBuf {
    install_root
        .join("installed")
        .join(&installation.provider_code)
        .join(&installation.plugin_version)
}

fn find_any_local_version_path(
    install_root: &Path,
    installation: &domain::PluginInstallationRecord,
) -> Option<PathBuf> {
    let family_root = install_root
        .join("installed")
        .join(&installation.provider_code);
    let mut candidates = fs::read_dir(family_root)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.into_iter().rev().find(|path| {
        path.join(ARTIFACT_MARKER_FILE).is_file()
            || path.file_name().and_then(|name| name.to_str())
                == Some(installation.plugin_version.as_str())
    })
}

pub(super) fn write_artifact_marker(
    install_path: &Path,
    plugin_id: &str,
    version: &str,
    checksum: Option<&str>,
    manifest_fingerprint: Option<&str>,
    source_kind: Option<&str>,
    trust_level: Option<&str>,
    signature_status: Option<&str>,
    signature_algorithm: Option<&str>,
    signing_key_id: Option<&str>,
) -> Result<()> {
    let marker = PluginArtifactMarker {
        plugin_id: plugin_id.to_string(),
        version: version.to_string(),
        checksum: checksum.map(str::to_string),
        manifest_fingerprint: manifest_fingerprint.map(str::to_string),
        source_kind: source_kind.map(str::to_string),
        trust_level: trust_level.map(str::to_string),
        signature_status: signature_status.map(str::to_string),
        signature_algorithm: signature_algorithm.map(str::to_string),
        signing_key_id: signing_key_id.map(str::to_string),
    };
    let bytes = serde_json::to_vec_pretty(&marker)?;
    fs::write(install_path.join(ARTIFACT_MARKER_FILE), bytes).with_context(|| {
        format!(
            "failed to write plugin artifact marker at {}",
            install_path.join(ARTIFACT_MARKER_FILE).display()
        )
    })?;
    Ok(())
}

async fn inspect_artifact_path(
    installation: &domain::PluginInstallationRecord,
    path: &Path,
) -> ScannedPluginArtifact {
    let marker = match read_artifact_marker(path) {
        Ok(marker) => marker,
        Err(error) if error == "artifact_marker_missing" => {
            return ScannedPluginArtifact {
                local_version: None,
                local_checksum: None,
                installed_path: None,
                artifact_status: domain::PluginArtifactInstanceStatus::Missing,
                last_error: Some(error),
            };
        }
        Err(error) => {
            return ScannedPluginArtifact {
                local_version: None,
                local_checksum: None,
                installed_path: Some(path.display().to_string()),
                artifact_status: domain::PluginArtifactInstanceStatus::Corrupted,
                last_error: Some(error),
            };
        }
    };
    let base = ScannedPluginArtifact {
        local_version: Some(marker.version.clone()),
        local_checksum: marker.checksum.clone(),
        installed_path: Some(path.display().to_string()),
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        last_error: None,
    };

    if marker.version != installation.plugin_version {
        return ScannedPluginArtifact {
            artifact_status: domain::PluginArtifactInstanceStatus::Outdated,
            last_error: Some("local_version_outdated".to_string()),
            ..base
        };
    }
    if marker.plugin_id != installation.plugin_id {
        return ScannedPluginArtifact {
            artifact_status: domain::PluginArtifactInstanceStatus::Mismatched,
            last_error: Some("plugin_id_mismatch".to_string()),
            ..base
        };
    }
    match (&installation.checksum, &marker.checksum) {
        (Some(expected), Some(actual)) if expected != actual => {
            return ScannedPluginArtifact {
                artifact_status: domain::PluginArtifactInstanceStatus::Ready,
                last_error: Some("checksum_mismatch".to_string()),
                ..base
            };
        }
        (Some(_), None) => {
            return ScannedPluginArtifact {
                artifact_status: domain::PluginArtifactInstanceStatus::Ready,
                last_error: Some("checksum_missing".to_string()),
                ..base
            };
        }
        _ => {}
    }

    let manifest_path = path.join("manifest.yaml");
    let raw_manifest = match fs::read_to_string(&manifest_path) {
        Ok(raw_manifest) => raw_manifest,
        Err(_) => {
            return ScannedPluginArtifact {
                artifact_status: domain::PluginArtifactInstanceStatus::Corrupted,
                last_error: Some("manifest_missing".to_string()),
                ..base
            };
        }
    };
    if parse_plugin_manifest(&raw_manifest).is_err() {
        return ScannedPluginArtifact {
            artifact_status: domain::PluginArtifactInstanceStatus::Corrupted,
            last_error: Some("manifest_parse_failed".to_string()),
            ..base
        };
    }
    let actual_fingerprint = match compute_manifest_fingerprint(&manifest_path).await {
        Ok(fingerprint) => fingerprint,
        Err(_) => {
            return ScannedPluginArtifact {
                artifact_status: domain::PluginArtifactInstanceStatus::Corrupted,
                last_error: Some("manifest_fingerprint_unavailable".to_string()),
                ..base
            };
        }
    };
    if marker.manifest_fingerprint.as_deref() != Some(actual_fingerprint.as_str()) {
        return ScannedPluginArtifact {
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            last_error: Some("manifest_fingerprint_mismatch".to_string()),
            ..base
        };
    }
    if let Some(expected) = installation.manifest_fingerprint.as_deref() {
        if expected != actual_fingerprint {
            return ScannedPluginArtifact {
                artifact_status: domain::PluginArtifactInstanceStatus::Ready,
                last_error: Some("expected_manifest_fingerprint_mismatch".to_string()),
                ..base
            };
        }
    }

    base
}

fn read_artifact_marker(path: &Path) -> std::result::Result<PluginArtifactMarker, String> {
    let marker_path = path.join(ARTIFACT_MARKER_FILE);
    let raw =
        fs::read_to_string(&marker_path).map_err(|_| "artifact_marker_missing".to_string())?;
    serde_json::from_str(&raw).map_err(|_| "artifact_marker_corrupted".to_string())
}

fn error_code_for_artifact_status(status: domain::PluginArtifactInstanceStatus) -> &'static str {
    match status {
        domain::PluginArtifactInstanceStatus::Missing => "plugin_artifact_missing",
        domain::PluginArtifactInstanceStatus::Outdated => "plugin_artifact_outdated",
        domain::PluginArtifactInstanceStatus::Mismatched => "plugin_artifact_mismatched",
        domain::PluginArtifactInstanceStatus::Corrupted => "plugin_artifact_corrupted",
        domain::PluginArtifactInstanceStatus::LoadFailed => "plugin_runtime_load_failed",
        domain::PluginArtifactInstanceStatus::Ready => "plugin_artifact_ready",
    }
}
