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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthProviderContributionManifest {
    pub auth_type: String,
    pub display_name: String,
    #[serde(default)]
    pub config_schema: Vec<PluginFormFieldSchema>,
    pub default_public_ui_block: String,
    #[serde(default)]
    pub public_variable_keys: Vec<String>,
    #[serde(default)]
    pub public_route_ids: Vec<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionInterfaceOperationMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HostExtensionInterfaceOperationMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionInterfaceOperationContractManifest {
    pub contract_id: String,
    pub contract_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionInterfaceOperationAuthPolicy {
    CoreConsoleOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionInterfaceOperationAuditPolicy {
    ReadOnly,
    Mutating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionInterfaceOperationErrorPolicy {
    CoreApiError,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionInterfaceOperationManifest {
    pub operation_id: String,
    pub method: HostExtensionInterfaceOperationMethod,
    pub path: String,
    pub input: HostExtensionInterfaceOperationContractManifest,
    pub output: HostExtensionInterfaceOperationContractManifest,
    pub required_core_permission: String,
    pub auth_policy: HostExtensionInterfaceOperationAuthPolicy,
    pub audit_policy: HostExtensionInterfaceOperationAuditPolicy,
    pub error_policy: HostExtensionInterfaceOperationErrorPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExtensionInterfaceAuthenticationPrincipalProfile {
    Public,
    User,
    Application,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionInterfaceAuthenticationManifest {
    pub contribution_id: String,
    pub interface_id: String,
    pub interface_version: String,
    pub binding_ids: Vec<String>,
    pub adapter_id: String,
    pub activation_id: String,
    pub principal_profile: HostExtensionInterfaceAuthenticationPrincipalProfile,
    pub credential: HostExtensionInterfaceOperationContractManifest,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionLifecycleContractManifest {
    pub contract_id: String,
    pub contract_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionLifecycleSubscriptionManifest {
    pub subscription_id: String,
    pub point_id: String,
    pub fact: HostExtensionLifecycleContractManifest,
    pub handler: HostExtensionLifecycleContractManifest,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostExtensionContributionManifest {
    #[serde(default)]
    pub settings_pages: Vec<crate::PluginSettingsPageManifest>,
    pub schema_version: String,
    pub extension_id: String,
    pub version: String,
    pub bootstrap_phase: HostExtensionBootstrapPhase,
    pub native: HostExtensionNativeEntrypointManifest,
    pub owned_resources: Vec<String>,
    pub extends_resources: Vec<String>,
    pub infrastructure_providers: Vec<HostInfrastructureProviderManifest>,
    #[serde(default)]
    pub auth_providers: Vec<AuthProviderContributionManifest>,
    #[serde(default)]
    pub scope_providers: Vec<ScopeProviderContributionManifest>,
    #[serde(default)]
    pub interface_operations: Vec<HostExtensionInterfaceOperationManifest>,
    #[serde(default)]
    pub interface_authentication_adapters: Vec<HostExtensionInterfaceAuthenticationManifest>,
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
    #[serde(default)]
    pub lifecycle_subscriptions: Vec<HostExtensionLifecycleSubscriptionManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostExtensionConsoleContribution {
    pub operations: Vec<ConsoleOperationRegistration>,
    pub resources: Vec<ResourceAccessRegistration>,
    pub locale_catalog: ConsoleLocaleCatalogContribution,
}

impl HostExtensionContributionManifest {
    pub fn validate_package_settings_pages(
        &self,
        package: &crate::PluginManifestV1,
    ) -> FrameworkResult<()> {
        if package.plugin_code()? != self.extension_id
            || package.version != self.version
            || package.settings_pages != self.settings_pages
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "native settings page declarations do not match the package",
            ));
        }
        Ok(())
    }

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
    let mut interface_operation_ids = BTreeSet::new();
    let mut interface_operation_routes = BTreeSet::new();
    for operation in &manifest.interface_operations {
        validate_non_empty(
            &operation.operation_id,
            "interface_operations[].operation_id",
        )?;
        if !interface_operation_ids.insert(operation.operation_id.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "interface_operations[].operation_id must be unique",
            ));
        }
        if !operation.path.starts_with("/api/console/") {
            return Err(PluginFrameworkError::invalid_provider_package(
                "interface_operations[].path must start with /api/console/",
            ));
        }
        if !interface_operation_routes.insert((operation.method, operation.path.as_str())) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "interface_operations[] method/path must be unique",
            ));
        }
        validate_non_empty(
            &operation.input.contract_id,
            "interface_operations[].input.contract_id",
        )?;
        validate_non_empty(
            &operation.input.contract_version,
            "interface_operations[].input.contract_version",
        )?;
        validate_non_empty(
            &operation.output.contract_id,
            "interface_operations[].output.contract_id",
        )?;
        validate_non_empty(
            &operation.output.contract_version,
            "interface_operations[].output.contract_version",
        )?;
        validate_non_empty(
            &operation.required_core_permission,
            "interface_operations[].required_core_permission",
        )?;
    }
    let mut authentication_contribution_ids = BTreeSet::new();
    let mut authentication_targets = BTreeSet::new();
    for authentication in &manifest.interface_authentication_adapters {
        validate_non_empty(
            &authentication.contribution_id,
            "interface_authentication_adapters[].contribution_id",
        )?;
        validate_extension_owned_id(
            &manifest.extension_id,
            &authentication.contribution_id,
            "interface_authentication_adapters[].contribution_id",
        )?;
        if !authentication_contribution_ids.insert(authentication.contribution_id.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "interface_authentication_adapters[].contribution_id must be unique",
            ));
        }
        validate_non_empty(
            &authentication.interface_id,
            "interface_authentication_adapters[].interface_id",
        )?;
        validate_non_empty(
            &authentication.interface_version,
            "interface_authentication_adapters[].interface_version",
        )?;
        if !authentication_targets.insert((
            authentication.interface_id.as_str(),
            authentication.interface_version.as_str(),
        )) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "interface_authentication_adapters[] interface identity must be unique",
            ));
        }
        if authentication.binding_ids.is_empty() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "interface_authentication_adapters[].binding_ids must not be empty",
            ));
        }
        let mut binding_ids = BTreeSet::new();
        for binding_id in &authentication.binding_ids {
            validate_non_empty(
                binding_id,
                "interface_authentication_adapters[].binding_ids[]",
            )?;
            if !binding_ids.insert(binding_id.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "interface_authentication_adapters[].binding_ids must be unique",
                ));
            }
        }
        validate_non_empty(
            &authentication.adapter_id,
            "interface_authentication_adapters[].adapter_id",
        )?;
        validate_non_empty(
            &authentication.activation_id,
            "interface_authentication_adapters[].activation_id",
        )?;
        validate_non_empty(
            &authentication.credential.contract_id,
            "interface_authentication_adapters[].credential.contract_id",
        )?;
        validate_non_empty(
            &authentication.credential.contract_version,
            "interface_authentication_adapters[].credential.contract_version",
        )?;
    }
    validate_auth_provider_contributions(manifest)?;
    let public_auth_route_ids = manifest
        .auth_providers
        .iter()
        .flat_map(|provider| provider.public_route_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    for route in &manifest.routes {
        validate_non_empty(&route.route_id, "routes[].route_id")?;
        validate_route_method(&route.method)?;
        if !is_controlled_host_route_path(&route.path)
            && !(public_auth_route_ids.contains(route.route_id.as_str())
                && route.path.starts_with("/api/public/auth/"))
        {
            return Err(PluginFrameworkError::invalid_provider_package(
                "routes[].path must start with /api/system/, /api/callbacks/, or be an auth_providers[].public_route_ids route under /api/public/auth/",
            ));
        }
        validate_non_empty(&route.action.resource, "routes[].action.resource")?;
        validate_non_empty(&route.action.action, "routes[].action.action")?;
    }
    validate_settings_features(manifest)?;
    crate::validate_plugin_settings_pages(&manifest.extension_id, &manifest.settings_pages)?;
    for page in &manifest.settings_pages {
        let feature = manifest
            .settings_features
            .iter()
            .find(|feature| feature.feature_id == page.feature_id)
            .ok_or_else(|| {
                PluginFrameworkError::invalid_provider_package(
                    "settings page references an unregistered feature",
                )
            })?;
        if feature.lifecycle != SettingsFeatureLifecycle::Active {
            return Err(PluginFrameworkError::invalid_provider_package(
                "settings page feature must be active",
            ));
        }
    }
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

fn validate_auth_provider_contributions(
    manifest: &HostExtensionContributionManifest,
) -> FrameworkResult<()> {
    let routes = manifest
        .routes
        .iter()
        .map(|route| (route.route_id.as_str(), route.path.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut auth_types = BTreeSet::new();
    let mut route_owners = BTreeMap::<&str, &str>::new();

    for provider in &manifest.auth_providers {
        validate_non_empty(&provider.auth_type, "auth_providers[].auth_type")?;
        validate_extension_owned_id(
            &manifest.extension_id,
            &provider.auth_type,
            "auth_providers[].auth_type",
        )?;
        if !auth_types.insert(provider.auth_type.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(
                "auth_providers[].auth_type must be unique",
            ));
        }
        validate_non_empty(&provider.display_name, "auth_providers[].display_name")?;
        validate_non_empty(
            &provider.default_public_ui_block,
            "auth_providers[].default_public_ui_block",
        )?;
        let mut config_keys = BTreeSet::new();
        let mut config_field_types = BTreeMap::new();
        for field in &provider.config_schema {
            validate_non_empty(&field.key, "auth_providers[].config_schema[].key")?;
            validate_non_empty(&field.label, "auth_providers[].config_schema[].label")?;
            validate_non_empty(&field.field_type, "auth_providers[].config_schema[].type")?;
            if [
                "title",
                "description",
                "enabled",
                "self_registration_enabled",
                "public_ui_block",
            ]
            .contains(&field.key.as_str())
            {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].config_schema[].key conflicts with a core authenticator field",
                ));
            }
            if !config_keys.insert(field.key.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].config_schema[].key must be unique",
                ));
            }
            config_field_types.insert(field.key.as_str(), field.field_type.as_str());
        }
        let mut public_variable_keys = BTreeSet::new();
        for key in &provider.public_variable_keys {
            validate_non_empty(key, "auth_providers[].public_variable_keys[]")?;
            if !public_variable_keys.insert(key.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].public_variable_keys[] must be unique",
                ));
            }
            if !config_keys.contains(key.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].public_variable_keys[] must reference config_schema[].key",
                ));
            }
            if config_field_types
                .get(key.as_str())
                .is_some_and(|field_type| matches!(*field_type, "secret" | "password"))
            {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].public_variable_keys[] cannot reference a secret field",
                ));
            }
        }
        let mut provider_routes = BTreeSet::new();
        for route_id in &provider.public_route_ids {
            validate_extension_owned_id(
                &manifest.extension_id,
                route_id,
                "auth_providers[].public_route_ids[]",
            )?;
            if !provider_routes.insert(route_id.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].public_route_ids[] must be unique",
                ));
            }
            let path = routes.get(route_id.as_str()).ok_or_else(|| {
                PluginFrameworkError::invalid_provider_package(
                    "auth_providers[].public_route_ids[] must reference routes[].route_id",
                )
            })?;
            if !path.starts_with("/api/public/auth/") {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth provider public routes must start with /api/public/auth/",
                ));
            }
            if route_owners
                .insert(route_id.as_str(), provider.auth_type.as_str())
                .is_some()
            {
                return Err(PluginFrameworkError::invalid_provider_package(
                    "auth provider public route must have one owner",
                ));
            }
        }
    }

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

#[path = "host_extension_contribution/console_validation.rs"]
mod console_validation;

use console_validation::*;
