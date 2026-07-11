use std::collections::HashSet;

use domain::ActorContext;
use serde::Serialize;

use crate::settings_route_specs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleSurfaceKind {
    System,
    DynamicPage,
    HostExtension,
}

impl ConsoleSurfaceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::DynamicPage => "dynamic_page",
            Self::HostExtension => "host_extension",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleNavigationSlot {
    Primary,
    Secondary,
    Settings,
}

impl ConsoleNavigationSlot {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Settings => "settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolePermissionRequirement {
    Authenticated,
    AnyPermission,
}

impl ConsolePermissionRequirement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authenticated => "authenticated",
            Self::AnyPermission => "any_permission",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleRouteDefinition {
    pub route_id: String,
    pub surface_key: String,
    pub path: String,
    pub surface_kind: ConsoleSurfaceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleNavigationItem {
    pub item_id: String,
    pub route_id: String,
    pub parent_item_id: Option<String>,
    pub label_key: String,
    pub navigation_slot: ConsoleNavigationSlot,
    pub order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsolePermissionBinding {
    pub binding_id: String,
    pub route_id: String,
    pub permission_codes: Vec<String>,
    pub requirement: ConsolePermissionRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleNavigation {
    pub route_definitions: Vec<ConsoleRouteDefinition>,
    pub navigation_items: Vec<ConsoleNavigationItem>,
    pub permission_bindings: Vec<ConsolePermissionBinding>,
}

#[derive(Clone, Copy)]
struct ConsoleRouteSpec {
    route_id: &'static str,
    surface_key: &'static str,
    path: &'static str,
    label_key: &'static str,
    navigation_slot: ConsoleNavigationSlot,
    parent_item_id: Option<&'static str>,
    order: i32,
    permission_codes: &'static [&'static str],
    requirement: ConsolePermissionRequirement,
}

const BUILTIN_CONSOLE_ROUTES: &[ConsoleRouteSpec] = &[
    ConsoleRouteSpec {
        route_id: "home",
        surface_key: "home",
        path: "/",
        label_key: "auto.workbench",
        navigation_slot: ConsoleNavigationSlot::Primary,
        parent_item_id: None,
        order: 100,
        permission_codes: &[],
        requirement: ConsolePermissionRequirement::Authenticated,
    },
    ConsoleRouteSpec {
        route_id: "embedded-apps",
        surface_key: "embedded-apps",
        path: "/embedded-apps",
        label_key: "auto.subsystem",
        navigation_slot: ConsoleNavigationSlot::Primary,
        parent_item_id: None,
        order: 300,
        permission_codes: &["embedded_app.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "templates",
        surface_key: "templates",
        path: "/templates",
        label_key: "auto.templates",
        navigation_slot: ConsoleNavigationSlot::Primary,
        parent_item_id: None,
        order: 400,
        permission_codes: &[],
        requirement: ConsolePermissionRequirement::Authenticated,
    },
    ConsoleRouteSpec {
        route_id: "settings",
        surface_key: "settings",
        path: "/settings",
        label_key: "auto.settings",
        navigation_slot: ConsoleNavigationSlot::Secondary,
        parent_item_id: None,
        order: 100,
        permission_codes: &[],
        requirement: ConsolePermissionRequirement::Authenticated,
    },
];

pub fn builtin_console_navigation() -> ConsoleNavigation {
    let mut route_definitions = BUILTIN_CONSOLE_ROUTES
        .iter()
        .map(|spec| route_definition(*spec))
        .collect::<Vec<_>>();
    let mut navigation_items = BUILTIN_CONSOLE_ROUTES
        .iter()
        .map(|spec| navigation_item(*spec))
        .collect::<Vec<_>>();
    let mut permission_bindings = BUILTIN_CONSOLE_ROUTES
        .iter()
        .map(|spec| permission_binding(*spec))
        .collect::<Vec<_>>();

    for spec in settings_route_specs() {
        route_definitions.push(ConsoleRouteDefinition {
            route_id: spec.route_id.to_string(),
            surface_key: spec.surface_key.to_string(),
            path: spec.path.to_string(),
            surface_kind: ConsoleSurfaceKind::System,
        });
        navigation_items.push(ConsoleNavigationItem {
            item_id: spec.route_id.to_string(),
            route_id: spec.route_id.to_string(),
            parent_item_id: Some("settings".to_string()),
            label_key: spec.label_key.to_string(),
            navigation_slot: ConsoleNavigationSlot::Settings,
            order: spec.order,
        });
        permission_bindings.push(ConsolePermissionBinding {
            binding_id: format!("{}.access", spec.route_id),
            route_id: spec.route_id.to_string(),
            permission_codes: vec![spec.visibility_permission_code.to_string()],
            requirement: ConsolePermissionRequirement::AnyPermission,
        });
    }

    ConsoleNavigation {
        route_definitions,
        navigation_items,
        permission_bindings,
    }
}

pub fn accessible_console_navigation(actor: &ActorContext) -> ConsoleNavigation {
    accessible_console_navigation_with_contributions(actor, &[])
}

pub fn accessible_console_navigation_with_contributions(
    actor: &ActorContext,
    contributions: &[ConsoleNavigation],
) -> ConsoleNavigation {
    let mut navigation = builtin_console_navigation();
    for contribution in contributions {
        navigation
            .route_definitions
            .extend(contribution.route_definitions.iter().cloned());
        navigation
            .navigation_items
            .extend(contribution.navigation_items.iter().cloned());
        navigation
            .permission_bindings
            .extend(contribution.permission_bindings.iter().cloned());
    }

    visible_console_navigation(actor, navigation)
}

fn visible_console_navigation(
    actor: &ActorContext,
    navigation: ConsoleNavigation,
) -> ConsoleNavigation {
    let visible_route_ids = navigation
        .permission_bindings
        .iter()
        .filter(|binding| is_binding_visible(actor, binding))
        .map(|binding| binding.route_id.clone())
        .collect::<HashSet<_>>();
    let visible_item_ids = navigation
        .navigation_items
        .iter()
        .filter(|item| visible_route_ids.contains(&item.route_id))
        .map(|item| item.item_id.clone())
        .collect::<HashSet<_>>();

    let mut navigation = ConsoleNavigation {
        route_definitions: navigation
            .route_definitions
            .into_iter()
            .filter(|route| visible_route_ids.contains(&route.route_id))
            .collect(),
        navigation_items: navigation
            .navigation_items
            .into_iter()
            .filter(|item| {
                visible_route_ids.contains(&item.route_id)
                    && match item.parent_item_id.as_ref() {
                        Some(parent_item_id) => visible_item_ids.contains(parent_item_id),
                        None => true,
                    }
            })
            .collect(),
        permission_bindings: navigation
            .permission_bindings
            .into_iter()
            .filter(|binding| visible_route_ids.contains(&binding.route_id))
            .collect(),
    };

    if !navigation
        .navigation_items
        .iter()
        .any(|item| item.parent_item_id.as_deref() == Some("settings"))
    {
        navigation
            .route_definitions
            .retain(|route| route.route_id != "settings");
        navigation
            .navigation_items
            .retain(|item| item.item_id != "settings");
        navigation
            .permission_bindings
            .retain(|binding| binding.route_id != "settings");
    }

    navigation
}

fn is_binding_visible(actor: &ActorContext, binding: &ConsolePermissionBinding) -> bool {
    match binding.requirement {
        ConsolePermissionRequirement::Authenticated => true,
        ConsolePermissionRequirement::AnyPermission => binding
            .permission_codes
            .iter()
            .any(|permission_code| actor.has_permission(permission_code)),
    }
}

fn route_definition(spec: ConsoleRouteSpec) -> ConsoleRouteDefinition {
    ConsoleRouteDefinition {
        route_id: spec.route_id.to_string(),
        surface_key: spec.surface_key.to_string(),
        path: spec.path.to_string(),
        surface_kind: ConsoleSurfaceKind::System,
    }
}

fn navigation_item(spec: ConsoleRouteSpec) -> ConsoleNavigationItem {
    ConsoleNavigationItem {
        item_id: spec.route_id.to_string(),
        route_id: spec.route_id.to_string(),
        parent_item_id: spec.parent_item_id.map(str::to_string),
        label_key: spec.label_key.to_string(),
        navigation_slot: spec.navigation_slot,
        order: spec.order,
    }
}

fn permission_binding(spec: ConsoleRouteSpec) -> ConsolePermissionBinding {
    ConsolePermissionBinding {
        binding_id: format!("{}.access", spec.route_id),
        route_id: spec.route_id.to_string(),
        permission_codes: spec
            .permission_codes
            .iter()
            .map(|permission_code| (*permission_code).to_string())
            .collect(),
        requirement: spec.requirement,
    }
}
