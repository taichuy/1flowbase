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
pub const SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID: &str = "system.data-models";
pub const SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.data-models";
pub const SYSTEM_DOCS_SETTINGS_FEATURE_ID: &str = "system.docs";
pub const SYSTEM_DOCS_SETTINGS_FEATURE_PERMISSION: &str = "settings_feature.access.system.docs";
pub const SYSTEM_FILES_SETTINGS_FEATURE_ID: &str = "system.files";
pub const SYSTEM_FILES_SETTINGS_FEATURE_PERMISSION: &str = "settings_feature.access.system.files";
pub const SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_ID: &str = "system.host-infrastructure";
pub const SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.host-infrastructure";
pub const SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_ID: &str = "system.memory-observation";
pub const SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.memory-observation";
pub const SYSTEM_MEMBERS_SETTINGS_FEATURE_ID: &str = "system.members";
pub const SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.members";
pub const SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_ID: &str = "system.mcp-management";
pub const SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.mcp-management";
pub const SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_ID: &str = "system.model-providers";
pub const SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.model-providers";
pub const SYSTEM_ROLES_SETTINGS_FEATURE_ID: &str = "system.roles";
pub const SYSTEM_ROLES_SETTINGS_FEATURE_PERMISSION: &str = "settings_feature.access.system.roles";
pub const SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_ID: &str = "system.system-runtime";
pub const SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_PERMISSION: &str =
    "settings_feature.access.system.system-runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsFeatureOwnerKind {
    Core,
    HostExtension,
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

pub fn core_settings_feature_registrations() -> Vec<SettingsFeatureRegistration> {
    vec![
        SettingsFeatureRegistration {
            feature_id: SYSTEM_DOCS_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.docs".to_string(),
                surface_key: "docs".to_string(),
                path: "/settings/docs".to_string(),
                label_key: "auto.api_documentation".to_string(),
                order: 100,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/docs/catalog"),
                (
                    "GET",
                    "/api/console/docs/categories/{category_id}/operations",
                ),
                (
                    "GET",
                    "/api/console/docs/categories/{category_id}/openapi.json",
                ),
                (
                    "GET",
                    "/api/console/docs/operations/{operation_id}/openapi.json",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_API_KEY_AUTHENTICATION_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.api-key-authentication".to_string(),
                surface_key: "api-key-authentication".to_string(),
                path: "/settings/api-key-authentication".to_string(),
                label_key: "auto.api_key_authentication".to_string(),
                order: 200,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/user-api-keys"),
                ("POST", "/api/console/user-api-keys"),
                ("GET", "/api/console/user-api-keys/role-options"),
                (
                    "POST",
                    "/api/console/user-api-keys/{api_key_id}/revoke",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_SYSTEM_RUNTIME_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.system-runtime".to_string(),
                surface_key: "system-runtime".to_string(),
                path: "/settings/system-runtime".to_string(),
                label_key: "auto.system_runtime".to_string(),
                order: 400,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/system/runtime-profile"),
                ("GET", "/api/console/system/release-status"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.applications".to_string(),
                surface_key: "applications".to_string(),
                path: "/settings/applications".to_string(),
                label_key: "auto.application_management".to_string(),
                order: 700,
            },
            api_routes: settings_api_routes(&[(
                "GET",
                "/api/console/settings/applications",
            )]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.auth-center".to_string(),
                surface_key: "auth-center".to_string(),
                path: "/settings/auth-center".to_string(),
                label_key: "auto.auth_center".to_string(),
                order: 300,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/auth-center/overview"),
                (
                    "POST",
                    "/api/console/settings/auth-center/authenticators",
                ),
                (
                    "PUT",
                    "/api/console/settings/auth-center/authenticators/order",
                ),
                (
                    "POST",
                    "/api/console/settings/auth-center/authenticators/{id}/actions/enable",
                ),
                (
                    "POST",
                    "/api/console/settings/auth-center/authenticators/{id}/copy",
                ),
                (
                    "PUT",
                    "/api/console/settings/auth-center/authenticators/{id}/config",
                ),
                (
                    "DELETE",
                    "/api/console/settings/auth-center/authenticators/{id}",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_DATA_MODELS_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.data-models".to_string(),
                surface_key: "data-models".to_string(),
                path: "/settings/data-models".to_string(),
                label_key: "auto.data_source".to_string(),
                order: 900,
            },
            api_routes: settings_api_routes(&[
                (
                    "GET",
                    "/api/console/settings/data-models/data-sources/catalog",
                ),
                (
                    "GET",
                    "/api/console/settings/data-models/data-sources",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/data-sources",
                ),
                (
                    "PATCH",
                    "/api/console/settings/data-models/data-sources/{data_source_id}/defaults",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/data-sources/{data_source_id}/validate",
                ),
                (
                    "GET",
                    "/api/console/settings/data-models/data-sources/{data_source_id}/resources",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/data-sources/{data_source_id}/resources/discover",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/data-sources/{data_source_id}/preview-read",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/data-sources/{data_source_id}/resources/map-to-model",
                ),
                (
                    "GET",
                    "/api/console/settings/data-models/model-definitions",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/model-definitions",
                ),
                (
                    "PATCH",
                    "/api/console/settings/data-models/model-definitions/{id}",
                ),
                (
                    "DELETE",
                    "/api/console/settings/data-models/model-definitions/{id}",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/model-definitions:batchDelete",
                ),
                (
                    "GET",
                    "/api/console/settings/data-models/model-definitions/{id}/advisor-findings",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/model-definitions/{id}/fields",
                ),
                (
                    "PATCH",
                    "/api/console/settings/data-models/model-definitions/{id}/fields/{field_id}",
                ),
                (
                    "DELETE",
                    "/api/console/settings/data-models/model-definitions/{id}/fields/{field_id}",
                ),
                (
                    "GET",
                    "/api/console/settings/data-models/model-definitions/{id}/scope-grants",
                ),
                (
                    "POST",
                    "/api/console/settings/data-models/model-definitions/{id}/scope-grants",
                ),
                (
                    "PATCH",
                    "/api/console/settings/data-models/model-definitions/{id}/scope-grants/{grant_id}",
                ),
                (
                    "GET",
                    "/api/console/settings/data-models/model-definitions/{model_id}/openapi.json",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_FILES_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.files".to_string(),
                surface_key: "files".to_string(),
                path: "/settings/files".to_string(),
                label_key: "auto.file_management".to_string(),
                order: 800,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/files/storages"),
                ("POST", "/api/console/settings/files/storages"),
                ("PUT", "/api/console/settings/files/storages/{id}"),
                ("DELETE", "/api/console/settings/files/storages/{id}"),
                ("GET", "/api/console/settings/files/tables"),
                ("POST", "/api/console/settings/files/tables"),
                (
                    "PUT",
                    "/api/console/settings/files/tables/{id}/binding",
                ),
                ("DELETE", "/api/console/settings/files/tables/{id}"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_HOST_INFRASTRUCTURE_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.host-infrastructure".to_string(),
                surface_key: "host-infrastructure".to_string(),
                path: "/settings/host-infrastructure".to_string(),
                label_key: "auto.infrastructure".to_string(),
                order: 500,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/host-infrastructure/cache"),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/entries",
                ),
                (
                    "POST",
                    "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/entries/reveal",
                ),
                (
                    "POST",
                    "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/entries/clear",
                ),
                (
                    "POST",
                    "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/clear",
                ),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/providers",
                ),
                (
                    "PUT",
                    "/api/console/settings/host-infrastructure/providers/{installation_id}/{provider_code}/config",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_MEMORY_OBSERVATION_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.memory-observation".to_string(),
                surface_key: "memory-observation".to_string(),
                path: "/settings/memory-observation".to_string(),
                label_key: "auto.memory_observation".to_string(),
                order: 600,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/host-infrastructure/memory"),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/memory/stats",
                ),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/entries",
                ),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/stats",
                ),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/entries/search",
                ),
                (
                    "GET",
                    "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/tree",
                ),
                (
                    "POST",
                    "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/entries/reveal",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_MEMBERS_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.members".to_string(),
                surface_key: "members".to_string(),
                path: "/settings/members".to_string(),
                label_key: "auto.user_management".to_string(),
                order: 1200,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/members"),
                ("POST", "/api/console/settings/members"),
                ("GET", "/api/console/settings/members/role-options"),
                ("PATCH", "/api/console/settings/members/{id}"),
                ("DELETE", "/api/console/settings/members/{id}"),
                ("POST", "/api/console/settings/members/{id}/disable"),
                ("POST", "/api/console/settings/members/{id}/enable"),
                ("POST", "/api/console/settings/members/{id}/reset-password"),
                ("PUT", "/api/console/settings/members/{id}/roles"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_MODEL_PROVIDERS_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.model-providers".to_string(),
                surface_key: "model-providers".to_string(),
                path: "/settings/model-providers".to_string(),
                label_key: "auto.model_providers".to_string(),
                order: 1000,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/model-providers/catalog"),
                ("GET", "/api/console/settings/model-providers/instances"),
                ("POST", "/api/console/settings/model-providers/instances"),
                (
                    "PATCH",
                    "/api/console/settings/model-providers/instances/{id}",
                ),
                (
                    "DELETE",
                    "/api/console/settings/model-providers/instances/{id}",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/instances/{id}/validate",
                ),
                (
                    "GET",
                    "/api/console/settings/model-providers/instances/{id}/models",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/instances/{id}/models/refresh",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/instances/{id}/secrets/reveal",
                ),
                (
                    "GET",
                    "/api/console/settings/model-providers/providers/{provider_code}/main-instance",
                ),
                (
                    "PUT",
                    "/api/console/settings/model-providers/providers/{provider_code}/main-instance",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/preview-models",
                ),
                ("GET", "/api/console/settings/model-providers/options"),
                (
                    "GET",
                    "/api/console/settings/model-providers/request-logs",
                ),
                (
                    "DELETE",
                    "/api/console/settings/model-providers/request-logs",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/request-logs/clear",
                ),
                (
                    "GET",
                    "/api/console/settings/model-providers/plugins/families",
                ),
                (
                    "GET",
                    "/api/console/settings/model-providers/plugins/official-catalog",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/plugins/install-official",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/plugins/install-upload",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/plugins/{installation_id}/artifact/refresh",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/plugins/{installation_id}/artifact/install-current-node",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/plugins/families/{provider_code}/upgrade-latest",
                ),
                (
                    "POST",
                    "/api/console/settings/model-providers/plugins/families/{provider_code}/switch-version",
                ),
                (
                    "DELETE",
                    "/api/console/settings/model-providers/plugins/families/{provider_code}",
                ),
                (
                    "GET",
                    "/api/console/settings/model-providers/plugins/tasks/{task_id}",
                ),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.mcp-management".to_string(),
                surface_key: "mcp-management".to_string(),
                path: "/settings/mcp-management".to_string(),
                label_key: "auto.mcp_management".to_string(),
                order: 1100,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/mcp/catalog"),
                ("GET", "/api/console/mcp/interface-capabilities"),
                ("GET", "/api/console/mcp/list"),
                ("GET", "/api/console/mcp/export"),
                ("GET", "/api/console/mcp/instances"),
                ("POST", "/api/console/mcp/instances"),
                ("GET", "/api/console/mcp/instances/export"),
                ("PUT", "/api/console/mcp/instances/{instance_id}"),
                ("DELETE", "/api/console/mcp/instances/{instance_id}"),
                (
                    "GET",
                    "/api/console/mcp/instances/{instance_id}/client-credential",
                ),
                (
                    "PUT",
                    "/api/console/mcp/instances/{instance_id}/client-credential",
                ),
                (
                    "DELETE",
                    "/api/console/mcp/instances/{instance_id}/client-credential",
                ),
                (
                    "POST",
                    "/api/console/mcp/instances/{instance_id}/groups",
                ),
                (
                    "DELETE",
                    "/api/console/mcp/instances/{instance_id}/groups",
                ),
                (
                    "POST",
                    "/api/console/mcp/instances/{instance_id}/groups/move",
                ),
                (
                    "POST",
                    "/api/console/mcp/instances/{instance_id}/tool-bindings",
                ),
                ("PUT", "/api/console/mcp/tool-bindings/{binding_id}"),
                ("DELETE", "/api/console/mcp/tool-bindings/{binding_id}"),
                ("GET", "/api/console/mcp/tools"),
                ("POST", "/api/console/mcp/tools"),
                ("GET", "/api/console/mcp/tools/{tool_id}"),
                ("PUT", "/api/console/mcp/tools/{tool_id}"),
                ("DELETE", "/api/console/mcp/tools/{tool_id}"),
                (
                    "POST",
                    "/api/console/mcp/tools/{tool_id}/description/refresh",
                ),
                (
                    "POST",
                    "/api/console/mcp/tools/{tool_id}/description-check",
                ),
                ("POST", "/api/console/mcp/debug/execute"),
                (
                    "GET",
                    "/api/console/mcp/instances/{instance_id}/discovery-policy",
                ),
                (
                    "PUT",
                    "/api/console/mcp/instances/{instance_id}/discovery-policy",
                ),
                ("GET", "/api/console/mcp/bundles/official"),
                ("POST", "/api/console/mcp/bundles/preview-official"),
                ("POST", "/api/console/mcp/bundles/import-official"),
                ("POST", "/api/console/mcp/bundles/export"),
                ("POST", "/api/console/mcp/bundles/preview-upload"),
                ("POST", "/api/console/mcp/bundles/import-upload"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_ROLES_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.roles".to_string(),
                surface_key: "roles".to_string(),
                path: "/settings/roles".to_string(),
                label_key: "auto.permission_management".to_string(),
                order: 1300,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/roles"),
                ("POST", "/api/console/settings/roles"),
                ("GET", "/api/console/settings/roles/permission-options"),
                ("GET", "/api/console/settings/roles/data-model-options"),
                ("PATCH", "/api/console/settings/roles/{id}"),
                ("DELETE", "/api/console/settings/roles/{id}"),
                ("GET", "/api/console/settings/roles/{id}/permissions"),
                ("PUT", "/api/console/settings/roles/{id}/permissions"),
                ("GET", "/api/console/settings/roles/{id}/frontstage-routes"),
                ("PUT", "/api/console/settings/roles/{id}/frontstage-routes"),
                ("GET", "/api/console/settings/roles/{id}/data-policy"),
                ("PUT", "/api/console/settings/roles/{id}/data-policy"),
            ]),
        },
    ]
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
    if !registration.console_surface.path.starts_with("/settings/") {
        return Err(SettingsFeatureRegistryError::new(
            "settings feature console path must start with /settings/",
        ));
    }
    if registration.api_routes.is_empty() {
        return Err(SettingsFeatureRegistryError::new(format!(
            "settings feature {} must own at least one API route",
            registration.feature_id
        )));
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
