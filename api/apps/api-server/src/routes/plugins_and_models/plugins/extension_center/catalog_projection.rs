use super::*;

pub(super) async fn installed_catalog_joins(
    dependencies: &ExtensionCenterDependencies,
    category: ExtensionCatalogCategory,
) -> Result<HashMap<String, InstalledCatalogJoin>, ApiError> {
    let records = extension_installation_service(dependencies)
        .list_installed_for_node(&dependencies.api_node_id)
        .await?;
    let install_service = extension_installation_service(dependencies);
    let mut unavailable_collectors = std::collections::HashSet::new();
    for record in &records {
        if domain::is_client_collector_receipt(&record.receipt)
            && install_service
                .client_collector_manifest(record)
                .await
                .is_err()
        {
            unavailable_collectors.insert(record.id);
        }
    }
    let mut joins = project_installed_catalog_joins(records, category);
    for join in joins.values_mut() {
        if unavailable_collectors.contains(&join.installation_id) {
            join.status = InstalledCatalogJoinStatus::Uninstalled;
        }
    }
    if matches!(
        category,
        ExtensionCatalogCategory::RuntimeExtensions | ExtensionCatalogCategory::CapabilityPlugins
    ) {
        for join in joins.values_mut() {
            let artifact = control_plane::ports::PluginRepository::get_artifact_instance(
                &dependencies.store,
                &dependencies.api_node_id,
                join.installation_id,
            )
            .await?;
            if artifact.is_some_and(|artifact| {
                artifact.artifact_status == domain::PluginArtifactInstanceStatus::Missing
            }) {
                join.status = InstalledCatalogJoinStatus::Uninstalled;
            }
        }
    }
    Ok(joins)
}

pub(super) fn project_installed_catalog_joins(
    records: impl IntoIterator<Item = domain::ExtensionInstallationRecord>,
    category: ExtensionCatalogCategory,
) -> HashMap<String, InstalledCatalogJoin> {
    let mut joins = HashMap::new();
    for family in group_installed_extension_families(records) {
        let entry = family.current;
        if entry.identity.category.as_str() != category.as_str() {
            continue;
        }
        joins.insert(
            entry.identity.catalog_id(),
            InstalledCatalogJoin {
                installation_id: entry.id,
                current_version: entry.identity.version,
                source: entry.source_kind,
                trust: entry.trust_level,
                status: InstalledCatalogJoinStatus::Installed,
            },
        );
    }
    joins
}

pub(super) fn catalog_entry_join<'a>(
    entry: &OfficialExtensionCatalogEntry,
    installed: &'a HashMap<String, InstalledCatalogJoin>,
) -> Option<&'a InstalledCatalogJoin> {
    installed.get(&entry.id)
}

pub(super) fn extension_update_status(
    latest_version: Option<&str>,
    installed_versions: &[String],
) -> &'static str {
    match latest_version {
        Some(version) if installed_versions.iter().any(|local| local == version) => "current",
        Some(_) => "update_available",
        None => "unknown_error",
    }
}

pub(super) async fn find_catalog_entry_for_requested_identity(
    dependencies: &ExtensionCenterDependencies,
    workspace_id: Uuid,
    category: ExtensionCatalogCategory,
    catalog_id: &str,
) -> Result<Option<LocatedOfficialExtensionCatalogEntry>, ApiError> {
    dependencies
        .official_extension_catalog_source
        .find_entry_for_workspace(workspace_id, category.as_str(), catalog_id)
        .await
        .map_err(Into::into)
}

pub(super) fn warning_message(code: &str) -> String {
    match code {
        "signature_missing" => "The catalog entry has no signature metadata.",
        "signing_key_unknown" => "The catalog entry references an unknown signing key.",
        "checksum_missing" => "The catalog entry has no artifact checksum.",
        "below_minimum_host_version" => "The extension requires a newer 1flowbase host version.",
        _ => "The extension requires confirmation before installation.",
    }
    .to_string()
}

pub(super) fn catalog_entry_warnings(
    entry: &OfficialExtensionCatalogEntry,
    trusted_key_ids: &[String],
) -> Vec<ExtensionRiskWarningResponse> {
    let mut warnings = Vec::new();
    if entry.signature.is_none() {
        warnings.push("signature_missing");
    } else if entry
        .signing_key_id()
        .is_none_or(|key_id| !trusted_key_ids.iter().any(|trusted| trusted == key_id))
    {
        warnings.push("signing_key_unknown");
    }
    if entry.checksum.is_none() {
        warnings.push("checksum_missing");
    }
    if catalog_entry_compatibility(entry).is_some() {
        warnings.push("below_minimum_host_version");
    }
    warnings
        .into_iter()
        .map(|code| ExtensionRiskWarningResponse {
            code: code.to_string(),
            message: warning_message(code),
            overridable: true,
        })
        .collect()
}

pub(super) fn current_host_version() -> &'static str {
    option_env!("FLOWBASE_API_SERVER_VERSION")
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

pub(super) fn catalog_entry_compatibility(
    entry: &OfficialExtensionCatalogEntry,
) -> Option<ExtensionCompatibilityWarningResponse> {
    compatibility_for_requirement(&entry.host_version_requirement)
}

pub(super) fn compatibility_for_requirement(
    host_version_requirement: &str,
) -> Option<ExtensionCompatibilityWarningResponse> {
    if host_version_requirement_is_satisfied(current_host_version(), host_version_requirement) {
        return None;
    }
    let minimum_host_version = host_version_requirement
        .trim()
        .strip_prefix(">=")?
        .trim()
        .to_string();
    Some(ExtensionCompatibilityWarningResponse {
        reason: "below_minimum_host_version".to_string(),
        current_host_version: current_host_version().to_string(),
        minimum_host_version,
    })
}

pub(super) fn host_version_requirement_is_satisfied(current: &str, requirement: &str) -> bool {
    let requirement = requirement.trim();
    if requirement == "*" || requirement.is_empty() {
        return true;
    }
    let Some(minimum) = requirement.strip_prefix(">=").map(str::trim) else {
        // Unknown requirement syntax remains visible in the original catalog field. It must not
        // be mislabeled as a below-minimum result without a comparable minimum version.
        return true;
    };
    let parse = |value: &str| {
        let mut parts = value.trim().trim_start_matches('v').split('.');
        Some((
            parts.next()?.parse::<u64>().ok()?,
            parts.next()?.parse::<u64>().ok()?,
            parts.next()?.split('-').next()?.parse::<u64>().ok()?,
        ))
    };
    match (parse(current), parse(minimum)) {
        (Some(current), Some(minimum)) => current >= minimum,
        _ => true,
    }
}

pub(super) fn project_catalog_entry(
    entry: OfficialExtensionCatalogEntry,
    catalog_source: &str,
    installed: &HashMap<String, InstalledCatalogJoin>,
    trusted_key_ids: &[String],
) -> ExtensionCatalogGatewayEntryResponse {
    let installation = catalog_entry_join(&entry, installed);
    let catalog_trust = match (
        catalog_source,
        entry
            .signing_key_id()
            .is_some_and(|key_id| trusted_key_ids.iter().any(|trusted| trusted == key_id)),
    ) {
        ("official", true) => "official",
        (_, true) => "trusted",
        _ => "unknown",
    };
    let source =
        serde_json::to_value(&entry.source).expect("typed extension catalog source must serialize");
    let warnings = catalog_entry_warnings(&entry, trusted_key_ids);
    let compatibility = catalog_entry_compatibility(&entry);
    ExtensionCatalogGatewayEntryResponse {
        category: entry.category,
        id: entry.id,
        name: entry.name,
        organization: entry.organization,
        artifact: entry.artifact,
        version: entry.version,
        description: entry.description,
        host_version_requirement: entry.host_version_requirement,
        slot_codes: entry.slot_codes,
        keywords: entry.keywords,
        source,
        signature: entry.signature,
        checksum: entry.checksum,
        download_locator: entry.download_locator,
        catalog_page: entry.catalog_page,
        catalog_source: catalog_source.to_string(),
        current_version: installation.map(|value| value.current_version.clone()),
        installation_status: installation
            .map(|value| value.status.as_str())
            .unwrap_or("not_installed")
            .to_string(),
        artifact_kind: None,
        installation_source: installation.map(|value| value.source.clone()),
        extension_installation_id: installation.map(|value| value.installation_id.to_string()),
        builtin_template_id: None,
        trust: installation
            .map(|value| value.trust.clone())
            .unwrap_or_else(|| catalog_trust.to_string()),
        warnings,
        compatibility,
        mcp_instances: Vec::new(),
    }
}
