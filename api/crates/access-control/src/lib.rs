extern crate self as access_control;

mod catalog;
mod evaluator;
mod navigation;
mod settings_features;
mod settings_routes;

pub use catalog::{builtin_role_templates, permission_catalog};
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
    SettingsFeatureRegistryError, SYSTEM_MEMBERS_SETTINGS_FEATURE_ID,
    SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION, SYSTEM_ROLES_SETTINGS_FEATURE_ID,
    SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION,
};
pub use settings_routes::{
    expand_permissions_with_settings_routes, settings_route_permission_definitions,
    settings_route_permissions_for_console_request, settings_route_spec_by_visibility_permission,
    settings_route_specs, SettingsRouteApiMethods, SettingsRouteApiPathMatch,
    SettingsRouteApiScope, SettingsRouteLegacyVisibility, SettingsRouteSpec,
};

pub fn crate_name() -> &'static str {
    "access-control"
}

#[cfg(test)]
mod _tests;
