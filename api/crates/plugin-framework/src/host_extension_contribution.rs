use std::collections::BTreeSet;

use access_control::{
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
    pub console_surfaces: HostExtensionConsoleSurfacesManifest,
    pub workers: Vec<HostExtensionWorkerManifest>,
    pub migrations: Vec<HostExtensionMigrationManifest>,
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
