use super::install::load_actor_context_for_user;
use super::*;

#[derive(Debug, Clone)]
pub struct PluginCatalogEntry {
    pub installation: domain::PluginInstallationRecord,
    pub local_artifact: domain::PluginArtifactInstanceRecord,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub provider_label_key: String,
    pub help_url: Option<String>,
    pub default_base_url: Option<String>,
    pub model_discovery_mode: String,
    pub assigned_to_current_workspace: bool,
    pub catalog_refresh_status: String,
    pub catalog_last_error_message: Option<String>,
    pub catalog_refreshed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
pub struct PluginCatalogView {
    pub entries: Vec<PluginCatalogEntry>,
    pub i18n_catalog: I18nCatalog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCatalogCategory {
    AgentFlow,
    CapabilityPlugins,
    HostExtensions,
    I18n,
    Mcp,
    RuntimeExtensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionDomainBindingOwner {
    Host,
    RuntimeExtension,
    CapabilityPlugin,
    Mcp,
    AgentFlow,
}

impl ExtensionDomainBindingOwner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::RuntimeExtension => "runtime_extension",
            Self::CapabilityPlugin => "capability_plugin",
            Self::Mcp => "mcp",
            Self::AgentFlow => "agent_flow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypedExtensionApplication {
    pub installs_node_artifact: bool,
    pub binding_owner: ExtensionDomainBindingOwner,
}

impl ExtensionCatalogCategory {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "agent-flow" => Ok(Self::AgentFlow),
            "capability-plugins" => Ok(Self::CapabilityPlugins),
            "host-extensions" => Ok(Self::HostExtensions),
            "i18n" => Ok(Self::I18n),
            "mcp" => Ok(Self::Mcp),
            "runtime-extensions" => Ok(Self::RuntimeExtensions),
            _ => Err(ControlPlaneError::InvalidInput("extension_catalog_category").into()),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AgentFlow => "agent-flow",
            Self::CapabilityPlugins => "capability-plugins",
            Self::HostExtensions => "host-extensions",
            Self::I18n => "i18n",
            Self::Mcp => "mcp",
            Self::RuntimeExtensions => "runtime-extensions",
        }
    }

    pub fn fixed_plugin_type(self) -> Option<&'static str> {
        match self {
            Self::HostExtensions => Some("host_extension"),
            Self::CapabilityPlugins => Some("capability_plugin"),
            Self::AgentFlow | Self::I18n | Self::Mcp | Self::RuntimeExtensions => None,
        }
    }

    pub fn application(self) -> TypedExtensionApplication {
        let binding_owner = match self {
            Self::HostExtensions => ExtensionDomainBindingOwner::Host,
            Self::RuntimeExtensions => ExtensionDomainBindingOwner::RuntimeExtension,
            Self::CapabilityPlugins => ExtensionDomainBindingOwner::CapabilityPlugin,
            Self::Mcp => ExtensionDomainBindingOwner::Mcp,
            Self::AgentFlow => ExtensionDomainBindingOwner::AgentFlow,
            Self::I18n => ExtensionDomainBindingOwner::Host,
        };
        TypedExtensionApplication {
            installs_node_artifact: matches!(
                self,
                Self::HostExtensions | Self::RuntimeExtensions | Self::CapabilityPlugins
            ),
            binding_owner,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRiskWarning {
    pub code: String,
    pub overridable: bool,
}

#[derive(Debug, Clone)]
pub struct LocalExtensionInventoryEntry {
    pub installation: domain::PluginInstallationRecord,
    pub local_artifact: domain::PluginArtifactInstanceRecord,
    pub category: String,
    pub artifact_kind: Option<String>,
    pub source: String,
    pub trust: String,
    pub warnings: Vec<ExtensionRiskWarning>,
    pub artifact_id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub current_version: String,
    pub system_requirements: Option<String>,
    pub installation_status: String,
}

#[derive(Debug, Clone)]
pub struct LocalExtensionInventoryPage {
    pub entries: Vec<LocalExtensionInventoryEntry>,
    pub limit: usize,
    pub next_cursor: Option<String>,
}

pub(super) fn extension_source(source_kind: &str) -> &'static str {
    match source_kind {
        "builtin" => "builtin",
        "official_registry" => "official_registry",
        "mirror_registry" => "mirror",
        "uploaded" => "upload",
        _ => "upload",
    }
}

pub(super) fn extension_trust(installation: &domain::PluginInstallationRecord) -> &'static str {
    extension_trust_values(&installation.source_kind, &installation.trust_level)
}

pub(super) fn extension_trust_values(source_kind: &str, trust_level: &str) -> &'static str {
    if source_kind == "builtin"
        || (source_kind == "official_registry" && trust_level == "verified_official")
    {
        "official"
    } else if trust_level == "verified_official" {
        "trusted"
    } else {
        "unknown"
    }
}

fn installation_category(installation: &domain::PluginInstallationRecord) -> &str {
    if is_host_extension_installation(installation) {
        return "host-extensions";
    }
    if is_model_provider_installation(installation) {
        return "runtime-extensions";
    }
    match installation
        .metadata_json
        .get("plugin_type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("capability_plugin")
    {
        "data_source" | "model_provider" => "runtime-extensions",
        "host_extension" => "host-extensions",
        _ => "capability-plugins",
    }
}

fn extension_warnings(
    installation: &domain::PluginInstallationRecord,
    artifact: &domain::PluginArtifactInstanceRecord,
) -> Vec<ExtensionRiskWarning> {
    let mut codes = Vec::new();
    if let Some(code) = artifact.last_error.as_deref() {
        codes.push(code.to_string());
    }
    match installation.signature_status {
        domain::ExtensionSignatureStatus::Verified => {}
        domain::ExtensionSignatureStatus::UnknownKey => {
            codes.push(PLUGIN_RISK_SIGNING_KEY_UNKNOWN.to_string())
        }
        domain::ExtensionSignatureStatus::Invalid => {
            codes.push(PLUGIN_RISK_SIGNATURE_INVALID.to_string())
        }
        domain::ExtensionSignatureStatus::Missing => {
            codes.push(PLUGIN_RISK_SIGNATURE_MISSING.to_string())
        }
    }
    codes.sort();
    codes.dedup();
    codes
        .into_iter()
        .map(|code| ExtensionRiskWarning {
            code,
            overridable: true,
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct PluginCatalogFilter {
    pub plugin_type: Option<String>,
}

impl PluginCatalogFilter {
    fn matches(&self, plugin_type: &str) -> bool {
        self.plugin_type
            .as_deref()
            .is_none_or(|value| value == plugin_type)
    }
}

#[derive(Debug, Clone)]
pub struct OfficialPluginCatalogFilter {
    pub plugin_type: Option<String>,
    pub search_query: Option<String>,
    pub cursor: Option<String>,
    pub limit: usize,
}

impl Default for OfficialPluginCatalogFilter {
    fn default() -> Self {
        Self {
            plugin_type: None,
            search_query: None,
            cursor: None,
            limit: 20,
        }
    }
}

impl OfficialPluginCatalogFilter {
    fn matches_plugin_type(&self, plugin_type: &str) -> bool {
        self.plugin_type
            .as_deref()
            .is_none_or(|value| value == plugin_type)
    }

    fn matches_search(&self, entry: &OfficialPluginCatalogEntry) -> bool {
        let Some(search_query) = self.search_query.as_deref() else {
            return true;
        };
        let query = search_query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }

        entry.display_name.to_lowercase().contains(&query)
            || entry
                .description
                .as_deref()
                .is_some_and(|description| description.to_lowercase().contains(&query))
            || entry.provider_code.to_lowercase().contains(&query)
            || entry.plugin_id.to_lowercase().contains(&query)
            || entry.protocol.to_lowercase().contains(&query)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficialPluginInstallStatus {
    NotInstalled,
    Installed,
    Assigned,
}

impl OfficialPluginInstallStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Installed => "installed",
            Self::Assigned => "assigned",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OfficialPluginCatalogEntry {
    pub plugin_id: String,
    pub plugin_type: String,
    pub provider_code: String,
    pub display_name: String,
    pub description: Option<String>,
    pub protocol: String,
    pub latest_version: String,
    pub minimum_host_version: String,
    pub current_host_version: String,
    pub compatibility_status: String,
    pub compatibility_warning_reason: Option<String>,
    pub icon: Option<String>,
    pub selected_artifact: OfficialPluginArtifact,
    pub help_url: Option<String>,
    pub model_discovery_mode: String,
    pub install_status: OfficialPluginInstallStatus,
}

#[derive(Debug, Clone)]
pub struct OfficialPluginCatalogPage {
    pub limit: usize,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OfficialPluginCatalogView {
    pub source_kind: String,
    pub source_label: String,
    pub registry_url: String,
    pub source_freshness: String,
    pub page: OfficialPluginCatalogPage,
    pub entries: Vec<OfficialPluginCatalogEntry>,
}

#[derive(Debug, Clone)]
pub struct PluginInstalledVersionView {
    pub installation_id: Uuid,
    pub plugin_version: String,
    pub source_kind: String,
    pub trust_level: String,
    pub desired_state: String,
    pub availability_status: String,
    pub local_artifact: domain::PluginArtifactInstanceRecord,
    pub created_at: OffsetDateTime,
    pub is_current: bool,
}

#[derive(Debug, Clone)]
pub struct PluginFamilyView {
    pub provider_code: String,
    pub plugin_type: String,
    pub namespace: String,
    pub label_key: String,
    pub description_key: Option<String>,
    pub provider_label_key: String,
    pub icon: Option<String>,
    pub protocol: String,
    pub help_url: Option<String>,
    pub default_base_url: Option<String>,
    pub model_discovery_mode: String,
    pub current_installation_id: Uuid,
    pub current_version: String,
    pub current_local_artifact: domain::PluginArtifactInstanceRecord,
    pub latest_version: Option<String>,
    pub has_update: bool,
    pub installed_versions: Vec<PluginInstalledVersionView>,
}

fn local_artifact_snapshot(
    snapshots: &HashMap<Uuid, domain::PluginArtifactInstanceRecord>,
    node_id: &str,
    installation: &domain::PluginInstallationRecord,
) -> domain::PluginArtifactInstanceRecord {
    snapshots.get(&installation.id).cloned().unwrap_or_else(|| {
        domain::PluginArtifactInstanceRecord {
            node_id: node_id.to_string(),
            installation_id: installation.id,
            local_version: None,
            local_checksum: None,
            local_path: None,
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Missing,
            runtime_status: domain::PluginRuntimeStatus::Inactive,
            availability_status: domain::PluginAvailabilityStatus::ArtifactMissing,
            checked_at: installation.updated_at,
            last_error: Some("artifact_snapshot_missing".to_string()),
            is_current: false,
        }
    })
}

#[derive(Debug, Clone)]
pub struct PluginFamilyCatalogView {
    pub entries: Vec<PluginFamilyView>,
    pub i18n_catalog: I18nCatalog,
}

#[derive(Debug)]
struct PluginCatalogProjectionView {
    help_url: Option<String>,
    default_base_url: Option<String>,
    model_discovery_mode: String,
    i18n_bundles: BTreeMap<String, serde_json::Value>,
    catalog_refresh_status: String,
    catalog_last_error_message: Option<String>,
    catalog_refreshed_at: Option<OffsetDateTime>,
}

fn compare_plugin_versions(left: &str, right: &str) -> Ordering {
    if let (Ok(left), Ok(right)) = (semver::Version::parse(left), semver::Version::parse(right)) {
        return left.cmp(&right);
    }
    let mut left_parts = left.split('.');
    let mut right_parts = right.split('.');

    loop {
        match (left_parts.next(), right_parts.next()) {
            (None, None) => return Ordering::Equal,
            (Some(left_part), Some(right_part)) => {
                let ordering = match (left_part.parse::<u64>(), right_part.parse::<u64>()) {
                    (Ok(left_number), Ok(right_number)) => left_number.cmp(&right_number),
                    _ => left_part.cmp(right_part),
                };

                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(left_part), None) => match left_part.parse::<u64>() {
                Ok(0) => continue,
                Ok(_) | Err(_) => return Ordering::Greater,
            },
            (None, Some(right_part)) => match right_part.parse::<u64>() {
                Ok(0) => continue,
                Ok(_) | Err(_) => return Ordering::Less,
            },
        }
    }
}

fn semver_version_is_newer(candidate: &str, current: &str) -> bool {
    match (
        semver::Version::parse(candidate),
        semver::Version::parse(current),
    ) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => false,
    }
}

fn pick_latest_official_entry(
    current: OfficialPluginSourceEntry,
    candidate: OfficialPluginSourceEntry,
) -> OfficialPluginSourceEntry {
    match compare_plugin_versions(&candidate.latest_version, &current.latest_version) {
        Ordering::Greater => candidate,
        Ordering::Less => current,
        Ordering::Equal => {
            if candidate.plugin_id < current.plugin_id {
                candidate
            } else {
                current
            }
        }
    }
}

pub(super) fn normalize_official_entries(
    entries: Vec<OfficialPluginSourceEntry>,
) -> Vec<OfficialPluginSourceEntry> {
    let mut grouped = HashMap::<String, OfficialPluginSourceEntry>::new();

    for entry in entries {
        let provider_code = entry.provider_code.clone();
        match grouped.remove(&provider_code) {
            Some(existing) => {
                grouped.insert(provider_code, pick_latest_official_entry(existing, entry));
            }
            None => {
                grouped.insert(provider_code, entry);
            }
        }
    }

    let mut normalized = grouped.into_values().collect::<Vec<_>>();
    normalized.sort_by(|left, right| {
        left.provider_code
            .cmp(&right.provider_code)
            .then_with(|| left.plugin_id.cmp(&right.plugin_id))
    });
    normalized
}

fn read_official_i18n_value(bundle: &serde_json::Value, dotted_key: &str) -> Option<String> {
    let mut current = bundle;
    for segment in dotted_key.split('.') {
        current = current.get(segment)?;
    }

    current.as_str().map(str::trim).and_then(|value| {
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn official_locale_candidates(
    i18n_summary: &crate::ports::OfficialPluginI18nSummary,
    locales: &RequestedLocales,
) -> Vec<String> {
    let mut candidates = vec![
        locales.resolved_locale.clone(),
        locales.fallback_locale.clone(),
        i18n_summary.default_locale.clone(),
    ];
    candidates.extend(i18n_summary.available_locales.iter().cloned());
    candidates.dedup();
    candidates
}

fn resolve_official_i18n_value(
    i18n_summary: &crate::ports::OfficialPluginI18nSummary,
    locales: &RequestedLocales,
    dotted_key: &str,
) -> Option<String> {
    for locale in official_locale_candidates(i18n_summary, locales) {
        let Some(bundle) = i18n_summary.bundles.get(&locale) else {
            continue;
        };
        if let Some(value) = read_official_i18n_value(bundle, dotted_key) {
            return Some(value);
        }
    }

    None
}

fn paginate_official_entries(
    entries: Vec<OfficialPluginCatalogEntry>,
    filter: &OfficialPluginCatalogFilter,
) -> (Vec<OfficialPluginCatalogEntry>, Option<String>) {
    let start_index = filter
        .cursor
        .as_deref()
        .and_then(|cursor| entries.iter().position(|entry| entry.plugin_id == cursor))
        .map_or(0, |index| index.saturating_add(1));
    let page_end = start_index.saturating_add(filter.limit).min(entries.len());
    let page_entries = entries[start_index..page_end].to_vec();
    let next_cursor = if page_end < entries.len() {
        page_entries.last().map(|entry| entry.plugin_id.clone())
    } else {
        None
    };

    (page_entries, next_cursor)
}

fn metadata_string(metadata: &serde_json::Value, key: &str) -> Option<String> {
    metadata.get(key)?.as_str().map(str::to_string)
}

impl<R, H> PluginManagementService<R, H>
where
    R: AuthRepository
        + PluginRepository
        + ModelProviderRepository
        + NodeContributionRepository
        + JsDependencyRepository,
    H: ProviderRuntimePort,
{
    pub async fn list_local_inventory(
        &self,
        actor_user_id: Uuid,
        category: Option<ExtensionCatalogCategory>,
        cursor: Option<Uuid>,
        limit: usize,
    ) -> Result<LocalExtensionInventoryPage> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.view.all")
            .await?;
        let mut installations = self.repository.list_installations().await?;
        installations.sort_by(|left, right| left.id.cmp(&right.id));
        let start = cursor
            .and_then(|cursor| installations.iter().position(|item| item.id == cursor))
            .map_or(0, |index| index.saturating_add(1));
        let mut entries = Vec::with_capacity(limit);
        let mut last_seen = None;
        for installation in installations.into_iter().skip(start) {
            let item_category = installation_category(&installation);
            if category.is_some_and(|category| category.as_str() != item_category) {
                continue;
            }
            last_seen = Some(installation.id);
            let artifact = self
                .refresh_current_node_artifact_snapshot(&installation)
                .await?;
            if artifact.local_path.is_none() {
                continue;
            }
            entries.push(LocalExtensionInventoryEntry {
                category: item_category.to_string(),
                artifact_kind: Some(
                    installation
                        .metadata_json
                        .get("plugin_type")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            if is_host_extension_installation(&installation) {
                                "host_extension".to_string()
                            } else if is_model_provider_installation(&installation) {
                                "model_provider".to_string()
                            } else {
                                "capability_plugin".to_string()
                            }
                        }),
                ),
                source: extension_source(&installation.source_kind).to_string(),
                trust: extension_trust(&installation).to_string(),
                warnings: extension_warnings(&installation, &artifact),
                artifact_id: metadata_string(&installation.metadata_json, "official_plugin_id")
                    .unwrap_or_else(|| installation.plugin_id.clone()),
                display_name: metadata_string(&installation.metadata_json, "display_name")
                    .unwrap_or_else(|| installation.provider_code.clone()),
                description: metadata_string(&installation.metadata_json, "description"),
                current_version: installation.plugin_version.clone(),
                system_requirements: metadata_string(
                    &installation.metadata_json,
                    "minimum_host_version",
                ),
                installation_status: "installed".to_string(),
                installation,
                local_artifact: artifact,
            });
            if entries.len() == limit {
                break;
            }
        }
        let next_cursor = (entries.len() == limit)
            .then(|| last_seen.map(|id| id.to_string()))
            .flatten();
        Ok(LocalExtensionInventoryPage {
            entries,
            limit,
            next_cursor,
        })
    }

    pub async fn list_catalog(
        &self,
        actor_user_id: Uuid,
        filter: PluginCatalogFilter,
        locales: RequestedLocales,
    ) -> Result<PluginCatalogView> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.view.all")
            .await?;

        let assigned_installation_ids = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|assignment| assignment.installation_id)
            .collect::<HashSet<_>>();
        let installations = self.repository.list_installations().await?;
        let artifact_snapshots = self
            .repository
            .list_artifact_instances(&self.node_id)
            .await?
            .into_iter()
            .map(|snapshot| (snapshot.installation_id, snapshot))
            .collect::<HashMap<_, _>>();
        let projections = self
            .repository
            .list_plugin_package_catalog_projections()
            .await?
            .into_iter()
            .map(|projection| (projection.installation_id, projection))
            .collect::<HashMap<_, _>>();
        let mut catalog = Vec::with_capacity(installations.len());
        let mut i18n_catalog = BTreeMap::new();
        for installation in installations {
            if !filter.matches("model_provider") {
                continue;
            }
            if !is_model_provider_installation(&installation) {
                continue;
            }
            let namespace = plugin_namespace(&installation.provider_code);
            let projection = plugin_catalog_projection_view(projections.get(&installation.id));
            merge_i18n_catalog(
                &mut i18n_catalog,
                trim_json_bundles(&namespace, &projection.i18n_bundles, &locales),
            );
            catalog.push(PluginCatalogEntry {
                plugin_type: "model_provider".to_string(),
                namespace,
                label_key: "plugin.label".to_string(),
                description_key: Some("plugin.description".to_string()),
                provider_label_key: "provider.label".to_string(),
                help_url: projection.help_url,
                default_base_url: projection.default_base_url,
                model_discovery_mode: projection.model_discovery_mode,
                assigned_to_current_workspace: assigned_installation_ids.contains(&installation.id),
                catalog_refresh_status: projection.catalog_refresh_status,
                catalog_last_error_message: projection.catalog_last_error_message,
                catalog_refreshed_at: projection.catalog_refreshed_at,
                local_artifact: local_artifact_snapshot(
                    &artifact_snapshots,
                    &self.node_id,
                    &installation,
                ),
                installation,
            });
        }

        Ok(PluginCatalogView {
            entries: catalog,
            i18n_catalog,
        })
    }

    pub async fn list_official_catalog(
        &self,
        actor_user_id: Uuid,
        filter: OfficialPluginCatalogFilter,
        locales: RequestedLocales,
    ) -> Result<OfficialPluginCatalogView> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.view.all")
            .await?;

        let assigned_installation_ids = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|assignment| assignment.installation_id)
            .collect::<HashSet<_>>();
        let installations = self.repository.list_installations().await?;
        let official_snapshot = self.official_source.list_official_catalog().await?;
        let normalized_entries = normalize_official_entries(official_snapshot.entries);

        let entries = normalized_entries
            .into_iter()
            .filter(|entry| filter.matches_plugin_type(&entry.plugin_type))
            .map(|entry| {
                let matching_installations = installations
                    .iter()
                    .filter(|installation| installation.provider_code == entry.provider_code)
                    .collect::<Vec<_>>();
                let install_status = if matching_installations
                    .iter()
                    .any(|installation| assigned_installation_ids.contains(&installation.id))
                {
                    OfficialPluginInstallStatus::Assigned
                } else if !matching_installations.is_empty() {
                    OfficialPluginInstallStatus::Installed
                } else {
                    OfficialPluginInstallStatus::NotInstalled
                };
                let display_name =
                    resolve_official_i18n_value(&entry.i18n_summary, &locales, "provider.label")
                        .or_else(|| {
                            resolve_official_i18n_value(
                                &entry.i18n_summary,
                                &locales,
                                "plugin.label",
                            )
                        })
                        .unwrap_or_else(|| entry.provider_code.clone());
                let description = resolve_official_i18n_value(
                    &entry.i18n_summary,
                    &locales,
                    "plugin.description",
                );

                let compatibility = official_plugin_host_compatibility(
                    &entry.minimum_host_version,
                    &self.host_version,
                );

                OfficialPluginCatalogEntry {
                    plugin_id: entry.plugin_id,
                    plugin_type: entry.plugin_type,
                    provider_code: entry.provider_code,
                    display_name,
                    description,
                    protocol: entry.protocol,
                    latest_version: entry.latest_version,
                    minimum_host_version: compatibility.minimum_host_version,
                    current_host_version: compatibility.current_host_version,
                    compatibility_status: compatibility.status,
                    compatibility_warning_reason: compatibility.warning_reason,
                    icon: entry.icon,
                    selected_artifact: entry.selected_artifact,
                    help_url: entry.help_url,
                    model_discovery_mode: entry.model_discovery_mode,
                    install_status,
                }
            })
            .filter(|entry| filter.matches_search(entry))
            .collect();
        let (entries, next_cursor) = paginate_official_entries(entries, &filter);

        Ok(OfficialPluginCatalogView {
            source_kind: official_snapshot.source.source_kind,
            source_label: official_snapshot.source.source_label,
            registry_url: official_snapshot.source.registry_url,
            source_freshness: official_snapshot.freshness.as_str().to_string(),
            page: OfficialPluginCatalogPage {
                limit: filter.limit,
                next_cursor,
            },
            entries,
        })
    }

    pub async fn list_families(
        &self,
        actor_user_id: Uuid,
        filter: PluginCatalogFilter,
        locales: RequestedLocales,
    ) -> Result<PluginFamilyCatalogView> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        self.ensure_use_case_permission(&actor, "plugin_config.view.all")
            .await?;

        let assignments = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?;
        let official_snapshot = self.official_source.list_official_catalog().await?;
        let official_latest_by_provider = normalize_official_entries(official_snapshot.entries)
            .into_iter()
            .map(|entry| (entry.provider_code, entry.latest_version))
            .collect::<HashMap<_, _>>();
        let installations = self.repository.list_installations().await?;
        let artifact_snapshots = self
            .repository
            .list_artifact_instances(&self.node_id)
            .await?
            .into_iter()
            .map(|snapshot| (snapshot.installation_id, snapshot))
            .collect::<HashMap<_, _>>();
        let projections = self
            .repository
            .list_plugin_package_catalog_projections()
            .await?
            .into_iter()
            .map(|projection| (projection.installation_id, projection))
            .collect::<HashMap<_, _>>();
        let mut installation_map = HashMap::new();
        let mut installations_by_provider =
            HashMap::<String, Vec<domain::PluginInstallationRecord>>::new();
        for installation in installations {
            installation_map.insert(installation.id, installation.clone());
            installations_by_provider
                .entry(installation.provider_code.clone())
                .or_default()
                .push(installation);
        }
        for versions in installations_by_provider.values_mut() {
            versions.sort_by(|left, right| {
                right
                    .created_at
                    .cmp(&left.created_at)
                    .then_with(|| right.id.cmp(&left.id))
            });
        }
        let mut families = Vec::with_capacity(assignments.len());
        let mut i18n_catalog = BTreeMap::new();
        for assignment in assignments {
            if !filter.matches("model_provider") {
                continue;
            }
            let current = installation_map
                .get(&assignment.installation_id)
                .cloned()
                .ok_or(ControlPlaneError::NotFound("plugin_installation"))?;
            if !is_model_provider_installation(&current) {
                continue;
            }
            let namespace = plugin_namespace(&current.provider_code);
            let current_local_artifact =
                local_artifact_snapshot(&artifact_snapshots, &self.node_id, &current);
            let projection = plugin_catalog_projection_view(projections.get(&current.id));
            merge_i18n_catalog(
                &mut i18n_catalog,
                trim_json_bundles(&namespace, &projection.i18n_bundles, &locales),
            );
            let latest_version = official_latest_by_provider
                .get(&assignment.provider_code)
                .cloned();
            let installed_versions = installations_by_provider
                .get(&assignment.provider_code)
                .into_iter()
                .flatten()
                .map(|installation| PluginInstalledVersionView {
                    installation_id: installation.id,
                    plugin_version: installation.plugin_version.clone(),
                    source_kind: installation.source_kind.clone(),
                    trust_level: installation.trust_level.clone(),
                    desired_state: installation.desired_state.as_str().to_string(),
                    availability_status: local_artifact_snapshot(
                        &artifact_snapshots,
                        &self.node_id,
                        installation,
                    )
                    .availability_status
                    .as_str()
                    .to_string(),
                    local_artifact: local_artifact_snapshot(
                        &artifact_snapshots,
                        &self.node_id,
                        installation,
                    ),
                    created_at: installation.created_at,
                    is_current: installation.id == current.id,
                })
                .collect();

            families.push(PluginFamilyView {
                provider_code: current.provider_code.clone(),
                plugin_type: "model_provider".to_string(),
                namespace,
                label_key: "plugin.label".to_string(),
                description_key: Some("plugin.description".to_string()),
                provider_label_key: "provider.label".to_string(),
                protocol: current.protocol.clone(),
                help_url: projection
                    .help_url
                    .clone()
                    .or_else(|| metadata_string(&current.metadata_json, "help_url")),
                default_base_url: projection
                    .default_base_url
                    .clone()
                    .or_else(|| metadata_string(&current.metadata_json, "default_base_url")),
                model_discovery_mode: if projection.model_discovery_mode == "unknown" {
                    metadata_string(&current.metadata_json, "model_discovery_mode")
                        .unwrap_or(projection.model_discovery_mode)
                } else {
                    projection.model_discovery_mode
                },
                icon: metadata_string(&current.metadata_json, "icon"),
                current_installation_id: current.id,
                current_version: current.plugin_version.clone(),
                current_local_artifact,
                latest_version: latest_version.clone(),
                has_update: latest_version.as_deref().is_some_and(|version| {
                    semver_version_is_newer(version, &current.plugin_version)
                }),
                installed_versions,
            });
        }
        families.sort_by(|left, right| left.provider_code.cmp(&right.provider_code));

        Ok(PluginFamilyCatalogView {
            entries: families,
            i18n_catalog,
        })
    }
}

fn plugin_catalog_projection_view(
    projection: Option<&domain::PluginPackageCatalogProjectionRecord>,
) -> PluginCatalogProjectionView {
    let Some(projection) = projection else {
        return PluginCatalogProjectionView {
            help_url: None,
            default_base_url: None,
            model_discovery_mode: "unknown".to_string(),
            i18n_bundles: BTreeMap::new(),
            catalog_refresh_status: domain::PluginPackageCatalogProjectionStatus::Missing
                .as_str()
                .to_string(),
            catalog_last_error_message: None,
            catalog_refreshed_at: None,
        };
    };

    let snapshot = &projection.catalog_snapshot_json;
    PluginCatalogProjectionView {
        help_url: projection_provider_string(snapshot, "help_url"),
        default_base_url: projection_provider_string(snapshot, "default_base_url"),
        model_discovery_mode: projection_provider_string(snapshot, "model_discovery_mode")
            .unwrap_or_else(|| "unknown".to_string()),
        i18n_bundles: projection_i18n_bundles(snapshot),
        catalog_refresh_status: projection.projection_status.as_str().to_string(),
        catalog_last_error_message: projection.last_error_message.clone(),
        catalog_refreshed_at: projection.refreshed_at,
    }
}

fn projection_provider_string(snapshot: &serde_json::Value, field: &str) -> Option<String> {
    snapshot
        .get("provider")?
        .get(field)?
        .as_str()
        .map(str::to_string)
}

fn projection_i18n_bundles(snapshot: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    snapshot
        .pointer("/i18n/bundles")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}
