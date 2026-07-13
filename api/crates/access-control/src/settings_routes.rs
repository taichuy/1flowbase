use std::collections::BTreeSet;

use domain::PermissionDefinition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRouteApiMethods {
    Any,
    ReadOnly,
}

impl SettingsRouteApiMethods {
    fn matches(self, method: &str) -> bool {
        match self {
            Self::Any => true,
            Self::ReadOnly => matches!(method, "GET" | "HEAD" | "OPTIONS"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRouteApiPathMatch {
    Exact,
    Prefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsRouteApiScope {
    pub scope_id: &'static str,
    pub path: &'static str,
    pub path_match: SettingsRouteApiPathMatch,
    pub methods: SettingsRouteApiMethods,
}

impl SettingsRouteApiScope {
    fn matches(self, method: &str, path: &str) -> bool {
        self.methods.matches(method)
            && match self.path_match {
                SettingsRouteApiPathMatch::Exact => self.path == path,
                SettingsRouteApiPathMatch::Prefix => path.starts_with(self.path),
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRouteLegacyVisibility {
    Authenticated,
    AnyPermission(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsRouteSpec {
    pub route_id: &'static str,
    pub surface_key: &'static str,
    pub path: &'static str,
    pub label_key: &'static str,
    pub order: i32,
    pub visibility_permission_code: &'static str,
    pub legacy_visibility: SettingsRouteLegacyVisibility,
    pub implied_permissions: &'static [&'static str],
    pub api_scopes: &'static [SettingsRouteApiScope],
}

const SYSTEM_RUNTIME_PERMISSIONS: &[&str] = &["system_runtime.view.all"];
const API_REFERENCE_PERMISSIONS: &[&str] = &["api_reference.view.all"];
const STATE_MODEL_VISIBILITY_PERMISSIONS: &[&str] = &[
    "state_model.view.all",
    "state_model.view.own",
    "state_model.manage.all",
    "state_model.manage.own",
];
const DATA_MODEL_ALL_PERMISSIONS: &[&str] = &[
    "api_reference.view.all",
    "state_model.view.all",
    "state_model.view.own",
    "state_model.create.all",
    "state_model.edit.all",
    "state_model.edit.own",
    "state_model.delete.all",
    "state_model.delete.own",
    "state_model.manage.all",
    "state_model.manage.own",
    "external_data_source.view.all",
    "external_data_source.view.own",
    "external_data_source.create.all",
    "external_data_source.edit.all",
    "external_data_source.edit.own",
    "external_data_source.delete.all",
    "external_data_source.delete.own",
    "external_data_source.configure.all",
    "external_data_source.configure.own",
    "external_data_source.use.all",
    "external_data_source.use.own",
];
const MODEL_PROVIDER_ALL_PERMISSIONS: &[&str] = &[
    "state_model.view.all",
    "state_model.view.own",
    "state_model.create.all",
    "state_model.edit.all",
    "state_model.edit.own",
    "state_model.delete.all",
    "state_model.delete.own",
    "state_model.manage.all",
    "state_model.manage.own",
    "plugin_config.view.all",
    "plugin_config.configure.all",
];
const MCP_VISIBILITY_PERMISSIONS: &[&str] =
    &["mcp_management.view.all", "mcp_management.manage.all"];
const MCP_ALL_PERMISSIONS: &[&str] = &["mcp_management.view.all", "mcp_management.manage.all"];

const DOCS_API_SCOPES: &[SettingsRouteApiScope] = &[SettingsRouteApiScope {
    scope_id: "console.docs",
    path: "/api/console/docs/",
    path_match: SettingsRouteApiPathMatch::Prefix,
    methods: SettingsRouteApiMethods::ReadOnly,
}];

const API_KEY_AUTHENTICATION_API_SCOPES: &[SettingsRouteApiScope] = &[SettingsRouteApiScope {
    scope_id: "console.user_api_keys",
    path: "/api/console/user-api-keys",
    path_match: SettingsRouteApiPathMatch::Prefix,
    methods: SettingsRouteApiMethods::Any,
}];

const SYSTEM_RUNTIME_API_SCOPES: &[SettingsRouteApiScope] = &[SettingsRouteApiScope {
    scope_id: "console.system",
    path: "/api/console/system/",
    path_match: SettingsRouteApiPathMatch::Prefix,
    methods: SettingsRouteApiMethods::ReadOnly,
}];

const DATA_MODELS_API_SCOPES: &[SettingsRouteApiScope] = &[
    SettingsRouteApiScope {
        scope_id: "console.docs.data_models.openapi",
        path: "/api/console/docs/data-models/",
        path_match: SettingsRouteApiPathMatch::Prefix,
        methods: SettingsRouteApiMethods::ReadOnly,
    },
    SettingsRouteApiScope {
        scope_id: "console.data_sources.instances",
        path: "/api/console/data-sources/instances",
        path_match: SettingsRouteApiPathMatch::Prefix,
        methods: SettingsRouteApiMethods::Any,
    },
    SettingsRouteApiScope {
        scope_id: "console.models",
        path: "/api/console/models",
        path_match: SettingsRouteApiPathMatch::Prefix,
        methods: SettingsRouteApiMethods::Any,
    },
];

const MODEL_PROVIDERS_API_SCOPES: &[SettingsRouteApiScope] = &[
    SettingsRouteApiScope {
        scope_id: "console.model_providers",
        path: "/api/console/model-providers",
        path_match: SettingsRouteApiPathMatch::Prefix,
        methods: SettingsRouteApiMethods::Any,
    },
    SettingsRouteApiScope {
        scope_id: "console.plugins",
        path: "/api/console/plugins",
        path_match: SettingsRouteApiPathMatch::Prefix,
        methods: SettingsRouteApiMethods::Any,
    },
];

const MCP_MANAGEMENT_API_SCOPES: &[SettingsRouteApiScope] = &[SettingsRouteApiScope {
    scope_id: "console.mcp",
    path: "/api/console/mcp/",
    path_match: SettingsRouteApiPathMatch::Prefix,
    methods: SettingsRouteApiMethods::Any,
}];

const SETTINGS_ROUTE_SPECS: &[SettingsRouteSpec] = &[
    SettingsRouteSpec {
        route_id: "settings.docs",
        surface_key: "docs",
        path: "/settings/docs",
        label_key: "auto.api_documentation",
        order: 100,
        visibility_permission_code: "settings_route.visible.settings.docs",
        legacy_visibility: SettingsRouteLegacyVisibility::AnyPermission(API_REFERENCE_PERMISSIONS),
        implied_permissions: API_REFERENCE_PERMISSIONS,
        api_scopes: DOCS_API_SCOPES,
    },
    SettingsRouteSpec {
        route_id: "settings.api-key-authentication",
        surface_key: "api-key-authentication",
        path: "/settings/api-key-authentication",
        label_key: "auto.api_key_authentication",
        order: 200,
        visibility_permission_code: "settings_route.visible.settings.api-key-authentication",
        legacy_visibility: SettingsRouteLegacyVisibility::Authenticated,
        implied_permissions: &[],
        api_scopes: API_KEY_AUTHENTICATION_API_SCOPES,
    },
    SettingsRouteSpec {
        route_id: "settings.system-runtime",
        surface_key: "system-runtime",
        path: "/settings/system-runtime",
        label_key: "auto.system_runtime",
        order: 400,
        visibility_permission_code: "settings_route.visible.settings.system-runtime",
        legacy_visibility: SettingsRouteLegacyVisibility::AnyPermission(SYSTEM_RUNTIME_PERMISSIONS),
        implied_permissions: SYSTEM_RUNTIME_PERMISSIONS,
        api_scopes: SYSTEM_RUNTIME_API_SCOPES,
    },
    SettingsRouteSpec {
        route_id: "settings.data-models",
        surface_key: "data-models",
        path: "/settings/data-models",
        label_key: "auto.data_source",
        order: 900,
        visibility_permission_code: "settings_route.visible.settings.data-models",
        legacy_visibility: SettingsRouteLegacyVisibility::AnyPermission(
            STATE_MODEL_VISIBILITY_PERMISSIONS,
        ),
        implied_permissions: DATA_MODEL_ALL_PERMISSIONS,
        api_scopes: DATA_MODELS_API_SCOPES,
    },
    SettingsRouteSpec {
        route_id: "settings.model-providers",
        surface_key: "model-providers",
        path: "/settings/model-providers",
        label_key: "auto.model_providers",
        order: 1000,
        visibility_permission_code: "settings_route.visible.settings.model-providers",
        legacy_visibility: SettingsRouteLegacyVisibility::AnyPermission(
            STATE_MODEL_VISIBILITY_PERMISSIONS,
        ),
        implied_permissions: MODEL_PROVIDER_ALL_PERMISSIONS,
        api_scopes: MODEL_PROVIDERS_API_SCOPES,
    },
    SettingsRouteSpec {
        route_id: "settings.mcp-management",
        surface_key: "mcp-management",
        path: "/settings/mcp-management",
        label_key: "auto.mcp_management",
        order: 1100,
        visibility_permission_code: "settings_route.visible.settings.mcp-management",
        legacy_visibility: SettingsRouteLegacyVisibility::AnyPermission(MCP_VISIBILITY_PERMISSIONS),
        implied_permissions: MCP_ALL_PERMISSIONS,
        api_scopes: MCP_MANAGEMENT_API_SCOPES,
    },
];

pub fn settings_route_specs() -> &'static [SettingsRouteSpec] {
    SETTINGS_ROUTE_SPECS
}

pub fn settings_route_spec_by_visibility_permission(
    permission_code: &str,
) -> Option<&'static SettingsRouteSpec> {
    SETTINGS_ROUTE_SPECS
        .iter()
        .find(|spec| spec.visibility_permission_code == permission_code)
}

pub fn settings_route_permission_definitions() -> Vec<PermissionDefinition> {
    SETTINGS_ROUTE_SPECS
        .iter()
        .map(|spec| PermissionDefinition {
            code: spec.visibility_permission_code.to_string(),
            resource: "settings_route".to_string(),
            action: "visible".to_string(),
            scope: spec.route_id.to_string(),
            name: format!("settings_route:visible:{}", spec.route_id),
        })
        .collect()
}

pub fn expand_permissions_with_settings_routes(permission_codes: &[String]) -> Vec<String> {
    let mut expanded = permission_codes.iter().cloned().collect::<BTreeSet<_>>();

    for spec in SETTINGS_ROUTE_SPECS {
        if expanded.contains(spec.visibility_permission_code) {
            for permission_code in spec.implied_permissions {
                expanded.insert((*permission_code).to_string());
            }
        }
    }

    expanded.into_iter().collect()
}

pub fn settings_route_permissions_for_console_request(
    method: &str,
    path: &str,
) -> Vec<&'static str> {
    SETTINGS_ROUTE_SPECS
        .iter()
        .filter(|spec| {
            spec.api_scopes
                .iter()
                .copied()
                .any(|scope| scope.matches(method, path))
        })
        .map(|spec| spec.visibility_permission_code)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
