use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{Display, Formatter},
};

use serde::{Deserialize, Serialize};

pub const SETTINGS_FEATURE_INVENTORY_SCHEMA_VERSION: &str =
    "1flowbase.settings-feature-inventory/v1";
pub const SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID: &str = "system.applications";
pub const SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.applications";
pub const SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_ID: &str = "system.api-key-authentication";
pub const SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.api-key-authentication";
pub const SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_ID: &str = "system.auth-center";
pub const SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.auth-center";
pub const SYSTEM_BACKUPS_SETTINGS_FEATURE_ID: &str = "system.backups";
pub const SYSTEM_BACKUPS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.backups";
pub const SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID: &str = "system.data-models";
pub const SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.data-models";
pub const SYSTEM_DOCS_SETTINGS_FEATURE_ID: &str = "system.docs";
pub const SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION: &str = "settings_feature.access.system.docs";
pub const SYSTEM_EXTENSION_CENTER_SETTINGS_FEATURE_ID: &str = "system.extension-center";
pub const SYSTEM_EXTENSION_CENTER_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.extension-center";
pub const SYSTEM_FILES_SETTINGS_FEATURE_ID: &str = "system.files";
pub const SYSTEM_FILES_SETTINGS_FEATURE_PERMISSION: &str = "settings_feature.access.system.files";
pub const SYSTEM_I18N_CATALOG_SETTINGS_FEATURE_ID: &str = "system.i18n-catalog";
pub const SYSTEM_I18N_CATALOG_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.i18n-catalog";
pub const SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_ID: &str = "system.memory-observation";
pub const SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.memory-observation";
pub const SYSTEM_NOTIFICATIONS_SETTINGS_FEATURE_ID: &str = "system.notifications";
pub const SYSTEM_MEMBERS_SETTINGS_FEATURE_ID: &str = "system.members";
pub const SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.members";
pub const SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_ID: &str = "system.mcp-management";
pub const SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.mcp-management";
pub const SYSTEM_NETWORK_CENTER_SETTINGS_FEATURE_ID: &str = "system.network-center";
pub const SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_ID: &str = "system.model-providers";
pub const SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.model-providers";
pub const SYSTEM_ROLES_SETTINGS_FEATURE_ID: &str = "system.roles";
pub const SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION: &str = "settings_feature.access.system.roles";
pub const SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_ID: &str = "system.system-runtime";
pub const SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.system-runtime";
pub const SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_ID: &str = "system.ui-management";
pub const SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.ui-management";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsFeatureOwnerKind {
    Core,
    HostExtension,
    ManagedService,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsFeatureOwner {
    pub kind: SettingsFeatureOwnerKind,
    pub owner_id: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsFeatureLifecycle {
    Active,
    Inactive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsFeatureConsoleSurface {
    pub route_id: String,
    pub surface_key: String,
    pub path: String,
    pub label_key: String,
    pub description_key: String,
    pub order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsApiRoute {
    pub method: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsFeatureRegistration {
    pub feature_id: String,
    pub owner: SettingsFeatureOwner,
    pub lifecycle: SettingsFeatureLifecycle,
    pub console_surface: SettingsFeatureConsoleSurface,
    pub api_routes: Vec<SettingsApiRoute>,
}

impl SettingsFeatureRegistration {
    pub fn permission_code(&self) -> String {
        format!("settings_feature.access.{}", self.feature_id)
    }
}

pub fn settings_feature_permission_definitions() -> Vec<domain::PermissionDefinition> {
    core_settings_feature_registrations()
        .into_iter()
        .map(|registration| domain::PermissionDefinition {
            code: registration.permission_code(),
            resource: "settings_feature".to_string(),
            action: "access".to_string(),
            scope: registration.feature_id.clone(),
            name: format!("settings_feature:access:{}", registration.feature_id),
        })
        .collect()
}

fn settings_api_routes(routes: &[(&str, &str)]) -> Vec<SettingsApiRoute> {
    routes
        .iter()
        .map(|(method, path)| SettingsApiRoute {
            method: (*method).to_string(),
            path: (*path).to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccessRule {
    Public,
    Authenticated,
    Action { resource: String, action: String },
    SettingsFeature(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsFeatureInventoryEntry {
    pub feature_id: String,
    pub permission_code: String,
    pub owner: SettingsFeatureOwner,
    pub lifecycle: SettingsFeatureLifecycle,
    pub console_surface: SettingsFeatureConsoleSurface,
    pub api_routes: Vec<SettingsApiRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsFeatureCompiledInventory {
    pub schema_version: &'static str,
    pub features: Vec<SettingsFeatureInventoryEntry>,
}

#[derive(Debug)]
pub struct SettingsFeatureRegistry {
    inventory: SettingsFeatureCompiledInventory,
    access_rules: BTreeMap<(String, String), AccessRule>,
}

impl SettingsFeatureRegistry {
    pub fn compile(
        registrations: impl IntoIterator<Item = SettingsFeatureRegistration>,
    ) -> Result<Self, SettingsFeatureRegistryError> {
        let mut features = BTreeMap::new();
        let mut access_rules = BTreeMap::new();

        for mut registration in registrations {
            validate_registration(&registration)?;
            if features.contains_key(&registration.feature_id) {
                return Err(SettingsFeatureRegistryError::new(format!(
                    "duplicate feature_id {}",
                    registration.feature_id
                )));
            }

            registration.api_routes = registration
                .api_routes
                .into_iter()
                .map(normalize_api_route)
                .collect();
            registration.api_routes.sort();

            for route in &registration.api_routes {
                let route_key = (route.method.clone(), route.path.clone());
                if access_rules.contains_key(&route_key) {
                    return Err(SettingsFeatureRegistryError::new(format!(
                        "duplicate Settings API ownership {} {}",
                        route.method, route.path
                    )));
                }
                access_rules.insert(
                    route_key,
                    AccessRule::SettingsFeature(registration.feature_id.clone()),
                );
            }

            features.insert(registration.feature_id.clone(), registration);
        }

        let features = features
            .into_values()
            .map(|registration| SettingsFeatureInventoryEntry {
                permission_code: registration.permission_code(),
                feature_id: registration.feature_id,
                owner: registration.owner,
                lifecycle: registration.lifecycle,
                console_surface: registration.console_surface,
                api_routes: registration.api_routes,
            })
            .collect();

        Ok(Self {
            inventory: SettingsFeatureCompiledInventory {
                schema_version: SETTINGS_FEATURE_INVENTORY_SCHEMA_VERSION,
                features,
            },
            access_rules,
        })
    }

    pub fn inventory(&self) -> &SettingsFeatureCompiledInventory {
        &self.inventory
    }

    pub fn access_rule(&self, method: &str, path: &str) -> Option<&AccessRule> {
        let method = method.to_ascii_uppercase();
        self.access_rules
            .get(&(method.clone(), path.to_string()))
            .or_else(|| {
                self.access_rules
                    .iter()
                    .find_map(|((route_method, route_path), rule)| {
                        (route_method == &method && settings_api_route_matches(route_path, path))
                            .then_some(rule)
                    })
            })
    }
}

fn settings_api_route_matches(route_template: &str, request_path: &str) -> bool {
    let template_segments = route_template.split('/').collect::<Vec<_>>();
    let request_segments = request_path.split('/').collect::<Vec<_>>();
    template_segments.len() == request_segments.len()
        && template_segments
            .iter()
            .zip(request_segments)
            .all(|(template, actual)| {
                (template.starts_with('{') && template.ends_with('}') && !actual.is_empty())
                    || template == &actual
            })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsFeatureRegistryError {
    message: String,
}

impl SettingsFeatureRegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for SettingsFeatureRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for SettingsFeatureRegistryError {}

fn validate_registration(
    registration: &SettingsFeatureRegistration,
) -> Result<(), SettingsFeatureRegistryError> {
    validate_non_empty(&registration.feature_id, "settings feature_id")?;
    validate_non_empty(&registration.owner.owner_id, "settings feature owner_id")?;
    validate_non_empty(
        &registration.owner.version,
        "settings feature owner version",
    )?;
    validate_non_empty(
        &registration.console_surface.route_id,
        "settings feature console route_id",
    )?;
    validate_non_empty(
        &registration.console_surface.surface_key,
        "settings feature console surface_key",
    )?;
    validate_non_empty(
        &registration.console_surface.label_key,
        "settings feature console label_key",
    )?;
    validate_non_empty(
        &registration.console_surface.description_key,
        "settings feature console description_key",
    )?;
    if !registration.console_surface.path.starts_with("/settings/") {
        return Err(SettingsFeatureRegistryError::new(
            "settings feature console path must start with /settings/",
        ));
    }
    if registration.lifecycle == SettingsFeatureLifecycle::Inactive {
        return Err(SettingsFeatureRegistryError::new(format!(
            "inactive settings feature {} cannot own API routes",
            registration.feature_id
        )));
    }

    for route in &registration.api_routes {
        validate_api_route(route)?;
    }

    Ok(())
}

fn validate_api_route(route: &SettingsApiRoute) -> Result<(), SettingsFeatureRegistryError> {
    let method = route.method.to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "HEAD" | "OPTIONS" | "POST" | "PUT" | "PATCH" | "DELETE"
    ) {
        return Err(SettingsFeatureRegistryError::new(format!(
            "unsupported Settings API method {}",
            route.method
        )));
    }
    if !route.path.starts_with("/api/") {
        return Err(SettingsFeatureRegistryError::new(
            "Settings API path must start with /api/",
        ));
    }
    Ok(())
}

fn normalize_api_route(mut route: SettingsApiRoute) -> SettingsApiRoute {
    route.method.make_ascii_uppercase();
    route
}

fn validate_non_empty(value: &str, field: &str) -> Result<(), SettingsFeatureRegistryError> {
    if value.trim().is_empty() {
        return Err(SettingsFeatureRegistryError::new(format!(
            "{field} must not be empty"
        )));
    }
    Ok(())
}

#[path = "settings_features/core_registrations.rs"]
mod core_registrations;

pub use core_registrations::core_settings_feature_registrations;
