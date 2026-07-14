use std::{collections::BTreeSet, sync::Arc};

use anyhow::{bail, Result};
use plugin_framework::HostExtensionContributionManifest;

use crate::{app_state::ApiState, routes::console_route_assembly::ConsoleRouteAssembly};

/// A console route source is linked into this API binary. It intentionally is not a native
/// library ABI: route handlers remain ordinary Axum handlers compiled with this host.
#[derive(Clone, Copy)]
pub(crate) struct LinkedHostConsoleRouteSource {
    pub(crate) extension_id: &'static str,
    pub(crate) version: &'static str,
    pub(crate) route_assembly: fn() -> ConsoleRouteAssembly<Arc<ApiState>>,
}

pub(crate) struct ResolvedHostExtensionConsoleContribution {
    pub(crate) contribution: HostExtensionContributionManifest,
    pub(crate) route_assembly: Option<ConsoleRouteAssembly<Arc<ApiState>>>,
}

const LINKED_HOST_CONSOLE_ROUTE_SOURCES: &[LinkedHostConsoleRouteSource] = &[];

pub(crate) fn linked_host_console_route_sources() -> &'static [LinkedHostConsoleRouteSource] {
    LINKED_HOST_CONSOLE_ROUTE_SOURCES
}

pub(crate) fn resolve_linked_host_extension_console_contribution(
    contribution: HostExtensionContributionManifest,
    sources: &[LinkedHostConsoleRouteSource],
) -> Result<ResolvedHostExtensionConsoleContribution> {
    validate_linked_host_console_route_sources(sources)?;
    let source = sources.iter().find(|source| {
        source.extension_id == contribution.extension_id && source.version == contribution.version
    });

    if contribution_requires_console_route_source(&contribution) && source.is_none() {
        bail!(
            "HostExtension {}@{} declares console API registrations but has no linked console route source",
            contribution.extension_id,
            contribution.version,
        );
    }

    Ok(ResolvedHostExtensionConsoleContribution {
        contribution,
        route_assembly: source.map(|source| (source.route_assembly)()),
    })
}

fn contribution_requires_console_route_source(
    contribution: &HostExtensionContributionManifest,
) -> bool {
    !contribution.settings_features.is_empty() || !contribution.console_operations.is_empty()
}

fn validate_linked_host_console_route_sources(
    sources: &[LinkedHostConsoleRouteSource],
) -> Result<()> {
    let mut keys = BTreeSet::new();
    for source in sources {
        if source.extension_id.trim().is_empty() || source.version.trim().is_empty() {
            bail!("linked HostExtension console route source must have an id and version");
        }
        if !keys.insert((source.extension_id, source.version)) {
            bail!(
                "duplicate linked HostExtension console route source {}@{}",
                source.extension_id,
                source.version,
            );
        }
    }
    Ok(())
}
