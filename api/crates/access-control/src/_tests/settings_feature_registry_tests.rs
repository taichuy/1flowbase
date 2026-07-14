use access_control::{
    core_settings_feature_registrations, AccessRule, SettingsApiRoute,
    SettingsFeatureConsoleSurface, SettingsFeatureLifecycle, SettingsFeatureOwner,
    SettingsFeatureOwnerKind, SettingsFeatureRegistration, SettingsFeatureRegistry,
};

fn feature(
    feature_id: &str,
    owner_kind: SettingsFeatureOwnerKind,
    owner_id: &str,
    api_routes: &[(&str, &str)],
) -> SettingsFeatureRegistration {
    SettingsFeatureRegistration {
        feature_id: feature_id.to_string(),
        owner: SettingsFeatureOwner {
            kind: owner_kind,
            owner_id: owner_id.to_string(),
            version: "1.0.0".to_string(),
        },
        lifecycle: SettingsFeatureLifecycle::Active,
        console_surface: SettingsFeatureConsoleSurface {
            route_id: format!("settings.{feature_id}"),
            surface_key: feature_id.to_string(),
            path: format!("/settings/{feature_id}"),
            label_key: format!("settings.{feature_id}"),
            order: 100,
        },
        api_routes: api_routes
            .iter()
            .map(|(method, path)| SettingsApiRoute {
                method: (*method).to_string(),
                path: (*path).to_string(),
            })
            .collect(),
    }
}

#[test]
fn ac_001_explicit_core_settings_features_compile_exact_method_path_inventory() {
    let registry = SettingsFeatureRegistry::compile(core_settings_feature_registrations())
        .expect("Core SettingsFeature inventory must compile");

    let routes = |feature_id: &str| {
        registry
            .inventory()
            .features
            .iter()
            .find(|feature| feature.feature_id == feature_id)
            .unwrap_or_else(|| panic!("missing Core SettingsFeature {feature_id}"))
            .api_routes
            .iter()
            .map(|route| (route.method.as_str(), route.path.as_str()))
            .collect::<Vec<_>>()
    };

    assert_eq!(routes("system.model-providers").len(), 26);
    assert_eq!(
        routes("system.model-providers"),
        vec![
            (
                "DELETE",
                "/api/console/settings/model-providers/instances/{id}",
            ),
            (
                "DELETE",
                "/api/console/settings/model-providers/plugins/families/{provider_code}",
            ),
            (
                "DELETE",
                "/api/console/settings/model-providers/request-logs",
            ),
            ("GET", "/api/console/settings/model-providers/catalog"),
            ("GET", "/api/console/settings/model-providers/instances"),
            (
                "GET",
                "/api/console/settings/model-providers/instances/{id}/models",
            ),
            ("GET", "/api/console/settings/model-providers/options"),
            (
                "GET",
                "/api/console/settings/model-providers/plugins/families",
            ),
            (
                "GET",
                "/api/console/settings/model-providers/plugins/official-catalog",
            ),
            (
                "GET",
                "/api/console/settings/model-providers/plugins/tasks/{task_id}",
            ),
            (
                "GET",
                "/api/console/settings/model-providers/providers/{provider_code}/main-instance",
            ),
            (
                "GET",
                "/api/console/settings/model-providers/request-logs",
            ),
            (
                "PATCH",
                "/api/console/settings/model-providers/instances/{id}",
            ),
            ("POST", "/api/console/settings/model-providers/instances"),
            (
                "POST",
                "/api/console/settings/model-providers/instances/{id}/models/refresh",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/instances/{id}/secrets/reveal",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/instances/{id}/validate",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/families/{provider_code}/switch-version",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/families/{provider_code}/upgrade-latest",
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
                "/api/console/settings/model-providers/plugins/{installation_id}/artifact/install-current-node",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/{installation_id}/artifact/refresh",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/preview-models",
            ),
            (
                "POST",
                "/api/console/settings/model-providers/request-logs/clear",
            ),
            (
                "PUT",
                "/api/console/settings/model-providers/providers/{provider_code}/main-instance",
            ),
        ]
    );

    assert_eq!(
        routes("system.auth-center"),
        vec![
            (
                "DELETE",
                "/api/console/settings/auth-center/authenticators/{id}",
            ),
            ("GET", "/api/console/settings/auth-center/overview"),
            ("POST", "/api/console/settings/auth-center/authenticators"),
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
                "/api/console/settings/auth-center/authenticators/order",
            ),
            (
                "PUT",
                "/api/console/settings/auth-center/authenticators/{id}/config",
            ),
        ]
    );
    assert_eq!(
        routes("system.host-infrastructure"),
        vec![
            ("GET", "/api/console/settings/host-infrastructure/cache"),
            (
                "GET",
                "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/entries",
            ),
            ("GET", "/api/console/settings/host-infrastructure/providers"),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/clear",
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/entries/clear",
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/{domain_code}/entries/reveal",
            ),
            (
                "PUT",
                "/api/console/settings/host-infrastructure/providers/{installation_id}/{provider_code}/config",
            ),
        ]
    );
    assert_eq!(
        routes("system.memory-observation"),
        vec![
            ("GET", "/api/console/settings/host-infrastructure/memory"),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/entries",
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/entries/search",
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/stats",
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/tree",
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/stats",
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/memory/contracts/{contract_code}/entries/reveal",
            ),
        ]
    );
    assert_eq!(
        routes("system.applications"),
        vec![("GET", "/api/console/settings/applications")]
    );
    assert_eq!(
        routes("system.docs"),
        vec![
            ("GET", "/api/console/docs/catalog"),
            (
                "GET",
                "/api/console/docs/categories/{category_id}/openapi.json",
            ),
            (
                "GET",
                "/api/console/docs/categories/{category_id}/operations",
            ),
            (
                "GET",
                "/api/console/docs/operations/{operation_id}/openapi.json",
            ),
        ]
    );
    assert_eq!(
        routes("system.api-key-authentication"),
        vec![
            ("GET", "/api/console/user-api-keys"),
            ("GET", "/api/console/user-api-keys/role-options"),
            ("POST", "/api/console/user-api-keys"),
            ("POST", "/api/console/user-api-keys/{api_key_id}/revoke",),
        ]
    );
    assert_eq!(
        routes("system.system-runtime"),
        vec![
            ("GET", "/api/console/system/release-status"),
            ("GET", "/api/console/system/runtime-profile"),
        ]
    );
    assert!(routes("system.mcp-management").contains(&("GET", "/api/console/mcp/catalog")));
    assert!(routes("system.mcp-management").contains(&("POST", "/api/console/mcp/instances")));
    assert_eq!(
        routes("system.files"),
        vec![
            ("DELETE", "/api/console/settings/files/storages/{id}"),
            ("DELETE", "/api/console/settings/files/tables/{id}"),
            ("GET", "/api/console/settings/files/storages"),
            ("GET", "/api/console/settings/files/tables"),
            ("POST", "/api/console/settings/files/storages"),
            ("POST", "/api/console/settings/files/tables"),
            ("PUT", "/api/console/settings/files/storages/{id}"),
            ("PUT", "/api/console/settings/files/tables/{id}/binding",),
        ]
    );
    assert_eq!(
        routes("system.data-models"),
        vec![
            (
                "DELETE",
                "/api/console/settings/data-models/model-definitions/{id}",
            ),
            (
                "DELETE",
                "/api/console/settings/data-models/model-definitions/{id}/fields/{field_id}",
            ),
            (
                "GET",
                "/api/console/settings/data-models/data-sources",
            ),
            (
                "GET",
                "/api/console/settings/data-models/data-sources/catalog",
            ),
            (
                "GET",
                "/api/console/settings/data-models/data-sources/{data_source_id}/resources",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions/{id}/advisor-findings",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions/{id}/scope-grants",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions/{model_id}/openapi.json",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/data-sources/{data_source_id}/defaults",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/model-definitions/{id}",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/model-definitions/{id}/fields/{field_id}",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/model-definitions/{id}/scope-grants/{grant_id}",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/{data_source_id}/preview-read",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/{data_source_id}/resources/discover",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/{data_source_id}/resources/map-to-model",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/{data_source_id}/validate",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions/{id}/fields",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions/{id}/scope-grants",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions:batchDelete",
            ),
        ]
    );
}

#[test]
fn ac_001_core_and_host_extension_compile_one_stably_sorted_inventory() {
    let registry = SettingsFeatureRegistry::compile([
        feature(
            "system.roles",
            SettingsFeatureOwnerKind::Core,
            "core",
            &[
                ("POST", "/api/console/settings/roles"),
                ("GET", "/api/console/settings/roles"),
            ],
        ),
        feature(
            "file-security",
            SettingsFeatureOwnerKind::HostExtension,
            "file-security",
            &[("PUT", "/api/console/settings/file-security")],
        ),
    ])
    .expect("valid Core and HostExtension registrations must compile");

    let inventory = registry.inventory();
    let feature_ids = inventory
        .features
        .iter()
        .map(|feature| feature.feature_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        inventory.schema_version,
        "1flowbase.settings-feature-inventory/v1"
    );
    assert_eq!(feature_ids, vec!["file-security", "system.roles"]);
    assert_eq!(
        inventory.features[1]
            .api_routes
            .iter()
            .map(|route| (route.method.as_str(), route.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("GET", "/api/console/settings/roles"),
            ("POST", "/api/console/settings/roles")
        ]
    );
    assert_eq!(
        registry.access_rule("GET", "/api/console/settings/roles"),
        Some(&AccessRule::SettingsFeature("system.roles".to_string()))
    );
}

#[test]
fn ac_002_duplicate_feature_id_fails_closed() {
    let error = SettingsFeatureRegistry::compile([
        feature(
            "system.roles",
            SettingsFeatureOwnerKind::Core,
            "core",
            &[("GET", "/api/console/settings/roles")],
        ),
        feature(
            "system.roles",
            SettingsFeatureOwnerKind::HostExtension,
            "roles-extension",
            &[("GET", "/api/console/extension-roles")],
        ),
    ])
    .expect_err("duplicate feature_id must fail closed");

    assert!(error
        .to_string()
        .contains("duplicate feature_id system.roles"));
}

#[test]
fn ac_002_duplicate_method_and_path_ownership_fails_closed() {
    let error = SettingsFeatureRegistry::compile([
        feature(
            "system.roles",
            SettingsFeatureOwnerKind::Core,
            "core",
            &[("GET", "/api/console/settings/roles")],
        ),
        feature(
            "system.members",
            SettingsFeatureOwnerKind::Core,
            "core",
            &[("get", "/api/console/settings/roles")],
        ),
    ])
    .expect_err("duplicate method + path ownership must fail closed");

    assert!(error
        .to_string()
        .contains("duplicate Settings API ownership GET /api/console/settings/roles"));
}

#[test]
fn ac_002_missing_owner_fails_closed() {
    let error = SettingsFeatureRegistry::compile([feature(
        "system.roles",
        SettingsFeatureOwnerKind::Core,
        "",
        &[("GET", "/api/console/settings/roles")],
    )])
    .expect_err("missing owner must fail closed");

    assert!(error
        .to_string()
        .contains("settings feature owner_id must not be empty"));
}

#[test]
fn ac_002_inactive_owner_with_api_routes_fails_closed() {
    let mut registration = feature(
        "file-security",
        SettingsFeatureOwnerKind::HostExtension,
        "file-security",
        &[("PUT", "/api/console/settings/file-security")],
    );
    registration.lifecycle = SettingsFeatureLifecycle::Inactive;

    let error = SettingsFeatureRegistry::compile([registration])
        .expect_err("inactive owner API registration must fail closed");

    assert!(error
        .to_string()
        .contains("inactive settings feature file-security cannot own API routes"));
}
