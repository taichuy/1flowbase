use std::collections::{BTreeMap, BTreeSet};

use access_control::{
    ConsoleAuthorization, ConsoleLocaleCatalogContribution, ConsoleLocaleText,
    ConsoleOperationRegistration, ConsoleOtherPolicyGroupDisplay, ConsolePolicyGroup,
    ResourceAccessAction, ResourceAccessRegistration, SettingsFeatureLifecycle,
    SettingsFeatureOwnerKind, SettingsFeatureRegistration, SettingsFeatureRegistry,
};
use serde::Deserialize;

use crate::error::{FrameworkResult, PluginFrameworkError};
use crate::provider_contract::PluginFormFieldSchema;
use crate::scope_provider_contract::{
    validate_scope_provider_contribution, ScopeProviderContributionManifest,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionBootstrapPhase {
    PreState,
    Boot,
}

impl HostExtensionBootstrapPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreState => "pre_state",
            Self::Boot => "boot",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionNativeEntrypointManifest {
    pub abi_version: String,
    pub library: String,
    pub entry_symbol: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostInfrastructureProviderManifest {
    pub contract: String,
    pub provider_code: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub config_ref: String,
    #[serde(default)]
    pub config_schema: Vec<PluginFormFieldSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionRouteActionManifest {
    pub resource: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionRouteManifest {
    pub route_id: String,
    pub method: String,
    pub path: String,
    pub action: HostExtensionRouteActionManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsoleSurfacesManifest {
    #[serde(default)]
    pub route_definitions: Vec<HostExtensionConsoleRouteDefinitionManifest>,
    #[serde(default)]
    pub navigation_items: Vec<HostExtensionConsoleNavigationItemManifest>,
    #[serde(default)]
    pub permission_bindings: Vec<HostExtensionConsolePermissionBindingManifest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionConsoleSurfaceKind {
    System,
    DynamicPage,
    HostExtension,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsoleRouteDefinitionManifest {
    pub route_id: String,
    pub surface_key: String,
    pub path: String,
    pub surface_kind: HostExtensionConsoleSurfaceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionConsoleNavigationSlot {
    Primary,
    Secondary,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsoleNavigationItemManifest {
    pub item_id: String,
    pub route_id: String,
    pub parent_item_id: String,
    pub label_key: String,
    pub navigation_slot: HostExtensionConsoleNavigationSlot,
    pub order: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionConsolePermissionRequirement {
    Authenticated,
    AnyPermission,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsolePermissionBindingManifest {
    pub binding_id: String,
    pub route_id: String,
    #[serde(default)]
    pub permission_codes: Vec<String>,
    pub requirement: HostExtensionConsolePermissionRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsoleLocaleTextManifest {
    pub reference: String,
    pub en_us: String,
    pub zh_hans: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsoleOtherPolicyGroupDisplayManifest {
    pub group_id: String,
    pub label_ref: String,
    pub description_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionConsoleLocaleCatalogManifest {
    #[serde(default)]
    pub texts: Vec<HostExtensionConsoleLocaleTextManifest>,
    #[serde(default)]
    pub policy_groups: Vec<HostExtensionConsoleOtherPolicyGroupDisplayManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionWorkerManifest {
    pub worker_id: String,
    pub queue: String,
    pub handler: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionMigrationManifest {
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionContributionManifest {
    pub schema_version: String,
    pub extension_id: String,
    pub version: String,
    pub bootstrap_phase: HostExtensionBootstrapPhase,
    pub native: HostExtensionNativeEntrypointManifest,
    pub owned_resources: Vec<String>,
    pub extends_resources: Vec<String>,
    pub infrastructure_providers: Vec<HostInfrastructureProviderManifest>,
    #[serde(default)]
    pub scope_providers: Vec<ScopeProviderContributionManifest>,
    pub routes: Vec<HostExtensionRouteManifest>,
    #[serde(default)]
    pub settings_features: Vec<SettingsFeatureRegistration>,
    #[serde(default)]
    pub console_operations: Vec<ConsoleOperationRegistration>,
    #[serde(default)]
    pub console_resources: Vec<ResourceAccessRegistration>,
    #[serde(default)]
    pub console_locale_catalog: HostExtensionConsoleLocaleCatalogManifest,
    #[serde(default)]
    pub console_surfaces: HostExtensionConsoleSurfacesManifest,
    pub workers: Vec<HostExtensionWorkerManifest>,
    pub migrations: Vec<HostExtensionMigrationManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostExtensionConsoleContribution {
    pub operations: Vec<ConsoleOperationRegistration>,
    pub resources: Vec<ResourceAccessRegistration>,
    pub locale_catalog: ConsoleLocaleCatalogContribution,
}

impl HostExtensionContributionManifest {
    pub fn console_contribution(&self) -> FrameworkResult<HostExtensionConsoleContribution> {
        validate_console_contributions(self)?;
        Ok(HostExtensionConsoleContribution {
            operations: self.console_operations.clone(),
            resources: self.console_resources.clone(),
            locale_catalog: ConsoleLocaleCatalogContribution {
                owner: access_control::ConsoleOperationOwner {
                    kind: SettingsFeatureOwnerKind::HostExtension,
                    owner_id: self.extension_id.clone(),
                    version: self.version.clone(),
                },
                lifecycle: SettingsFeatureLifecycle::Active,
                texts: self
                    .console_locale_catalog
                    .texts
                    .iter()
                    .map(|text| ConsoleLocaleText {
                        reference: text.reference.clone(),
                        en_us: text.en_us.clone(),
                        zh_hans: text.zh_hans.clone(),
                    })
                    .collect(),
                policy_groups: self
                    .console_locale_catalog
                    .policy_groups
                    .iter()
                    .map(|group| ConsoleOtherPolicyGroupDisplay {
                        group_id: group.group_id.clone(),
                        label_ref: group.label_ref.clone(),
                        description_ref: group.description_ref.clone(),
                    })
                    .collect(),
            },
        })
    }
}

pub fn parse_host_extension_contribution_manifest(
    raw: &str,
) -> FrameworkResult<HostExtensionContributionManifest> {
    let manifest: HostExtensionContributionManifest = serde_yaml::from_str(raw)
        .map_err(|error| PluginFrameworkError::invalid_provider_package(error.to_string()))?;
    validate_host_extension_contribution_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_host_extension_contribution_manifest(
    manifest: &HostExtensionContributionManifest,
) -> FrameworkResult<()> {
    if manifest.schema_version != "1flowbase.host-extension/v1" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "schema_version must be 1flowbase.host-extension/v1",
        ));
    }
    validate_non_empty(&manifest.extension_id, "extension_id")?;
    validate_non_empty(&manifest.version, "version")?;
    if manifest.native.abi_version != "1flowbase.host.native/v1" {
        return Err(PluginFrameworkError::invalid_provider_package(
            "native.abi_version must be 1flowbase.host.native/v1",
        ));
    }
    validate_non_empty(&manifest.native.library, "native.library")?;
    validate_non_empty(&manifest.native.entry_symbol, "native.entry_symbol")?;

    for provider in &manifest.infrastructure_providers {
        validate_non_empty(&provider.contract, "infrastructure_providers[].contract")?;
        validate_non_empty(
            &provider.provider_code,
            "infrastructure_providers[].provider_code",
        )?;
        validate_non_empty(
            &provider.display_name,
            "infrastructure_providers[].display_name",
        )?;
        if !provider.config_ref.starts_with("secret://system/") {
            return Err(PluginFrameworkError::invalid_provider_package(
                "infrastructure_providers[].config_ref must start with secret://system/",
            ));
        }
        for field in &provider.config_schema {
            validate_non_empty(&field.key, "infrastructure_providers[].config_schema[].key")?;
            validate_non_empty(
                &field.label,
                "infrastructure_providers[].config_schema[].label",
            )?;
            validate_non_empty(
                &field.field_type,
                "infrastructure_providers[].config_schema[].type",
            )?;
        }
    }
    for provider in &manifest.scope_providers {
        validate_scope_provider_contribution(provider)?;
    }
    for route in &manifest.routes {
        validate_non_empty(&route.route_id, "routes[].route_id")?;
        validate_route_method(&route.method)?;
        if !is_controlled_host_route_path(&route.path) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "routes[].path must start with /api/system/ or /api/callbacks/",
            ));
        }
        validate_non_empty(&route.action.resource, "routes[].action.resource")?;
        validate_non_empty(&route.action.action, "routes[].action.action")?;
    }
    validate_settings_features(manifest)?;
    validate_console_contributions(manifest)?;
    validate_console_surfaces(&manifest.extension_id, &manifest.console_surfaces)?;
    for worker in &manifest.workers {
        validate_non_empty(&worker.worker_id, "workers[].worker_id")?;
        validate_extension_owned_id(
            &manifest.extension_id,
            &worker.worker_id,
            "workers[].worker_id",
        )?;
        validate_non_empty(&worker.queue, "workers[].queue")?;
        validate_non_empty(&worker.handler, "workers[].handler")?;
    }
    for migration in &manifest.migrations {
        validate_non_empty(&migration.id, "migrations[].id")?;
        if !migration.path.starts_with("migrations/postgres/") || !migration.path.ends_with(".sql")
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "migrations[].path must start with migrations/postgres/ and end with .sql",
            ));
        }
    }

    Ok(())
}

fn validate_console_contributions(
    manifest: &HostExtensionContributionManifest,
) -> FrameworkResult<()> {
    let declared_i18n_refs = manifest
        .console_locale_catalog
        .texts
        .iter()
        .map(|text| text.reference.clone())
        .collect::<BTreeSet<_>>();
    if declared_i18n_refs.len() != manifest.console_locale_catalog.texts.len() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_locale_catalog.texts[].reference must be unique",
        ));
    }
    for text in &manifest.console_locale_catalog.texts {
        validate_i18n_ref(
            &manifest.extension_id,
            &text.reference,
            "console_locale_catalog.texts[].reference",
        )?;
        validate_non_empty(&text.en_us, "console_locale_catalog.texts[].en_us")?;
        validate_non_empty(&text.zh_hans, "console_locale_catalog.texts[].zh_hans")?;
    }

    let mut operation_ids = BTreeSet::<String>::new();
    let mut resource_codes = BTreeSet::<String>::new();
    let mut route_owners = BTreeMap::<(String, String), String>::new();
    let mut referenced_i18n_refs = BTreeSet::<String>::new();

    for feature in &manifest.settings_features {
        validate_i18n_reference(
            manifest,
            &feature.console_surface.label_key,
            "settings_features[].console_surface.label_key",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
        validate_i18n_reference(
            manifest,
            &feature.console_surface.description_key,
            "settings_features[].console_surface.description_key",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
    }

    for operation in &manifest.console_operations {
        validate_console_operation(
            manifest,
            operation,
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
            &mut operation_ids,
            &mut route_owners,
        )?;
    }

    for resource in &manifest.console_resources {
        validate_console_resource(
            manifest,
            resource,
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
            &mut resource_codes,
        )?;
    }

    let resources = manifest
        .console_resources
        .iter()
        .map(|resource| (resource.resource_code.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    for operation in &manifest.console_operations {
        if let ConsoleAuthorization::ResourceAction {
            resource_code,
            action_code,
        } = &operation.authorization
        {
            let Some(resource) = resources.get(resource_code.as_str()) else {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "console_operations[].authorization references unknown resource",
                ));
            };
            if !resource
                .actions
                .iter()
                .any(|action| action.action_code == *action_code)
            {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "console_operations[].authorization references unknown resource action",
                ));
            }
        }
    }

    let used_other_groups = manifest
        .console_operations
        .iter()
        .filter_map(|operation| match &operation.policy_group {
            ConsolePolicyGroup::Other(group_id) => Some(group_id.as_str()),
            ConsolePolicyGroup::SettingsFeature(_) => None,
        })
        .collect::<BTreeSet<_>>();
    let mut declared_other_groups = BTreeSet::new();
    for group in &manifest.console_locale_catalog.policy_groups {
        validate_non_empty(
            &group.group_id,
            "console_locale_catalog.policy_groups[].group_id",
        )?;
        if !is_extension_owned_group_id(&manifest.extension_id, &group.group_id) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_locale_catalog.policy_groups[].group_id must stay in the extension namespace",
            ));
        }
        if !declared_other_groups.insert(group.group_id.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_locale_catalog.policy_groups[].group_id must be unique",
            ));
        }
        if !used_other_groups.contains(group.group_id.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_locale_catalog.policy_groups[] contains an unreferenced group",
            ));
        }
        validate_i18n_reference(
            manifest,
            &group.label_ref,
            "console_locale_catalog.policy_groups[].label_ref",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
        validate_i18n_reference(
            manifest,
            &group.description_ref,
            "console_locale_catalog.policy_groups[].description_ref",
            &declared_i18n_refs,
            &mut referenced_i18n_refs,
        )?;
    }
    if used_other_groups
        .iter()
        .any(|group_id| !declared_other_groups.contains(group_id))
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_operations[].policy_group is missing a locale catalog display",
        ));
    }

    if referenced_i18n_refs.len() != declared_i18n_refs.len()
        || declared_i18n_refs
            .iter()
            .any(|reference| !referenced_i18n_refs.contains(reference))
    {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_locale_catalog.texts[] contains an unused reference",
        ));
    }

    Ok(())
}

fn validate_console_operation(
    manifest: &HostExtensionContributionManifest,
    operation: &ConsoleOperationRegistration,
    declared_i18n_refs: &BTreeSet<String>,
    referenced_i18n_refs: &mut BTreeSet<String>,
    operation_ids: &mut BTreeSet<String>,
    route_owners: &mut BTreeMap<(String, String), String>,
) -> FrameworkResult<()> {
    let field = "console_operations[]";
    validate_non_empty(&operation.operation_id, &format!("{field}.operation_id"))?;
    if !operation_ids.insert(operation.operation_id.clone()) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_operations[].operation_id must be unique",
        ));
    }
    validate_console_owner(manifest, &operation.owner, &format!("{field}.owner"))?;
    validate_extension_owned_id(
        &manifest.extension_id,
        &operation.operation_id,
        &format!("{field}.operation_id"),
    )?;
    validate_active_lifecycle(operation.lifecycle, &format!("{field}.lifecycle"))?;
    validate_console_policy_group(manifest, &operation.policy_group, field)?;
    validate_i18n_reference(
        manifest,
        &operation.label_ref,
        &format!("{field}.label_ref"),
        declared_i18n_refs,
        referenced_i18n_refs,
    )?;
    if let Some(description_ref) = operation.description_ref.as_deref() {
        validate_i18n_reference(
            manifest,
            description_ref,
            &format!("{field}.description_ref"),
            declared_i18n_refs,
            referenced_i18n_refs,
        )?;
    }
    if operation.routes.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_operations[].routes must not be empty",
        ));
    }

    for route in &operation.routes {
        validate_console_route(route, field, route_owners, &operation.operation_id)?;
    }

    Ok(())
}

fn validate_console_resource(
    manifest: &HostExtensionContributionManifest,
    resource: &ResourceAccessRegistration,
    declared_i18n_refs: &BTreeSet<String>,
    referenced_i18n_refs: &mut BTreeSet<String>,
    resource_codes: &mut BTreeSet<String>,
) -> FrameworkResult<()> {
    let field = "console_resources[]";
    validate_non_empty(&resource.resource_code, &format!("{field}.resource_code"))?;
    if !resource_codes.insert(resource.resource_code.clone()) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_resources[].resource_code must be unique",
        ));
    }
    validate_console_owner(manifest, &resource.owner, &format!("{field}.owner"))?;
    validate_extension_owned_id(
        &manifest.extension_id,
        &resource.resource_code,
        &format!("{field}.resource_code"),
    )?;
    validate_active_lifecycle(resource.lifecycle, &format!("{field}.lifecycle"))?;
    validate_non_empty(&resource.identity_field, &format!("{field}.identity_field"))?;
    validate_optional_non_empty(
        resource.scope_field.as_deref(),
        &format!("{field}.scope_field"),
    )?;
    validate_optional_non_empty(
        resource.owner_field.as_deref(),
        &format!("{field}.owner_field"),
    )?;
    validate_i18n_reference(
        manifest,
        &resource.label_ref,
        &format!("{field}.label_ref"),
        declared_i18n_refs,
        referenced_i18n_refs,
    )?;
    if let Some(description_ref) = resource.description_ref.as_deref() {
        validate_i18n_reference(
            manifest,
            description_ref,
            &format!("{field}.description_ref"),
            declared_i18n_refs,
            referenced_i18n_refs,
        )?;
    }
    if resource.actions.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_resources[].actions must not be empty",
        ));
    }

    let mut action_codes = BTreeSet::new();
    for action in &resource.actions {
        validate_console_resource_action(
            manifest,
            action,
            declared_i18n_refs,
            referenced_i18n_refs,
            &mut action_codes,
        )?;
    }

    Ok(())
}

fn validate_console_resource_action(
    manifest: &HostExtensionContributionManifest,
    action: &ResourceAccessAction,
    declared_i18n_refs: &BTreeSet<String>,
    referenced_i18n_refs: &mut BTreeSet<String>,
    action_codes: &mut BTreeSet<String>,
) -> FrameworkResult<()> {
    let field = "console_resources[].actions[]";
    validate_non_empty(&action.action_code, &format!("{field}.action_code"))?;
    if !action_codes.insert(action.action_code.clone()) {
        return Err(PluginFrameworkError::invalid_provider_package(
            "console_resources[].actions[].action_code must be unique",
        ));
    }
    validate_i18n_reference(
        manifest,
        &action.label_ref,
        &format!("{field}.label_ref"),
        declared_i18n_refs,
        referenced_i18n_refs,
    )?;
    if let Some(description_ref) = action.description_ref.as_deref() {
        validate_i18n_reference(
            manifest,
            description_ref,
            &format!("{field}.description_ref"),
            declared_i18n_refs,
            referenced_i18n_refs,
        )?;
    }
    Ok(())
}

fn validate_console_owner(
    manifest: &HostExtensionContributionManifest,
    owner: &access_control::ConsoleOperationOwner,
    field: &str,
) -> FrameworkResult<()> {
    if owner.kind != SettingsFeatureOwnerKind::HostExtension {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.kind must be host_extension"
        )));
    }
    if owner.owner_id != manifest.extension_id {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.owner_id must equal extension_id"
        )));
    }
    if owner.version != manifest.version {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.version must equal extension version"
        )));
    }
    validate_non_empty(&owner.owner_id, &format!("{field}.owner_id"))?;
    validate_non_empty(&owner.version, &format!("{field}.version"))?;
    Ok(())
}

fn validate_active_lifecycle(
    lifecycle: SettingsFeatureLifecycle,
    field: &str,
) -> FrameworkResult<()> {
    if lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must be active"
        )));
    }
    Ok(())
}

fn validate_console_policy_group(
    manifest: &HostExtensionContributionManifest,
    group: &ConsolePolicyGroup,
    field: &str,
) -> FrameworkResult<()> {
    match group {
        ConsolePolicyGroup::SettingsFeature(feature_id) => {
            if !manifest
                .settings_features
                .iter()
                .any(|feature| feature.feature_id == *feature_id)
            {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "{field}.policy_group references unknown settings feature"
                )));
            }
        }
        ConsolePolicyGroup::Other(group_id) => {
            validate_non_empty(group_id, &format!("{field}.policy_group"))?;
            if !is_extension_owned_group_id(&manifest.extension_id, group_id) {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "{field}.policy_group must stay in the extension namespace"
                )));
            }
        }
    }
    Ok(())
}

fn validate_console_route(
    route: &access_control::ConsoleRouteBinding,
    field: &str,
    route_owners: &mut BTreeMap<(String, String), String>,
    operation_id: &str,
) -> FrameworkResult<()> {
    let method = route.method.to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "HEAD" | "OPTIONS" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.routes[].method is unsupported"
        )));
    }
    if !route.path.starts_with("/api/console/") && route.path != "/api/console" {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.routes[].path must start with /api/console/"
        )));
    }

    let shape = console_route_shape(&route.path);
    let key = (method.clone(), shape.clone());
    if route_owners.contains_key(&key)
        || route_owners
            .iter()
            .any(|((existing_method, existing_shape), _)| {
                existing_method == &method
                    && console_route_templates_are_ambiguous(existing_shape, &shape)
            })
    {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field}.routes contains duplicate or ambiguous ownership"
        )));
    }
    route_owners.insert(key, operation_id.to_string());
    Ok(())
}

fn validate_i18n_ref(extension_id: &str, reference: &str, field: &str) -> FrameworkResult<()> {
    validate_non_empty(reference, field)?;
    validate_extension_owned_id(extension_id, reference, field)
}

fn validate_i18n_reference(
    manifest: &HostExtensionContributionManifest,
    reference: &str,
    field: &str,
    declared_i18n_refs: &BTreeSet<String>,
    referenced_i18n_refs: &mut BTreeSet<String>,
) -> FrameworkResult<()> {
    validate_i18n_ref(&manifest.extension_id, reference, field)?;
    if !declared_i18n_refs.contains(reference) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} references unknown console_i18n_refs entry"
        )));
    }
    referenced_i18n_refs.insert(reference.to_string());
    Ok(())
}

fn is_extension_owned_group_id(extension_id: &str, group_id: &str) -> bool {
    is_extension_owned_id(extension_id, group_id)
        || group_id == format!("other.{extension_id}")
        || group_id.starts_with(&format!("other.{extension_id}."))
}

fn console_route_shape(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.starts_with(':') || segment.starts_with('{') && segment.ends_with('}') {
                "{}"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn console_route_templates_are_ambiguous(left: &str, right: &str) -> bool {
    let left_segments = left.split('/').collect::<Vec<_>>();
    let right_segments = right.split('/').collect::<Vec<_>>();
    if left_segments.len() != right_segments.len()
        || !left_segments
            .iter()
            .zip(right_segments.iter())
            .all(|(left, right)| *left == *right || *left == "{}" || *right == "{}")
    {
        return false;
    }

    let mut left_is_more_specific = false;
    let mut right_is_more_specific = false;
    for (left, right) in left_segments.iter().zip(right_segments.iter()) {
        match (*left == "{}", *right == "{}") {
            (false, true) => left_is_more_specific = true,
            (true, false) => right_is_more_specific = true,
            (false, false) | (true, true) => {}
        }
    }

    left_is_more_specific == right_is_more_specific
}

fn validate_settings_features(manifest: &HostExtensionContributionManifest) -> FrameworkResult<()> {
    for registration in &manifest.settings_features {
        if registration.owner.kind != SettingsFeatureOwnerKind::HostExtension {
            return Err(PluginFrameworkError::invalid_provider_package(
                "settings_features[].owner.kind must be host_extension",
            ));
        }
        if registration.owner.owner_id != manifest.extension_id {
            return Err(PluginFrameworkError::invalid_provider_package(
                "settings_features[].owner.owner_id must equal extension_id",
            ));
        }
        if registration.owner.version != manifest.version {
            return Err(PluginFrameworkError::invalid_provider_package(
                "settings_features[].owner.version must equal extension version",
            ));
        }
        validate_extension_owned_id(
            &manifest.extension_id,
            &registration.feature_id,
            "settings_features[].feature_id",
        )?;
        validate_extension_owned_id(
            &manifest.extension_id,
            &registration.console_surface.route_id,
            "settings_features[].console_surface.route_id",
        )?;
    }

    SettingsFeatureRegistry::compile(manifest.settings_features.clone()).map_err(|error| {
        PluginFrameworkError::invalid_provider_package(format!(
            "invalid settings_features registration: {error}"
        ))
    })?;

    Ok(())
}

fn validate_console_surfaces(
    extension_id: &str,
    console_surfaces: &HostExtensionConsoleSurfacesManifest,
) -> FrameworkResult<()> {
    let mut route_ids = BTreeSet::new();
    let mut route_paths = BTreeSet::new();
    let mut item_ids = BTreeSet::new();
    let mut binding_ids = BTreeSet::new();

    for route in &console_surfaces.route_definitions {
        validate_non_empty(
            &route.route_id,
            "console_surfaces.route_definitions[].route_id",
        )?;
        validate_extension_owned_id(
            extension_id,
            &route.route_id,
            "console_surfaces.route_definitions[].route_id",
        )?;
        validate_non_empty(
            &route.surface_key,
            "console_surfaces.route_definitions[].surface_key",
        )?;
        validate_non_empty(&route.path, "console_surfaces.route_definitions[].path")?;
        if !route.path.starts_with("/settings/") {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_surfaces.route_definitions[].path must start with /settings/",
            ));
        }
        if route.surface_kind != HostExtensionConsoleSurfaceKind::HostExtension {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_surfaces.route_definitions[].surface_kind must be host_extension",
            ));
        }
        validate_unique_insert(
            &mut route_ids,
            route.route_id.as_str(),
            "console_surfaces.route_definitions[].route_id",
        )?;
        validate_unique_insert(
            &mut route_paths,
            route.path.as_str(),
            "console_surfaces.route_definitions[].path",
        )?;
    }

    for item in &console_surfaces.navigation_items {
        validate_non_empty(&item.item_id, "console_surfaces.navigation_items[].item_id")?;
        validate_extension_owned_id(
            extension_id,
            &item.item_id,
            "console_surfaces.navigation_items[].item_id",
        )?;
        validate_unique_insert(
            &mut item_ids,
            item.item_id.as_str(),
            "console_surfaces.navigation_items[].item_id",
        )?;
        validate_non_empty(
            &item.route_id,
            "console_surfaces.navigation_items[].route_id",
        )?;
        validate_console_route_reference(
            &route_ids,
            &item.route_id,
            "console_surfaces.navigation_items[].route_id",
        )?;
        validate_non_empty(
            &item.parent_item_id,
            "console_surfaces.navigation_items[].parent_item_id",
        )?;
        if item.parent_item_id != "settings"
            && !is_extension_owned_id(extension_id, &item.parent_item_id)
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_surfaces.navigation_items[].parent_item_id must be settings or equal extension_id or start with <extension_id>.",
            ));
        }
        validate_non_empty(
            &item.label_key,
            "console_surfaces.navigation_items[].label_key",
        )?;
        if item.navigation_slot != HostExtensionConsoleNavigationSlot::Settings {
            return Err(PluginFrameworkError::invalid_provider_package(
                "console_surfaces.navigation_items[].navigation_slot must be settings",
            ));
        }
    }

    for binding in &console_surfaces.permission_bindings {
        validate_non_empty(
            &binding.binding_id,
            "console_surfaces.permission_bindings[].binding_id",
        )?;
        validate_extension_owned_id(
            extension_id,
            &binding.binding_id,
            "console_surfaces.permission_bindings[].binding_id",
        )?;
        validate_unique_insert(
            &mut binding_ids,
            binding.binding_id.as_str(),
            "console_surfaces.permission_bindings[].binding_id",
        )?;
        validate_non_empty(
            &binding.route_id,
            "console_surfaces.permission_bindings[].route_id",
        )?;
        validate_console_route_reference(
            &route_ids,
            &binding.route_id,
            "console_surfaces.permission_bindings[].route_id",
        )?;
        for permission_code in &binding.permission_codes {
            validate_non_empty(
                permission_code,
                "console_surfaces.permission_bindings[].permission_codes[]",
            )?;
        }
        match binding.requirement {
            HostExtensionConsolePermissionRequirement::AnyPermission => {
                if binding.permission_codes.is_empty() {
                    return Err(PluginFrameworkError::invalid_provider_package(
                        "console_surfaces.permission_bindings[].permission_codes must not be empty when requirement is any_permission",
                    ));
                }
            }
            HostExtensionConsolePermissionRequirement::Authenticated => {
                if !binding.permission_codes.is_empty() {
                    return Err(PluginFrameworkError::invalid_provider_package(
                        "console_surfaces.permission_bindings[].permission_codes must be empty when requirement is authenticated",
                    ));
                }
            }
        }
    }

    Ok(())
}

fn validate_unique_insert<'a>(
    seen: &mut BTreeSet<&'a str>,
    value: &'a str,
    field: &str,
) -> FrameworkResult<()> {
    if !seen.insert(value) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must be unique"
        )));
    }
    Ok(())
}

fn validate_extension_owned_id(
    extension_id: &str,
    candidate: &str,
    field: &str,
) -> FrameworkResult<()> {
    if !is_extension_owned_id(extension_id, candidate) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must equal extension_id or start with <extension_id>."
        )));
    }
    Ok(())
}

fn validate_console_route_reference(
    route_ids: &BTreeSet<&str>,
    route_id: &str,
    field: &str,
) -> FrameworkResult<()> {
    if !route_ids.contains(route_id) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must reference console_surfaces.route_definitions[].route_id"
        )));
    }
    Ok(())
}

fn validate_route_method(method: &str) -> FrameworkResult<()> {
    match method {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" => Ok(()),
        _ => Err(PluginFrameworkError::invalid_provider_package(
            "routes[].method must be GET, POST, PUT, PATCH, or DELETE",
        )),
    }
}

fn is_controlled_host_route_path(path: &str) -> bool {
    path.starts_with("/api/system/") || path.starts_with("/api/callbacks/")
}

fn is_extension_owned_id(extension_id: &str, candidate: &str) -> bool {
    candidate == extension_id
        || candidate
            .strip_prefix(extension_id)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn validate_non_empty(value: &str, field: &str) -> FrameworkResult<()> {
    if value.trim().is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

fn validate_optional_non_empty(value: Option<&str>, field: &str) -> FrameworkResult<()> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "{field} must not be empty when present"
        )));
    }
    Ok(())
}
