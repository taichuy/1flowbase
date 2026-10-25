use super::*;

pub fn core_settings_feature_registrations() -> Vec<SettingsFeatureRegistration> {
    vec![
        SettingsFeatureRegistration {
            feature_id: "system.branding".to_owned(),
            owner: SettingsFeatureOwner { kind: SettingsFeatureOwnerKind::Core, owner_id: "boot-core".to_owned(), version: env!("CARGO_PKG_VERSION").to_owned() },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface { route_id: "settings.system".to_owned(), surface_key: "system".to_owned(), path: "/settings/system".to_owned(), label_key: "system_settings.title".to_owned(), description_key: "console.policy_groups.settings.system.branding.description".to_owned(), order: 2000 },
            api_routes: settings_api_routes(&[("GET", "/api/console/settings/system"), ("PUT", "/api/console/settings/system")]),
        },
        SettingsFeatureRegistration {
            feature_id:SYSTEM_NOTIFICATIONS_SETTINGS_FEATURE_ID.to_owned(),
            owner:SettingsFeatureOwner{kind:SettingsFeatureOwnerKind::Core,owner_id:"boot-core".to_owned(),version:env!("CARGO_PKG_VERSION").to_owned()},
            lifecycle:SettingsFeatureLifecycle::Active,
            console_surface:SettingsFeatureConsoleSurface{route_id:"settings.communications".to_owned(),surface_key:"communications".to_owned(),path:"/settings/communications".to_owned(),label_key:"auto.communications".to_owned(),description_key:"console.policy_groups.settings.system.notifications.description".to_owned(),order:510},
            api_routes:settings_api_routes(&[
                ("GET","/api/console/communications/options"),
                ("GET","/api/console/communications/instances/{instance_id}/binding"),
                ("PUT","/api/console/communications/instances/{instance_id}/binding"),
                ("GET","/api/console/communications/instances/{instance_id}/conversations"),
                ("GET","/api/console/communications/instances/{instance_id}/conversations/{conversation_id}/messages"),
                ("GET","/api/console/communications/instances/{instance_id}/conversations/{conversation_id}/files"),
                ("GET","/api/console/communications/instances/{instance_id}/files/{attachment_id}"),
                ("POST","/api/console/communications/instances/{instance_id}/messages/{message_id}/retry"),

                ("GET","/api/console/communications/instances"),("POST","/api/console/communications/instances"),
                ("GET","/api/console/communications/instances/{instance_id}"),("PATCH","/api/console/communications/instances/{instance_id}"),
                ("DELETE","/api/console/communications/instances/{instance_id}"),("POST","/api/console/communications/instances/{instance_id}/send"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_BACKUPS_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.backups".to_string(),
                surface_key: "backups".to_string(),
                path: "/settings/backups".to_string(),
                label_key: "auto.backups".to_string(),
                description_key: "console.policy_groups.settings.system.backups.description"
                    .to_string(),
                order: 575,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/system-backups"),
                ("GET", "/api/console/settings/system-backups/catalog"),
                ("GET", "/api/console/settings/system-templates/catalog"),
                ("POST", "/api/console/settings/system-templates/export"),
                ("POST", "/api/console/settings/system-templates/preview"),
                ("POST", "/api/console/settings/system-templates/install"),

                ("POST", "/api/console/settings/system-backups"),
                ("POST", "/api/console/settings/system-backups/import"),
                ("POST", "/api/console/settings/system-backups/recovery/reauth"),
                ("GET", "/api/console/settings/system-backups/recovery/status"),
                ("GET", "/api/console/settings/system-backups/{backup_set_id}"),
                ("DELETE", "/api/console/settings/system-backups/{backup_set_id}"),
                ("POST", "/api/console/settings/system-backups/{backup_set_id}/verify"),
                ("GET", "/api/console/settings/system-backups/{backup_set_id}/download"),
                ("POST", "/api/console/settings/system-backups/{backup_set_id}/recovery/preflight"),
                ("POST", "/api/console/settings/system-backups/{backup_set_id}/recovery/intents"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_UI_MANAGEMENT_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.ui-management".to_string(),
                surface_key: "ui-management".to_string(),
                path: "/settings/ui-management".to_string(),
                label_key: "auto.ui_management".to_string(),
                description_key: "console.policy_groups.settings.system.ui-management.description".to_string(),
                order: 550,
            },
            api_routes: settings_api_routes(&[
                ("POST", "/api/console/frontstage/pages/groups"),
                ("POST", "/api/console/frontstage/pages"),
                ("DELETE", "/api/console/frontstage/pages/{page_id}"),
                ("POST", "/api/console/frontstage/pages/{page_id}/move"),
                ("PATCH", "/api/console/frontstage/pages/{page_id}"),
                ("POST", "/api/console/frontstage/pages/{page_id}/tabs"),
                ("DELETE", "/api/console/frontstage/pages/{page_id}/tabs/{tab_reference}"),
                ("PUT", "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/document"),
                ("PATCH", "/api/console/frontstage/pages/{page_id}/tabs/{tab_reference}"),
                ("GET", "/api/console/settings/ui-management/templates"),
                ("POST", "/api/console/settings/ui-management/templates"),
                ("PUT", "/api/console/settings/ui-management/templates/{id}"),
                ("POST", "/api/console/settings/ui-management/templates/{id}/publish"),
                ("PUT", "/api/console/settings/ui-management/templates/{id}/default"),
                ("DELETE", "/api/console/settings/ui-management/templates/default"),
                ("DELETE", "/api/console/settings/ui-management/templates/{id}"),
                ("GET", "/api/console/settings/ui-management/components"),
                ("POST", "/api/console/settings/ui-management/components"),
                ("GET", "/api/console/settings/ui-management/components/{id}"),
                ("PUT", "/api/console/settings/ui-management/components/{id}"),
                ("DELETE", "/api/console/settings/ui-management/components/{id}"),
                ("GET", "/api/console/settings/ui-management/components/catalog/index"),
                ("GET", "/api/console/settings/ui-management/components/catalog/pages/{page}"),
                ("GET", "/api/console/settings/ui-management/components/catalog/search"),
                ("GET", "/api/console/settings/ui-management/components/catalog/update-status"),
                ("POST", "/api/console/settings/ui-management/components/catalog/{component_code}/download"),
                ("POST", "/api/console/settings/ui-management/components/catalog/groups/{source}/{group}/sync"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_EXTENSION_CENTER_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.extension-center".to_string(),
                surface_key: "extension-center".to_string(),
                path: "/settings/extension-center".to_string(),
                label_key: "auto.extension_center".to_string(),
                description_key: "console.policy_groups.settings.system.extension-center.description"
                    .to_string(),
                order: 100,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/extension-center/installed"),
                (
                    "GET",
                    "/api/console/settings/extension-center/catalog/{category}",
                ),
                (
                    "GET",
                    "/api/console/settings/extension-center/catalog/{category}/{catalog_id}",
                ),
                (
                    "POST",
                    "/api/console/settings/extension-center/update-check",
                ),
                ("POST", "/api/console/settings/extension-center/install"),
                (
                    "POST",
                    "/api/console/settings/extension-center/install-upload",
                ),
                ("POST", "/api/console/settings/extension-center/update"),
            ]),
        },
        SettingsFeatureRegistration {
            feature_id: SYSTEM_I18N_CATALOG_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.i18n".to_string(),
                surface_key: "i18n".to_string(),
                path: "/settings/i18n".to_string(),
                label_key: "auto.translation_catalog_title".to_string(),
                description_key: "auto.translation_catalog_description".to_string(),
                order: 1500,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/i18n/catalog"),
                ("GET", "/api/console/settings/i18n/update-check"),
                ("POST", "/api/console/settings/i18n/activate"),
                ("GET", "/api/console/settings/i18n/entries"),
                ("GET", "/api/console/settings/i18n/entries/detail"),
                ("PUT", "/api/console/settings/i18n/overrides"),
                ("DELETE", "/api/console/settings/i18n/overrides"),
                (
                    "PUT",
                    "/api/console/settings/i18n/custom-translations",
                ),
                ("DELETE", "/api/console/settings/i18n/custom-keys"),
                ("POST", "/api/console/settings/i18n/restore-overrides"),
            ]),
        },
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
                description_key: "console.policy_groups.settings.system.docs.description"
                    .to_string(),
                order: 700,
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
                description_key:
                    "console.policy_groups.settings.system.api-key-authentication.description"
                        .to_string(),
                order: 800,
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
                description_key: "console.policy_groups.settings.system.system-runtime.description"
                    .to_string(),
                order: 1200,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/system/runtime-profile"),
                ("GET", "/api/console/system/runtime-processes"),
                ("GET", "/api/console/system/release-status"),
                (
                    "POST",
                    "/api/console/system/runtime-profile/processes/{pid}/terminate",
                ),
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
                description_key: "console.policy_groups.settings.system.applications.description"
                    .to_string(),
                order: 300,
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
                description_key: "console.policy_groups.settings.system.auth-center.description"
                    .to_string(),
                order: 1000,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/auth-center/overview"),
                (
                    "POST",
                    "/api/console/settings/auth-center/login-entries",
                ),
                (
                    "PUT",
                    "/api/console/settings/auth-center/login-entries/order",
                ),
                (
                    "PUT",
                    "/api/console/settings/auth-center/login-entries/{id}/enabled",
                ),
                (
                    "POST",
                    "/api/console/settings/auth-center/login-entries/{id}/copy",
                ),
                (
                    "PUT",
                    "/api/console/settings/auth-center/login-entries/{id}/config",
                ),
                (
                    "DELETE",
                    "/api/console/settings/auth-center/login-entries/{id}",
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
                description_key: "console.policy_groups.settings.system.data-models.description"
                    .to_string(),
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
                description_key: "console.policy_groups.settings.system.files.description"
                    .to_string(),
                order: 1100,
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
                description_key:
                    "console.policy_groups.settings.system.memory-observation.description"
                        .to_string(),
                order: 1300,
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
                description_key: "console.policy_groups.settings.system.members.description"
                    .to_string(),
                order: 500,
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
            feature_id: SYSTEM_NETWORK_CENTER_SETTINGS_FEATURE_ID.to_string(),
            owner: SettingsFeatureOwner {
                kind: SettingsFeatureOwnerKind::Core,
                owner_id: "boot-core".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            lifecycle: SettingsFeatureLifecycle::Active,
            console_surface: SettingsFeatureConsoleSurface {
                route_id: "settings.network-center".to_string(),
                surface_key: "network-center".to_string(),
                path: "/settings/network-center/proxy-types".to_string(),
                label_key: "auto.network_center".to_string(),
                description_key:
                    "console.policy_groups.settings.system.network-center.description".to_string(),
                order: 250,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/settings/network-center/providers"),
                ("GET", "/api/console/settings/network-center/providers/types"),
                (
                    "GET",
                    "/api/console/settings/network-center/proxy-plugins/official-catalog",
                ),
                ("POST", "/api/console/settings/network-center/providers"),
                (
                    "POST",
                    "/api/console/settings/network-center/proxy-plugins/install-official",
                ),
                (
                    "POST",
                    "/api/console/settings/network-center/proxy-plugins/install-upload",
                ),
                ("PATCH", "/api/console/settings/network-center/providers/{id}"),
                ("POST", "/api/console/settings/network-center/providers/{id}/sync"),
                ("GET", "/api/console/network-center/pools"),
                ("POST", "/api/console/network-center/pools/proxies"),
                ("POST", "/api/console/network-center/pools"),
                ("PATCH", "/api/console/network-center/pools/{pool_id}"),
                ("DELETE", "/api/console/network-center/pools/{pool_id}"),
                ("POST", "/api/console/network-center/pools/{pool_id}/members"),
                ("PATCH", "/api/console/network-center/pools/{pool_id}/members/{member_id}"),
                ("DELETE", "/api/console/network-center/pools/{pool_id}/members/{member_id}"),
                ("GET", "/api/console/network-center/routes"),
                ("POST", "/api/console/network-center/routes"),
                ("PATCH", "/api/console/network-center/routes/{route_id}"),
                ("DELETE", "/api/console/network-center/routes/{route_id}"),
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
                description_key:
                    "console.policy_groups.settings.system.model-providers.description"
                        .to_string(),
                order: 200,
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
                description_key:
                    "console.policy_groups.settings.system.mcp-management.description".to_string(),
                order: 400,
            },
            api_routes: settings_api_routes(&[
                ("GET", "/api/console/mcp/catalog"),
                ("GET", "/api/console/mcp/interface-capabilities"),
                ("GET", "/api/console/mcp/list"),
                ("GET", "/api/console/mcp/export"),
                ("GET", "/api/console/mcp/instances"),
                ("POST", "/api/console/mcp/instances"),
                (
                    "POST",
                    "/api/console/mcp/instances/{instance_id}/copy",
                ),
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
                ("GET", "/api/console/mcp/bundles/export-defaults"),
                ("POST", "/api/console/mcp/bundles/export"),
                (
                    "POST",
                    "/api/console/mcp/instances/{instance_id}/bundles/export",
                ),
                ("POST", "/api/console/mcp/bundles/preview-upload"),
                ("POST", "/api/console/mcp/bundles/import-upload"),
                ("GET", "/api/console/mcp/upstream-connections"),
                ("POST", "/api/console/mcp/upstream-connections"),
                (
                    "PUT",
                    "/api/console/mcp/upstream-connections/{connection_id}",
                ),
                (
                    "DELETE",
                    "/api/console/mcp/upstream-connections/{connection_id}",
                ),
                (
                    "PUT",
                    "/api/console/mcp/upstream-connections/{connection_id}/credentials",
                ),
                (
                    "DELETE",
                    "/api/console/mcp/upstream-connections/{connection_id}/credentials",
                ),
                (
                    "POST",
                    "/api/console/mcp/upstream-connections/{connection_id}/test",
                ),
                ("POST", "/api/console/mcp/upstream-connections/test"),
                (
                    "POST",
                    "/api/console/mcp/upstream-connections/{connection_id}/discover",
                ),
                (
                    "POST",
                    "/api/console/mcp/upstream-connections/{connection_id}/imports",
                ),
                ("POST", "/api/console/mcp/tools/{tool_id}/debug"),
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
                description_key: "console.policy_groups.settings.system.roles.description"
                    .to_string(),
                order: 600,
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
