use std::collections::HashSet;

use domain::ActorContext;
use serde::Serialize;

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

const STATE_MODEL_PERMISSIONS: &[&str] = &[
    "state_model.view.all",
    "state_model.view.own",
    "state_model.manage.all",
    "state_model.manage.own",
];

const SYSTEM_CONSOLE_ROUTES: &[ConsoleRouteSpec] = &[
    ConsoleRouteSpec {
        route_id: "home",
        surface_key: "home",
        path: "/",
        label_key: "auto.workbench",
        navigation_slot: ConsoleNavigationSlot::Primary,
        parent_item_id: None,
        order: 100,
        permission_codes: &["route_page.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "frontstage",
        surface_key: "frontstage",
        path: "/frontstage",
        label_key: "auto.frontstage",
        navigation_slot: ConsoleNavigationSlot::Primary,
        parent_item_id: None,
        order: 200,
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
        permission_codes: &["route_page.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
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
    ConsoleRouteSpec {
        route_id: "docs",
        surface_key: "docs",
        path: "/settings/docs",
        label_key: "auto.api_documentation",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 100,
        permission_codes: &["api_reference.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "api-key-authentication",
        surface_key: "api-key-authentication",
        path: "/settings/api-key-authentication",
        label_key: "auto.api_key_authentication",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 200,
        permission_codes: &[],
        requirement: ConsolePermissionRequirement::Authenticated,
    },
    ConsoleRouteSpec {
        route_id: "auth-center",
        surface_key: "auth-center",
        path: "/settings/auth-center",
        label_key: "auto.auth_center",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 300,
        permission_codes: &["user.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "system-runtime",
        surface_key: "system-runtime",
        path: "/settings/system-runtime",
        label_key: "auto.system_runtime",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 400,
        permission_codes: &["system_runtime.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "host-infrastructure",
        surface_key: "host-infrastructure",
        path: "/settings/host-infrastructure",
        label_key: "auto.infrastructure",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 500,
        permission_codes: &["plugin_config.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "memory-observation",
        surface_key: "memory-observation",
        path: "/settings/memory-observation",
        label_key: "auto.memory_observation",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 600,
        permission_codes: &["plugin_config.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "files",
        surface_key: "files",
        path: "/settings/files",
        label_key: "auto.file_management",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 700,
        permission_codes: &[
            "file_table.view.all",
            "file_table.view.own",
            "file_table.create.all",
        ],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "data-models",
        surface_key: "data-models",
        path: "/settings/data-models",
        label_key: "auto.data_source",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 800,
        permission_codes: STATE_MODEL_PERMISSIONS,
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "model-providers",
        surface_key: "model-providers",
        path: "/settings/model-providers",
        label_key: "auto.model_providers",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 900,
        permission_codes: STATE_MODEL_PERMISSIONS,
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "mcp-management",
        surface_key: "mcp-management",
        path: "/settings/mcp-management",
        label_key: "auto.mcp_management",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 1000,
        permission_codes: &["mcp_management.view.all", "mcp_management.manage.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "members",
        surface_key: "members",
        path: "/settings/members",
        label_key: "auto.user_management",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 1100,
        permission_codes: &["user.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
    ConsoleRouteSpec {
        route_id: "roles",
        surface_key: "roles",
        path: "/settings/roles",
        label_key: "auto.permission_management",
        navigation_slot: ConsoleNavigationSlot::Settings,
        parent_item_id: Some("settings"),
        order: 1200,
        permission_codes: &["role_permission.view.all"],
        requirement: ConsolePermissionRequirement::AnyPermission,
    },
];

pub fn builtin_console_navigation() -> ConsoleNavigation {
    ConsoleNavigation {
        route_definitions: SYSTEM_CONSOLE_ROUTES
            .iter()
            .map(|spec| route_definition(*spec))
            .collect(),
        navigation_items: SYSTEM_CONSOLE_ROUTES
            .iter()
            .map(|spec| navigation_item(*spec))
            .collect(),
        permission_bindings: SYSTEM_CONSOLE_ROUTES
            .iter()
            .map(|spec| permission_binding(*spec))
            .collect(),
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

    ConsoleNavigation {
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
    }
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
