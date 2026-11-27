use super::*;
use super::{
    filesystem::copy_installation_artifact,
    install::{load_plugin_manifest, map_framework_error},
};
use control_plane_contracts::ports::ActivatePluginArtifactOutcome;
use serde::{Deserialize, Serialize};

pub(crate) const ARTIFACT_MARKER_FILE: &str = ".1flowbase-artifact.json";

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
pub(crate) struct ScannedPluginArtifact {
    pub(crate) manifest_fingerprint: Option<String>,
    pub(crate) local_version: Option<String>,
    pub(crate) local_checksum: Option<String>,
    pub(crate) local_path: Option<String>,
    pub(crate) artifact_status: domain::PluginArtifactInstanceStatus,
    pub(crate) last_error: Option<String>,
}

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository + PluginRepository,
    H: ProviderRuntimePort,
{
    async fn audit_local_artifact_warning(
        &self,
        actor: &domain::ActorContext,
        actor_user_id: Uuid,
        installation_id: Uuid,
        warning: Option<&str>,
    ) -> Result<()> {
        let Some(warning) = warning else {
            return Ok(());
        };
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(actor_user_id),
                "plugin_installation",
                Some(installation_id),
                "plugin.local_artifact_warning",
                json!({
                    "warning": warning,
                    "node_id": self.node_id,
                    "remote_io": false,
                }),
            ))
            .await
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

        let local_artifact = self
            .refresh_current_node_artifact_snapshot(&installation)
            .await?;
        self.audit_local_artifact_warning(
            &actor,
            command.actor_user_id,
            installation.id,
            local_artifact.last_error.as_deref(),
        )
        .await?;
        Ok(local_artifact)
    }

    pub async fn install_current_node_artifact(
        &self,
        command: InstallCurrentNodePluginArtifactCommand,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        let runtime_lock =
            super::family::node_installation_runtime_lock(&self.node_id, command.installation_id);
        let _lifecycle_guard = runtime_lock.lock().await;
        self.install_current_node_artifact_locked(command).await
    }

    pub(super) async fn install_current_node_artifact_locked(
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

        let was_active_on_node = self.current_node_runtime_is_active(installation.id).await?;
        // Installed files are the node truth. Refreshing or repairing metadata must never
        // replace a present local development artifact with a registry download.
        let local_artifact = self
            .refresh_current_node_artifact_snapshot(&installation)
            .await?;
        self.audit_local_artifact_warning(
            &actor,
            command.actor_user_id,
            installation.id,
            local_artifact.last_error.as_deref(),
        )
        .await?;
        if local_artifact.local_path.is_some() && local_artifact.artifact_status.is_ready() {
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
                expected_artifact_sha256: installation.expected_checksum.clone(),
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
            if let Some(expected) = local_artifact.manifest_fingerprint.as_deref() {
                if expected != manifest_fingerprint {
                    return Err(ControlPlaneError::Conflict("plugin_artifact_mismatched").into());
                }
            }
            let checksum = intake
                .checksum
                .as_deref()
                .or(installation.expected_checksum.as_deref());
            write_artifact_marker(
                &install_path,
                &installation.plugin_id,
                &installation.plugin_version,
                checksum,
                Some(&manifest_fingerprint),
                Some(&installation.source_kind),
                Some(&installation.trust_level),
                Some(installation.signature_status.as_str()),
                installation.signature_algorithm.as_deref(),
                installation.signing_key_id.as_deref(),
            )?;
            self.refresh_current_node_artifact_snapshot(&installation)
                .await
        }
        .await;
        let _ = fs::remove_dir_all(&package_root);
        let artifact = install_result?;
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::ActiveRequested
        ) && !is_host_extension_installation(&installation)
        {
            let local_installation = self
                .ready_current_node_installation(installation.id)
                .await?;
            match self.runtime.activate_plugin(&local_installation).await {
                Ok(loaded) => {
                    let error = match self
                        .try_mark_current_node_runtime_active(&local_installation, loaded.as_ref())
                        .await?
                    {
                        ActivatePluginArtifactOutcome::Activated(active) => return Ok(active),
                        ActivatePluginArtifactOutcome::Disabled => {
                            ControlPlaneError::Conflict("plugin_installation_disabled")
                        }
                        ActivatePluginArtifactOutcome::ArtifactInvalidated => {
                            ControlPlaneError::PluginUnavailable
                        }
                    };
                    self.deactivate_rejected_activation(&installation, was_active_on_node)
                        .await?;
                    return Err(error.into());
                }
                Err(error) => {
                    self.mark_current_node_runtime_status(
                        &installation,
                        domain::PluginRuntimeStatus::LoadFailed,
                        Some(error.to_string()),
                    )
                    .await?;
                    return Err(error);
                }
            }
        }
        Ok(artifact)
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

    pub(super) async fn reconcile_boot_artifact_snapshot(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        scan_and_refresh_current_node_artifact(
            &self.repository,
            &self.node_id,
            &self.install_root,
            installation,
            true,
        )
        .await
    }

    pub(super) async fn ready_current_node_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<domain::LocalPluginInstallationRecord> {
        let installation = self
            .repository
            .get_installation(installation_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
        let artifact = self
            .refresh_current_node_artifact_snapshot(&installation)
            .await?;
        if !artifact.artifact_status.is_ready()
            || artifact.local_path.is_none()
            || domain::has_local_artifact_warning(artifact.last_error.as_deref())
        {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        Ok(domain::LocalPluginInstallationRecord {
            installation,
            artifact,
        })
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

    pub(super) async fn current_node_runtime_is_active(
        &self,
        installation_id: Uuid,
    ) -> Result<bool> {
        Ok(self
            .repository
            .get_artifact_instance(&self.node_id, installation_id)
            .await?
            .is_some_and(|artifact| artifact.runtime_status == domain::PluginRuntimeStatus::Active))
    }

    pub(super) async fn deactivate_rejected_activation(
        &self,
        installation: &domain::PluginInstallationRecord,
        was_active_on_node: bool,
    ) -> Result<()> {
        // An Inactive projection does not imply an unloaded worker. References from any
        // workspace protect it even if the projection was invalidated during verification.
        // A committed disable can supersede the previous Active observation.
        let was_active_and_still_requested = was_active_on_node
            && self
                .repository
                .get_installation(installation.id)
                .await?
                .is_some_and(|current| {
                    current.desired_state == domain::PluginDesiredState::ActiveRequested
                });
        if !was_active_and_still_requested
            && !self
                .repository
                .installation_has_live_references(installation.id)
                .await?
        {
            self.runtime.deactivate_plugin(installation).await?;
        }
        Ok(())
    }

    pub(super) async fn try_mark_current_node_runtime_active(
        &self,
        local_installation: &domain::LocalPluginInstallationRecord,
        loaded_identity: Option<&domain::LoadedRuntimeArtifactIdentity>,
    ) -> Result<ActivatePluginArtifactOutcome> {
        try_mark_current_node_plugin_runtime_active(
            &self.repository,
            &self.node_id,
            local_installation,
            loaded_identity,
        )
        .await
    }

    async fn package_bytes_for_current_node_install(
        &self,
        installation: &domain::PluginInstallationRecord,
    ) -> Result<Vec<u8>> {
        let artifact = self
            .repository
            .get_artifact_instance(&self.node_id, installation.id)
            .await?;
        if let Some(package_path) = artifact
            .as_ref()
            .and_then(|artifact| artifact.package_path.as_deref())
        {
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
    scan_and_refresh_current_node_artifact(repository, node_id, install_root, installation, false)
        .await
}

async fn scan_and_refresh_current_node_artifact<R: PluginRepository + ?Sized>(
    repository: &R,
    node_id: &str,
    install_root: &Path,
    installation: &domain::PluginInstallationRecord,
    boot_reconciliation: bool,
) -> Result<domain::PluginArtifactInstanceRecord> {
    for _ in 0..crate::plugin_lifecycle::ARTIFACT_REFRESH_ATTEMPTS {
        let observed_artifact = repository
            .get_artifact_instance(node_id, installation.id)
            .await?;
        let scanned = scan_current_node_artifact_at(install_root, installation).await;
        let refreshed = repository
            .refresh_artifact_instance(
                &control_plane_contracts::ports::RefreshPluginArtifactInstanceInput {
                    node_id: node_id.to_string(),
                    installation_id: installation.id,
                    observed_artifact,
                    touch_checked_at: !boot_reconciliation,
                    local_version: scanned.local_version,
                    local_checksum: scanned.local_checksum,
                    local_path: scanned.local_path,
                    manifest_fingerprint: scanned.manifest_fingerprint,
                    artifact_status: scanned.artifact_status,
                    last_error: scanned.last_error,
                    reset_runtime_status: boot_reconciliation,
                    checked_at: OffsetDateTime::now_utc(),
                },
            )
            .await?;
        if let Some(record) = refreshed {
            return Ok(record);
        }
    }
    Err(ControlPlaneError::Conflict("plugin_artifact_state_changed").into())
}

pub async fn ready_current_node_plugin_installation<R>(
    repository: &R,
    node_id: &str,
    _install_root: &Path,
    installation_id: Uuid,
) -> Result<domain::LocalPluginInstallationRecord>
where
    R: PluginRepository + ?Sized,
{
    let installation = repository
        .get_installation(installation_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    let artifact = repository
        .get_artifact_instance(node_id, installation_id)
        .await?
        .ok_or(ControlPlaneError::PluginUnavailable)?;
    if !artifact.artifact_status.is_ready()
        || artifact.local_path.is_none()
        || domain::has_local_artifact_warning(artifact.last_error.as_deref())
    {
        return Err(ControlPlaneError::PluginUnavailable.into());
    }
    Ok(domain::LocalPluginInstallationRecord {
        installation,
        artifact,
    })
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
    repository
        .record_artifact_runtime_status(
            &control_plane_contracts::ports::RecordPluginRuntimeStatusInput {
                node_id: node_id.to_string(),
                installation_id: installation.id,
                runtime_status,
                last_error,
                checked_at: OffsetDateTime::now_utc(),
            },
        )
        .await
}

pub async fn try_mark_current_node_plugin_runtime_active<R>(
    repository: &R,
    node_id: &str,
    local_installation: &domain::LocalPluginInstallationRecord,
    loaded_identity: Option<&domain::LoadedRuntimeArtifactIdentity>,
) -> Result<ActivatePluginArtifactOutcome>
where
    R: PluginRepository + ?Sized,
{
    let artifact = &local_installation.artifact;
    if domain::has_local_artifact_warning(artifact.last_error.as_deref()) {
        return Ok(ActivatePluginArtifactOutcome::ArtifactInvalidated);
    }
    let local_path = artifact
        .local_path
        .as_deref()
        .ok_or(ControlPlaneError::PluginUnavailable)?;
    let path = Path::new(local_path);
    let marker_exists = match path.join(ARTIFACT_MARKER_FILE).try_exists() {
        Ok(exists) => exists,
        Err(_) => return Ok(ActivatePluginArtifactOutcome::ArtifactInvalidated),
    };
    let (version, checksum, fingerprint) = if marker_exists
        || artifact.manifest_fingerprint.is_some()
        || loaded_identity.is_some_and(|loaded| loaded.marker.is_some())
    {
        let scanned = inspect_artifact_path(&local_installation.installation, path).await;
        if !scanned.artifact_status.is_ready() || scanned.last_error.is_some() {
            return Ok(ActivatePluginArtifactOutcome::ArtifactInvalidated);
        }
        if let Some(loaded) = loaded_identity {
            if scanned.manifest_fingerprint.as_deref() != Some(loaded.manifest_fingerprint.as_str())
                || scanned.local_version.as_deref() != Some(loaded.plugin_version.as_str())
                || !loaded.marker.as_ref().is_some_and(|marker| {
                    Some(marker.version.as_str()) == scanned.local_version.as_deref()
                        && marker.checksum == scanned.local_checksum
                })
            {
                return Ok(ActivatePluginArtifactOutcome::ArtifactInvalidated);
            }
        }
        (
            scanned.local_version,
            scanned.local_checksum,
            scanned.manifest_fingerprint,
        )
    } else {
        (
            artifact.local_version.clone(),
            artifact.local_checksum.clone(),
            artifact.manifest_fingerprint.clone(),
        )
    };
    let (version, fingerprint) = match loaded_identity {
        Some(loaded) => (
            Some(loaded.plugin_version.clone()),
            Some(loaded.manifest_fingerprint.clone()),
        ),
        None => (version, fingerprint),
    };
    repository
        .try_mark_active_artifact_instance(
            &control_plane_contracts::ports::ActivatePluginArtifactInstanceInput {
                validate_manifest: validate_installed_artifact_manifest,
                node_id: node_id.to_string(),
                installation_id: local_installation.id,
                expected_local_version: version,
                expected_local_checksum: checksum,
                expected_local_path: local_path.to_owned(),
                expected_manifest_fingerprint: fingerprint,
                checked_at: OffsetDateTime::now_utc(),
            },
        )
        .await
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
        manifest_fingerprint: None,
        local_version: None,
        local_checksum: None,
        local_path: None,
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

#[allow(clippy::too_many_arguments)] // Marker fields mirror the persisted artifact metadata one-to-one.
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

/// Compare the installed marker and manifest with the node's recorded identity.
pub async fn validate_installed_plugin_artifact(
    installation: &domain::PluginInstallationRecord,
    artifact: &domain::PluginArtifactInstanceRecord,
) -> Result<()> {
    let path = Path::new(
        artifact
            .local_path
            .as_deref()
            .ok_or(ControlPlaneError::PluginUnavailable)?,
    );
    if installation.expected_checksum.is_none()
        && artifact.local_checksum.is_none()
        && artifact.manifest_fingerprint.is_none()
        && !tokio::fs::try_exists(path.join(ARTIFACT_MARKER_FILE)).await?
    {
        return Ok(());
    }
    let scanned = inspect_artifact_path(installation, path).await;
    if !scanned.artifact_status.is_ready() || scanned.last_error.is_some() {
        anyhow::bail!(
            "invalid installed plugin artifact: {}",
            scanned.last_error.as_deref().unwrap_or("not ready")
        );
    }
    if scanned.local_version != artifact.local_version
        || scanned.local_checksum != artifact.local_checksum
        || scanned.local_path != artifact.local_path
        || scanned.manifest_fingerprint != artifact.manifest_fingerprint
    {
        anyhow::bail!("installed plugin artifact identity changed");
    }
    Ok(())
}

pub(crate) async fn inspect_artifact_path(
    installation: &domain::PluginInstallationRecord,
    path: &Path,
) -> ScannedPluginArtifact {
    let marker = match tokio::fs::read(path.join(ARTIFACT_MARKER_FILE)).await {
        Ok(raw) => serde_json::from_slice::<PluginArtifactMarker>(&raw)
            .map_err(|_| "artifact_marker_corrupted"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("artifact_marker_missing")
        }
        Err(_) => Err("artifact_marker_corrupted"),
    };
    let marker = match marker {
        Ok(marker) => marker,
        Err(error) => {
            let missing = error == "artifact_marker_missing";
            return ScannedPluginArtifact {
                manifest_fingerprint: None,
                local_version: None,
                local_checksum: None,
                local_path: (!missing).then(|| path.display().to_string()),
                artifact_status: if missing {
                    domain::PluginArtifactInstanceStatus::Missing
                } else {
                    domain::PluginArtifactInstanceStatus::Corrupted
                },
                last_error: Some(error.to_string()),
            };
        }
    };
    let base = ScannedPluginArtifact {
        manifest_fingerprint: None,
        local_version: Some(marker.version.clone()),
        local_checksum: marker.checksum.clone(),
        local_path: Some(path.display().to_string()),
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
    let raw_manifest = match tokio::fs::read_to_string(path.join("manifest.yaml")).await {
        Ok(raw_manifest) => raw_manifest,
        Err(_) => {
            return ScannedPluginArtifact {
                artifact_status: domain::PluginArtifactInstanceStatus::Corrupted,
                last_error: Some("manifest_missing".to_string()),
                ..base
            };
        }
    };
    let actual_fingerprint =
        plugin_framework::canonical_manifest_fingerprint(raw_manifest.as_bytes());
    let base = ScannedPluginArtifact {
        manifest_fingerprint: Some(actual_fingerprint.clone()),
        ..base
    };
    let fingerprint_matches =
        marker.manifest_fingerprint.as_deref() == Some(actual_fingerprint.as_str());
    if installation.legacy_manifest_compatibility.as_deref()
        == Some("missing_publisher_namespace_v1")
        && !fingerprint_matches
    {
        return ScannedPluginArtifact {
            artifact_status: domain::PluginArtifactInstanceStatus::Corrupted,
            last_error: Some("manifest_fingerprint_mismatch".to_string()),
            ..base
        };
    }
    if let Err(error) =
        parse_installed_artifact_manifest(installation, &raw_manifest, &actual_fingerprint)
    {
        return ScannedPluginArtifact {
            artifact_status: if error == "plugin_id_mismatch" {
                domain::PluginArtifactInstanceStatus::Mismatched
            } else {
                domain::PluginArtifactInstanceStatus::Corrupted
            },
            last_error: Some(error.to_string()),
            ..base
        };
    }
    let warning = match (&installation.expected_checksum, &marker.checksum) {
        (Some(expected), Some(actual)) if expected != actual => Some("checksum_mismatch"),
        (Some(_), None) => Some("checksum_missing"),
        _ if !fingerprint_matches => Some("manifest_fingerprint_mismatch"),
        _ => None,
    };
    ScannedPluginArtifact {
        last_error: warning.map(str::to_owned),
        ..base
    }
}

/// Validation bound to activation and cutover; storage supplies freshly read bytes.
pub fn validate_installed_managed_manifest(
    installation: &domain::PluginInstallationRecord,
    raw: &str,
) -> Option<String> {
    let manifest = plugin_framework::parse_plugin_manifest(raw).ok()?;
    if manifest.managed.is_none()
        || manifest.version != installation.plugin_version
        || manifest.versioned_plugin_id().ok().as_deref() != Some(installation.plugin_id.as_str())
    {
        return None;
    }
    Some(plugin_framework::canonical_manifest_fingerprint(
        raw.as_bytes(),
    ))
}

pub fn validate_installed_artifact_manifest(
    installation: &domain::PluginInstallationRecord,
    raw: &str,
) -> Option<String> {
    let fingerprint = plugin_framework::canonical_manifest_fingerprint(raw.as_bytes());
    parse_installed_artifact_manifest(installation, raw, &fingerprint).ok()?;
    Some(fingerprint)
}

fn parse_installed_artifact_manifest(
    installation: &domain::PluginInstallationRecord,
    raw: &str,
    fingerprint: &str,
) -> std::result::Result<(), &'static str> {
    let manifest = match installation.legacy_manifest_compatibility.as_deref() {
        None => parse_plugin_manifest(raw),
        Some("missing_publisher_namespace_v1") => {
            plugin_framework::parse_legacy_installed_plugin_manifest(
                raw,
                &plugin_framework::LegacyInstalledManifestEligibility {
                    expected_publisher_namespace: installation.organization.clone(),
                    expected_versioned_plugin_id: installation.plugin_id.clone(),
                    expected_raw_manifest_fingerprint: fingerprint.to_owned(),
                },
            )
        }
        Some(_) => return Err("manifest_compatibility_unsupported"),
    }
    .map_err(|_| "manifest_invalid")?;
    if manifest.version != installation.plugin_version
        || manifest.versioned_plugin_id().ok().as_deref() != Some(installation.plugin_id.as_str())
    {
        return Err("plugin_id_mismatch");
    }
    Ok(())
}
