use std::{
    fs,
    path::{Component, Path},
};

use anyhow::{bail, Context, Result};
use control_plane::{
    errors::ControlPlaneError,
    host_extension::{is_host_extension_installation, is_host_extension_manifest},
    ports::PluginRepository,
};
use domain::{PluginDesiredState, PluginRuntimeStatus};
use plugin_framework::{
    parse_host_extension_contribution_manifest, scan_host_extension_dropins_with_policy,
    HostExtensionContributionManifest, HostExtensionDropinPolicy, HostExtensionDropinScan,
    PluginManifestV1,
};

#[cfg(test)]
use crate::app_state::ApiState;
use crate::host_extensions::console::{
    linked_host_console_route_sources, resolve_linked_host_extension_console_contribution,
    ResolvedHostExtensionConsoleContribution,
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
    graph_extensions: Vec<(PluginManifestV1, HostExtensionContributionManifest)>,
    activation_candidates: Vec<(
        domain::LocalPluginInstallationRecord,
        domain::NativePluginTarget,
    )>,
    pub(crate) summary: HostExtensionStartupSummary,
    failed_candidates: Vec<(domain::NativePluginTarget, String)>,
}

impl PreparedHostExtensionsAtStartup {
    pub(crate) fn native_targets(&self) -> Vec<domain::NativePluginTarget> {
        self.activation_candidates
            .iter()
            .map(|(_, target)| target.clone())
            .collect()
    }
    pub(crate) async fn apply_templates(
        &self,
        store: &storage_durable_postgres::MainDurableStore,
        node_id: &str,
    ) -> Result<()> {
        for (_, target) in &self.activation_candidates {
            if let Err(error) = store.apply_native_plugin_settings_templates(target).await {
                store
                    .complete_native_plugin_startup(
                        target,
                        node_id,
                        PluginRuntimeStatus::LoadFailed,
                        Some(&format!("{error:#}")),
                    )
                    .await?;
                return Err(error);
            }
        }
        Ok(())
    }
    pub(crate) async fn record_boot_failure(
        &self,
        store: &storage_durable_postgres::MainDurableStore,
        node_id: &str,
        error: &anyhow::Error,
    ) {
        for (_, target) in &self.activation_candidates {
            if let Err(record_error) = store
                .complete_native_plugin_startup(
                    target,
                    node_id,
                    PluginRuntimeStatus::LoadFailed,
                    Some(&format!("{error:#}")),
                )
                .await
            {
                tracing::warn!(error=%record_error, "native startup failure observation was fenced");
            }
        }
    }
    pub(crate) fn graph_extensions(
        &self,
    ) -> &[(PluginManifestV1, HostExtensionContributionManifest)] {
        &self.graph_extensions
    }

    pub(crate) fn take_contributions(&mut self) -> Vec<ResolvedHostExtensionConsoleContribution> {
        std::mem::take(&mut self.contributions)
    }
}

pub(crate) async fn prepare_host_extensions_at_startup(
    store: &storage_durable_postgres::MainDurableStore,
    api_node_id: &str,
    _provider_install_root: &str,
    host_extension_dropin_root: &str,
    allow_unverified_filesystem_dropins: bool,
) -> Result<PreparedHostExtensionsAtStartup> {
    let mut conflicts = Vec::new();
    let mut families = std::collections::HashSet::new();
    for i in store
        .list_installations()
        .await?
        .into_iter()
        .filter(is_host_extension_installation)
    {
        if !families.insert((i.scope_id, i.organization.clone(), i.provider_code.clone())) {
            continue;
        }
        if let Err(error) = store.reconcile_legacy_native_plugin_target(i.id).await {
            if matches!(
                error.downcast_ref::<ControlPlaneError>(),
                Some(ControlPlaneError::Conflict(
                    "native_plugin_selection_conflict"
                ))
            ) {
                conflicts.push(format!("{error:#}"));
            } else {
                return Err(error);
            }
        }
    }
    let mut prepared = inspect_selected_host_extensions(
        store,
        api_node_id,
        _provider_install_root,
        host_extension_dropin_root,
        allow_unverified_filesystem_dropins,
    )
    .await?;
    prepared.summary.failed_count += conflicts.len();
    for conflict in &conflicts {
        tracing::warn!(conflict = %conflict, "native plugin activation requires an explicit installation selection");
    }
    prepared.summary.warnings.extend(conflicts);
    for (target, error) in &prepared.failed_candidates {
        store
            .complete_native_plugin_startup(
                target,
                api_node_id,
                PluginRuntimeStatus::LoadFailed,
                Some(error),
            )
            .await?;
    }
    Ok(prepared)
}

pub(crate) async fn inspect_selected_host_extensions(
    store: &storage_durable_postgres::MainDurableStore,
    api_node_id: &str,
    _provider_install_root: &str,
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
    let mut graph_extensions = Vec::new();
    let mut activation_candidates = Vec::new();

    let targets = store.list_native_plugin_targets().await?;
    let mut failed_candidates = Vec::new();
    for target in targets.into_iter().filter(|t| t.enabled) {
        let installation = installations
            .iter()
            .find(|i| i.id == target.installation_id)
            .ok_or_else(|| anyhow::anyhow!("selected native installation is absent"))?;
        let local_installation = match store
            .get_local_installation(api_node_id, installation.id)
            .await?
        {
            Some(local)
                if matches!(
                    local.artifact.artifact_status,
                    domain::PluginArtifactInstanceStatus::Ready
                        | domain::PluginArtifactInstanceStatus::LoadFailed
                ) && local.local_path().is_some() =>
            {
                local
            }
            _ => {
                summary.skipped_count += 1;
                continue;
            }
        };
        // A previous native load failure is retryable at restart. Revalidate the selected
        // immutable artifact below; never call this retry path for an unselected installation.

        let (manifest, contribution) =
            match validate_host_extension_installation(&local_installation) {
                Ok(package) => package,
                Err(error) => {
                    failed_candidates.push((target.clone(), format!("{error:#}")));
                    summary.failed_count += 1;
                    continue;
                }
            };
        let resolved = match resolve_linked_host_extension_console_contribution(
            contribution.clone(),
            linked_host_console_route_sources(),
        ) {
            Ok(contribution) => contribution,
            Err(error) => {
                failed_candidates.push((target.clone(), format!("{error:#}")));
                summary.failed_count += 1;
                continue;
            }
        };
        contributions.push(resolved);
        graph_extensions.push((manifest, contribution));
        activation_candidates.push((local_installation, target));
    }

    Ok(PreparedHostExtensionsAtStartup {
        contributions,
        graph_extensions,
        activation_candidates,
        summary,
        failed_candidates,
    })
}

#[cfg(test)]
static FAIL_AFTER_APPLY: std::sync::Mutex<Option<uuid::Uuid>> = std::sync::Mutex::new(None);
#[cfg(test)]
pub(crate) fn fail_after_template_apply_for(installation_id: uuid::Uuid) {
    *FAIL_AFTER_APPLY.lock().expect("native startup failpoint") = Some(installation_id);
}

pub(crate) async fn activate_prepared_host_extensions(
    store: &storage_durable_postgres::MainDurableStore,
    api_node_id: &str,
    mut prepared: PreparedHostExtensionsAtStartup,
) -> Result<HostExtensionStartupSummary> {
    #[cfg(test)]
    {
        let fail = {
            let mut selected = FAIL_AFTER_APPLY.lock().expect("native startup failpoint");
            if prepared
                .activation_candidates
                .iter()
                .any(|(_, t)| Some(t.installation_id) == *selected)
            {
                selected.take().is_some()
            } else {
                false
            }
        };
        if fail {
            let error = anyhow::anyhow!("controlled native startup failure after template commit");
            prepared
                .record_boot_failure(store, api_node_id, &error)
                .await;
            return Err(error);
        }
    }
    for (_, target) in &prepared.activation_candidates {
        if let Err(error) = store
            .complete_native_plugin_startup(target, api_node_id, PluginRuntimeStatus::Active, None)
            .await
        {
            prepared
                .record_boot_failure(store, api_node_id, &error)
                .await;
            return Err(error);
        }
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
    prepared
        .apply_templates(&state.store, &state.api_node_id)
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

fn validate_host_extension_installation(
    installation: &domain::LocalPluginInstallationRecord,
) -> Result<(PluginManifestV1, HostExtensionContributionManifest)> {
    let install_root = Path::new(
        installation
            .local_path()
            .ok_or(ControlPlaneError::Conflict("plugin_artifact_path_missing"))?,
    );
    let manifest_path = install_root.join("manifest.yaml");
    let manifest_raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = plugin_framework::parse_plugin_manifest(&manifest_raw)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    if manifest.versioned_plugin_id()? != installation.plugin_id
        || manifest.version != installation.plugin_version
        || installation.artifact.local_version.as_deref()
            != Some(installation.plugin_version.as_str())
        || installation.artifact.local_checksum != installation.expected_checksum
    {
        bail!("native selected installation/artifact identity mismatch");
    }
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
    contribution.validate_package_settings_pages(&manifest)?;
    for page in &manifest.settings_pages {
        plugin_framework::read_plugin_settings_page_source(install_root, page)?;
    }
    validate_native_library(install_root, &contribution)?;

    Ok((manifest, contribution))
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
