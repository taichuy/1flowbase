use std::{
    collections::HashSet,
    error::Error,
    fmt::{Display, Formatter},
};

use access_control::{
    ConsoleNavigation, ConsoleNavigationItem, ConsoleNavigationSlot, ConsolePermissionBinding,
    ConsolePermissionRequirement, ConsoleRouteDefinition, ConsoleSurfaceKind,
    accessible_console_navigation_with_contributions, builtin_console_navigation,
};
use domain::ActorContext;
use plugin_framework::{
    HostExtensionContributionManifest,
    host_extension_contribution::{
        HostExtensionConsoleNavigationSlot, HostExtensionConsolePermissionRequirement,
        HostExtensionConsoleSurfaceKind,
    },
};

/// Console navigation is compiled with the route and operation registries at boot. It has no
/// runtime mutation path, so a navigation read cannot observe a different HostExtension set from
/// the authorization registry that protects its API routes.
#[derive(Debug)]
pub struct ConsoleSurfaceRegistry {
    contributions: Vec<ConsoleNavigation>,
}

impl ConsoleSurfaceRegistry {
    pub fn from_host_extension_contributions<'a>(
        contributions: impl IntoIterator<Item = &'a HostExtensionContributionManifest>,
    ) -> Result<Self, ConsoleSurfaceRegistryError> {
        let builtin = builtin_console_navigation();
        let mut route_ids = builtin
            .route_definitions
            .iter()
            .map(|route| route.route_id.clone())
            .collect::<HashSet<_>>();
        let mut paths = builtin
            .route_definitions
            .iter()
            .map(|route| route.path.clone())
            .collect::<HashSet<_>>();
        let mut item_ids = builtin
            .navigation_items
            .iter()
            .map(|item| item.item_id.clone())
            .collect::<HashSet<_>>();
        let mut binding_ids = builtin
            .permission_bindings
            .iter()
            .map(|binding| binding.binding_id.clone())
            .collect::<HashSet<_>>();
        let mut compiled = Vec::new();

        for contribution in contributions {
            let navigation = host_extension_console_navigation(contribution);
            validate_console_navigation(
                &navigation,
                &mut route_ids,
                &mut paths,
                &mut item_ids,
                &mut binding_ids,
            )?;
            compiled.push(navigation);
        }

        Ok(Self {
            contributions: compiled,
        })
    }

    pub fn accessible_navigation(&self, actor: &ActorContext) -> ConsoleNavigation {
        accessible_console_navigation_with_contributions(actor, &self.contributions)
    }
}

impl Default for ConsoleSurfaceRegistry {
    fn default() -> Self {
        Self::from_host_extension_contributions([])
            .expect("builtin console navigation must have unique identifiers")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleSurfaceRegistryError {
    message: String,
}

impl ConsoleSurfaceRegistryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ConsoleSurfaceRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl Error for ConsoleSurfaceRegistryError {}

fn validate_console_navigation(
    contribution: &ConsoleNavigation,
    route_ids: &mut HashSet<String>,
    paths: &mut HashSet<String>,
    item_ids: &mut HashSet<String>,
    binding_ids: &mut HashSet<String>,
) -> Result<(), ConsoleSurfaceRegistryError> {
    for route in &contribution.route_definitions {
        ensure_unique_insert(
            route_ids,
            route.route_id.clone(),
            "duplicate console route id",
        )?;
        ensure_unique_insert(paths, route.path.clone(), "duplicate console route path")?;
    }
    for item in &contribution.navigation_items {
        ensure_unique_insert(
            item_ids,
            item.item_id.clone(),
            "duplicate console navigation item id",
        )?;
    }
    for binding in &contribution.permission_bindings {
        ensure_unique_insert(
            binding_ids,
            binding.binding_id.clone(),
            "duplicate console permission binding id",
        )?;
    }
    Ok(())
}

fn ensure_unique_insert(
    seen: &mut HashSet<String>,
    value: String,
    message: &str,
) -> Result<(), ConsoleSurfaceRegistryError> {
    if !seen.insert(value.clone()) {
        return Err(ConsoleSurfaceRegistryError::new(format!(
            "{message}: {value}"
        )));
    }
    Ok(())
}

fn host_extension_console_navigation(
    contribution: &HostExtensionContributionManifest,
) -> ConsoleNavigation {
    let surfaces = &contribution.console_surfaces;

    ConsoleNavigation {
        route_definitions: surfaces
            .route_definitions
            .iter()
            .map(|route| ConsoleRouteDefinition {
                route_id: route.route_id.clone(),
                surface_key: route.surface_key.clone(),
                path: route.path.clone(),
                surface_kind: console_surface_kind(route.surface_kind),
            })
            .collect(),
        navigation_items: surfaces
            .navigation_items
            .iter()
            .map(|item| ConsoleNavigationItem {
                item_id: item.item_id.clone(),
                route_id: item.route_id.clone(),
                parent_item_id: Some(item.parent_item_id.clone()),
                label_key: item.label_key.clone(),
                navigation_slot: console_navigation_slot(item.navigation_slot),
                order: item.order,
            })
            .collect(),
        permission_bindings: surfaces
            .permission_bindings
            .iter()
            .map(|binding| ConsolePermissionBinding {
                binding_id: binding.binding_id.clone(),
                route_id: binding.route_id.clone(),
                permission_codes: binding.permission_codes.clone(),
                requirement: console_permission_requirement(binding.requirement),
            })
            .collect(),
    }
}

fn console_surface_kind(kind: HostExtensionConsoleSurfaceKind) -> ConsoleSurfaceKind {
    match kind {
        HostExtensionConsoleSurfaceKind::System => ConsoleSurfaceKind::System,
        HostExtensionConsoleSurfaceKind::DynamicPage => ConsoleSurfaceKind::DynamicPage,
        HostExtensionConsoleSurfaceKind::HostExtension => ConsoleSurfaceKind::HostExtension,
    }
}

fn console_navigation_slot(slot: HostExtensionConsoleNavigationSlot) -> ConsoleNavigationSlot {
    match slot {
        HostExtensionConsoleNavigationSlot::Primary => ConsoleNavigationSlot::Primary,
        HostExtensionConsoleNavigationSlot::Secondary => ConsoleNavigationSlot::Secondary,
        HostExtensionConsoleNavigationSlot::Settings => ConsoleNavigationSlot::Settings,
    }
}

fn console_permission_requirement(
    requirement: HostExtensionConsolePermissionRequirement,
) -> ConsolePermissionRequirement {
    match requirement {
        HostExtensionConsolePermissionRequirement::Authenticated => {
            ConsolePermissionRequirement::Authenticated
        }
        HostExtensionConsolePermissionRequirement::AnyPermission => {
            ConsolePermissionRequirement::AnyPermission
        }
    }
}
