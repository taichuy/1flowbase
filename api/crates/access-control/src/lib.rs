extern crate self as access_control;

mod catalog;
mod console_operations;
mod evaluator;
mod navigation;
mod settings_features;

pub use catalog::{builtin_role_templates, permission_catalog};
pub use console_operations::{
    ConsoleAuthorization, ConsoleOperationCompiledInventory, ConsoleOperationInventoryEntry,
    ConsoleOperationOwner, ConsoleOperationRegistration, ConsoleOperationRegistry,
    ConsoleOperationRegistryDiff, ConsoleOperationRegistryError, ConsolePolicyGroup,
    ConsolePolicyGroupChange, ConsoleRouteAccess, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, APPLICATIONS_CREATE_ACTION_CODE, APPLICATIONS_CREATE_OPERATION_ID,
    APPLICATIONS_DELETE_ACTION_CODE, APPLICATIONS_DELETE_OPERATION_ID, APPLICATIONS_RESOURCE_CODE,
    APPLICATIONS_UPDATE_ACTION_CODE, APPLICATIONS_UPDATE_OPERATION_ID,
    APPLICATIONS_VIEW_ACTION_CODE, APPLICATIONS_VIEW_OPERATION_ID,
};
pub use evaluator::ensure_permission;
pub use navigation::{
    accessible_console_navigation, accessible_console_navigation_with_contributions,
    builtin_console_navigation, ConsoleNavigation, ConsoleNavigationItem, ConsoleNavigationSlot,
    ConsolePermissionBinding, ConsolePermissionRequirement, ConsoleRouteDefinition,
    ConsoleSurfaceKind,
};
pub use settings_features::{
    core_settings_feature_registrations, settings_feature_permission_definitions, AccessRule,
    SettingsApiRoute, SettingsFeatureCompiledInventory, SettingsFeatureConsoleSurface,
    SettingsFeatureInventoryEntry, SettingsFeatureLifecycle, SettingsFeatureOwner,
    SettingsFeatureOwnerKind, SettingsFeatureRegistration, SettingsFeatureRegistry,
    SettingsFeatureRegistryError, SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_ID,
    SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID, SYSTEM_APPLICATIONS_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_ID, SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID, SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_DOCS_SETTINGS_FEATURE_ID, SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_FILES_SETTINGS_FEATURE_ID, SYSTEM_FILES_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_ID,
    SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_ID, SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_MEMBERS_SETTINGS_FEATURE_ID, SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_ID,
    SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_ID, SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_ROLES_SETTINGS_FEATURE_ID, SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION,
    SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_ID, SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION,
};

pub fn crate_name() -> &'static str {
    "access-control"
}

#[cfg(test)]
mod _tests;
