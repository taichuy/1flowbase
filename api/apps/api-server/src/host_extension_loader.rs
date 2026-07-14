use std::{
    fs,
    path::{Component, Path},
};

use anyhow::{Context, Result, bail};
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
    HostExtensionContributionManifest, HostExtensionDropinPolicy, HostExtensionDropinScan,
    parse_host_extension_contribution_manifest, scan_host_extension_dropins_with_policy,
};

#[cfg(test)]
use crate::app_state::ApiState;
use crate::host_extensions::console::{
    ResolvedHostExtensionConsoleContribution, linked_host_console_route_sources,
    resolve_linked_host_extension_console_contribution,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostExtensionStartupSummary {
    pub detected_dropin_count: usize,
    pub pending_restart_count: usize,
    pub loaded_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub warnings: Vec<String>,
}

pub(crate) struct PreparedHostExtensionsAtStartup {
    pub(crate) contributions: Vec<ResolvedHostExtensionConsoleContribution>,
    activation_candidates: Vec<domain::PluginInstallationRecord>,
    pub(crate) summary: HostExtensionStartupSummary,
}

impl PreparedHostExtensionsAtStartup {
    pub(crate) fn take_contributions(&mut self) -> Vec<ResolvedHostExtensionConsoleContribution> {
        std::mem::take(&mut self.contributions)
    }
}

pub(crate) async fn prepare_host_extensions_at_startup(
    store: &storage_durable::MainDurableStore,
    api_node_id: &str,
    provider_install_root: &str,
    host_extension_dropin_root: &str,
    allow_unverified_filesystem_dropins: bool,
) -> Result<PreparedHostExtensionsAtStartup> {
    let detected = scan_host_extensions_from_dropins(
        host_extension_dropin_root,
        allow_unverified_filesystem_dropins,
    )?;
    let installations = store.list_installations().await?;
    let mut summary = HostExtensionStartupSummary {
        detected_dropin_count: detected.installations.len(),
        pending_restart_count: installations
            .iter()
            .filter(|installation| {
                is_host_extension_installation(installation)
                    && installation.desired_state == PluginDesiredState::PendingRestart
            })
            .count(),
        loaded_count: 0,
        failed_count: 0,
        skipped_count: 0,
        warnings: detected.warnings,
    };
    let mut contributions = Vec::new();
    let mut activation_candidates = Vec::new();

    for installation in installations.into_iter().filter(|installation| {
        is_host_extension_installation(installation)
            && matches!(
                installation.desired_state,
                PluginDesiredState::PendingRestart | PluginDesiredState::ActiveRequested
            )
    }) {
        if installation.artifact_status != PluginArtifactStatus::Ready {
            summary.skipped_count += 1;
            continue;
        }

        let local_installation = match ready_current_node_plugin_installation(
            store,
            api_node_id,
            Path::new(provider_install_root),
            installation.id,
        )
        .await
        {
            Ok(local_installation) => local_installation,
            Err(error) if is_current_node_artifact_conflict(&error) => {
                summary.skipped_count += 1;
                continue;
            }
            Err(error) => return Err(error),
        };

        let contribution = match validate_host_extension_installation(&local_installation) {
            Ok(contribution) => contribution,
            Err(error) => {
                mark_host_extension_load_failed(store, api_node_id, &local_installation, &error)
                    .await?;
                summary.failed_count += 1;
                continue;
            }
        };
        let contribution = match resolve_linked_host_extension_console_contribution(
            contribution,
            linked_host_console_route_sources(),
        ) {
            Ok(contribution) => contribution,
            Err(error) => {
                mark_host_extension_load_failed(store, api_node_id, &local_installation, &error)
                    .await?;
                return Err(error);
            }
        };
        contributions.push(contribution);
        activation_candidates.push(local_installation);
    }

    Ok(PreparedHostExtensionsAtStartup {
        contributions,
        activation_candidates,
        summary,
    })
}

async fn mark_host_extension_load_failed(
    store: &storage_durable::MainDurableStore,
    api_node_id: &str,
    installation: &domain::PluginInstallationRecord,
    error: &anyhow::Error,
) -> Result<()> {
    mark_current_node_plugin_runtime_status(
        store,
        api_node_id,
        installation,
        PluginRuntimeStatus::LoadFailed,
        Some(format!("{error:#}")),
    )
    .await?;
    Ok(())
}

pub(crate) async fn activate_prepared_host_extensions(
    store: &storage_durable::MainDurableStore,
    api_node_id: &str,
    mut prepared: PreparedHostExtensionsAtStartup,
) -> Result<HostExtensionStartupSummary> {
    for installation in prepared.activation_candidates {
        let desired_state = PluginDesiredState::ActiveRequested;
        store
            .update_desired_state(&UpdatePluginDesiredStateInput {
                installation_id: installation.id,
                availability_status: derive_availability_status(
                    desired_state,
                    PluginArtifactStatus::Ready,
                    PluginRuntimeStatus::Active,
                ),
                desired_state,
            })
            .await?;
        mark_current_node_plugin_runtime_status(
            store,
            api_node_id,
            &installation,
            PluginRuntimeStatus::Active,
            None,
        )
        .await?;
        prepared.summary.loaded_count += 1;
    }

    Ok(prepared.summary)
}

#[cfg(test)]
pub async fn load_host_extensions_at_startup(
    state: &ApiState,
) -> Result<HostExtensionStartupSummary> {
    let prepared = prepare_host_extensions_at_startup(
        &state.store,
        &state.api_node_id,
        &state.provider_install_root,
        &state.host_extension_dropin_root,
        state.allow_unverified_filesystem_dropins,
    )
    .await?;
    activate_prepared_host_extensions(&state.store, &state.api_node_id, prepared).await
}

fn scan_host_extensions_from_dropins(
    host_extension_dropin_root: &str,
    allow_unverified_filesystem_dropins: bool,
) -> Result<HostExtensionDropinScan> {
    let dropin_root = Path::new(host_extension_dropin_root);
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
            allow_unverified_filesystem_dropins,
        },
    )
    .map_err(anyhow::Error::from)
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
