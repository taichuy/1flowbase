extern crate self as access_control;

mod catalog;
mod evaluator;
mod navigation;

pub use catalog::{builtin_role_templates, permission_catalog};
pub use evaluator::ensure_permission;
pub use navigation::{
    accessible_console_navigation, accessible_console_navigation_with_contributions,
    builtin_console_navigation, ConsoleNavigation, ConsoleNavigationItem, ConsoleNavigationSlot,
    ConsolePermissionBinding, ConsolePermissionRequirement, ConsoleRouteDefinition,
    ConsoleSurfaceKind,
};

pub fn crate_name() -> &'static str {
    "access-control"
}

#[cfg(test)]
mod _tests;
