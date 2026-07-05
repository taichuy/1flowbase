use std::{
    collections::HashSet,
    error::Error,
    fmt::{Display, Formatter},
    sync::RwLock,
};

use access_control::{
    accessible_console_navigation_with_contributions, builtin_console_navigation,
    ConsoleNavigation, ConsoleNavigationItem, ConsoleNavigationSlot, ConsolePermissionBinding,
    ConsolePermissionRequirement, ConsoleRouteDefinition, ConsoleSurfaceKind,
};
use domain::ActorContext;
use plugin_framework::{
    host_extension_contribution::{
        HostExtensionConsoleNavigationSlot, HostExtensionConsolePermissionRequirement,
        HostExtensionConsoleSurfaceKind,
    },
    HostExtensionContributionManifest,
};

#[derive(Debug)]
pub struct ConsoleSurfaceRegistry {
    state: RwLock<ConsoleSurfaceRegistryState>,
}

impl ConsoleSurfaceRegistry {
    pub fn from_host_extension_contributions<'a>(
        contributions: impl IntoIterator<Item = &'a HostExtensionContributionManifest>,
    ) -> Result<Self, ConsoleSurfaceRegistryError> {
        let registry = Self::default();
        for contribution in contributions {
            registry.register_host_extension_contribution(contribution)?;
        }
        Ok(registry)
    }

    pub fn register_host_extension_contribution(
        &self,
        contribution: &HostExtensionContributionManifest,
    ) -> Result<(), ConsoleSurfaceRegistryError> {
        self.register_console_navigation(host_extension_console_navigation(contribution))
    }

    #[cfg(test)]
    pub fn register_host_extension_manifest(
        &self,
        raw: &str,
    ) -> Result<(), ConsoleSurfaceRegistryError> {
        let contribution = plugin_framework::parse_host_extension_contribution_manifest(raw)
            .map_err(|error| ConsoleSurfaceRegistryError::new(error.to_string()))?;
        self.register_host_extension_contribution(&contribution)
    }

    pub fn accessible_navigation(&self, actor: &ActorContext) -> ConsoleNavigation {
        let contributions = self
            .state
            .read()
            .expect("console surface registry lock must not be poisoned")
            .contributions
            .clone();

        accessible_console_navigation_with_contributions(actor, &contributions)
    }

    fn register_console_navigation(
        &self,
        contribution: ConsoleNavigation,
    ) -> Result<(), ConsoleSurfaceRegistryError> {
        if contribution.route_definitions.is_empty()
            && contribution.navigation_items.is_empty()
            && contribution.permission_bindings.is_empty()
        {
            return Ok(());
        }

        let mut state = self
            .state
            .write()
            .expect("console surface registry lock must not be poisoned");
        let mut next_route_ids = state.route_ids.clone();
        let mut next_paths = state.paths.clone();
        let mut next_item_ids = state.item_ids.clone();
        let mut next_binding_ids = state.binding_ids.clone();

        for route in &contribution.route_definitions {
            ensure_unique_insert(
                &mut next_route_ids,
                route.route_id.clone(),
                "duplicate console route id",
            )?;
            ensure_unique_insert(
                &mut next_paths,
                route.path.clone(),
                "duplicate console route path",
            )?;
        }
        for item in &contribution.navigation_items {
            ensure_unique_insert(
                &mut next_item_ids,
                item.item_id.clone(),
                "duplicate console navigation item id",
            )?;
        }
        for binding in &contribution.permission_bindings {
            ensure_unique_insert(
                &mut next_binding_ids,
                binding.binding_id.clone(),
                "duplicate console permission binding id",
            )?;
        }

        state.route_ids = next_route_ids;
        state.paths = next_paths;
        state.item_ids = next_item_ids;
        state.binding_ids = next_binding_ids;
        state.contributions.push(contribution);

        Ok(())
    }
}

impl Default for ConsoleSurfaceRegistry {
    fn default() -> Self {
        Self {
            state: RwLock::new(ConsoleSurfaceRegistryState::from(
                builtin_console_navigation(),
            )),
        }
    }
}

#[derive(Debug)]
struct ConsoleSurfaceRegistryState {
    contributions: Vec<ConsoleNavigation>,
    route_ids: HashSet<String>,
    item_ids: HashSet<String>,
    binding_ids: HashSet<String>,
    paths: HashSet<String>,
}

impl From<ConsoleNavigation> for ConsoleSurfaceRegistryState {
    fn from(navigation: ConsoleNavigation) -> Self {
        let mut route_ids = HashSet::new();
        let mut paths = HashSet::new();
        for route in navigation.route_definitions {
            route_ids.insert(route.route_id);
            paths.insert(route.path);
        }

        Self {
            contributions: Vec::new(),
            route_ids,
            item_ids: navigation
                .navigation_items
                .into_iter()
                .map(|item| item.item_id)
                .collect(),
            binding_ids: navigation
                .permission_bindings
                .into_iter()
                .map(|binding| binding.binding_id)
                .collect(),
            paths,
        }
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
