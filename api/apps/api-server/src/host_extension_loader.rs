use std::{
    fs,
    path::{Component, Path},
};

use anyhow::{bail, Context, Result};
use control_plane::{
    errors::ControlPlaneError,
    host_extension::{is_host_extension_installation, is_host_extension_manifest},
    plugin_lifecycle::derive_availability_status,
    plugin_management::{
        mark_current_node_plugin_runtime_status, ready_current_node_plugin_installation,
    },
    ports::{PluginRepository, UpdatePluginDesiredStateInput},
};
use domain::{PluginArtifactStatus, PluginDesiredState, PluginRuntimeStatus};
use plugin_framework::{
    parse_host_extension_contribution_manifest, scan_host_extension_dropins_with_policy,
    HostExtensionContributionManifest, HostExtensionDropinPolicy, HostExtensionDropinScan,
};

use crate::app_state::ApiState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostExtensionStartupSummary {
    pub detected_dropin_count: usize,
    pub pending_restart_count: usize,
    pub loaded_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub warnings: Vec<String>,
}

pub async fn load_host_extensions_at_startup(
    state: &ApiState,
) -> Result<HostExtensionStartupSummary> {
    let detected = scan_host_extensions_from_dropins(state)?;
    let pending = state.store.list_pending_restart_host_extensions().await?;
    let mut summary = HostExtensionStartupSummary {
        detected_dropin_count: detected.installations.len(),
        pending_restart_count: pending.len(),
        loaded_count: 0,
        failed_count: 0,
        skipped_count: 0,
        warnings: detected.warnings,
    };

    for installation in pending {
        match activate_pending_restart_installation(state, installation.id).await? {
            ActivationOutcome::Loaded => summary.loaded_count += 1,
            ActivationOutcome::Failed => summary.failed_count += 1,
            ActivationOutcome::Skipped => summary.skipped_count += 1,
        }
    }

    Ok(summary)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivationOutcome {
    Loaded,
    Failed,
    Skipped,
}

fn scan_host_extensions_from_dropins(state: &ApiState) -> Result<HostExtensionDropinScan> {
    let dropin_root = Path::new(&state.host_extension_dropin_root);
    if !dropin_root.exists() {
        return Ok(HostExtensionDropinScan {
            installations: Vec::new(),
            warnings: Vec::new(),
        });
    }
    if !dropin_root.is_dir() {
        bail!(
            "host extension dropin root must be a directory: {}",
            dropin_root.display()
        );
    }

    scan_host_extension_dropins_with_policy(
        dropin_root,
        HostExtensionDropinPolicy {
            allow_unverified_filesystem_dropins: state.allow_unverified_filesystem_dropins,
        },
    )
    .map_err(anyhow::Error::from)
}

async fn activate_pending_restart_installation(
    state: &ApiState,
    installation_id: uuid::Uuid,
) -> Result<ActivationOutcome> {
    let installation = state
        .store
        .get_installation(installation_id)
        .await?
        .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
    if !is_host_extension_installation(&installation) {
        return Ok(ActivationOutcome::Skipped);
    }
    if installation.artifact_status != PluginArtifactStatus::Ready {
        return Ok(ActivationOutcome::Skipped);
    }

    let local_installation = match ready_current_node_plugin_installation(
        &state.store,
        &state.api_node_id,
        Path::new(&state.provider_install_root),
        installation_id,
    )
    .await
    {
        Ok(local_installation) => local_installation,
        Err(error) if is_current_node_artifact_conflict(&error) => {
            return Ok(ActivationOutcome::Skipped);
        }
        Err(error) => return Err(error),
    };

    match validate_host_extension_installation(&local_installation).and_then(|contribution| {
        state
            .console_surface_registry
            .register_host_extension_contribution(&contribution)
            .map_err(anyhow::Error::from)?;
        Ok(contribution)
    }) {
        Ok(_) => {
            let desired_state = PluginDesiredState::ActiveRequested;
            state
                .store
                .update_desired_state(&UpdatePluginDesiredStateInput {
                    installation_id,
                    availability_status: derive_availability_status(
                        desired_state,
                        PluginArtifactStatus::Ready,
                        PluginRuntimeStatus::Active,
                    ),
                    desired_state,
                })
                .await?;
            mark_current_node_plugin_runtime_status(
                &state.store,
                &state.api_node_id,
                &local_installation,
                PluginRuntimeStatus::Active,
                None,
            )
            .await?;
            Ok(ActivationOutcome::Loaded)
        }
        Err(error) => {
            mark_current_node_plugin_runtime_status(
                &state.store,
                &state.api_node_id,
                &local_installation,
                PluginRuntimeStatus::LoadFailed,
                Some(format!("{error:#}")),
            )
            .await?;
            Ok(ActivationOutcome::Failed)
        }
    }
}

fn is_current_node_artifact_conflict(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::Conflict(
            "plugin_artifact_missing"
                | "plugin_artifact_outdated"
                | "plugin_artifact_mismatched"
                | "plugin_artifact_corrupted"
                | "plugin_runtime_load_failed"
        ))
    )
}

fn validate_host_extension_installation(
    installation: &domain::PluginInstallationRecord,
) -> Result<HostExtensionContributionManifest> {
    let install_root = Path::new(&installation.installed_path);
    let manifest_path = install_root.join("manifest.yaml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = plugin_framework::parse_plugin_manifest(&manifest_raw)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    if !is_host_extension_manifest(&manifest) {
        bail!(
            "installation {} is not a host extension manifest",
            installation.plugin_id
        );
    }

    let contribution_path = install_root.join(&manifest.runtime.entry);
    let contribution_raw = fs::read_to_string(&contribution_path)
        .with_context(|| format!("failed to read {}", contribution_path.display()))?;
    let contribution = parse_host_extension_contribution_manifest(&contribution_raw)
        .with_context(|| format!("failed to parse {}", contribution_path.display()))?;
    let plugin_code = manifest
        .plugin_code()
        .with_context(|| format!("invalid plugin identity {}", manifest.plugin_id))?;
    if plugin_code != contribution.extension_id {
        bail!(
            "host extension contribution identity mismatch: package {} contribution {}",
            plugin_code,
            contribution.extension_id
        );
    }
    if manifest.version != contribution.version {
        bail!(
            "host extension contribution version mismatch: package {} contribution {}",
            manifest.version,
            contribution.version
        );
    }
    validate_native_library(install_root, &contribution)?;

    Ok(contribution)
}

fn validate_native_library(
    install_root: &Path,
    contribution: &HostExtensionContributionManifest,
) -> Result<()> {
    if contribution.native.library.starts_with("builtin://") {
        return Ok(());
    }

    let library = Path::new(&contribution.native.library);
    if library.is_absolute()
        || library
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        bail!(
            "host extension native library must stay under install root: {}",
            contribution.native.library
        );
    }

    let library_path = install_root.join(library);
    if !library_path.is_file() {
        bail!("native library not found at {}", library_path.display());
    }

    Ok(())
}
