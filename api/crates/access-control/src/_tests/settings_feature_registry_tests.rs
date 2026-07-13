use access_control::{
    AccessRule, SettingsApiRoute, SettingsFeatureConsoleSurface, SettingsFeatureLifecycle,
    SettingsFeatureOwner, SettingsFeatureOwnerKind, SettingsFeatureRegistration,
    SettingsFeatureRegistry,
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
fn ac_001_core_and_host_extension_compile_one_stably_sorted_inventory() {
    let registry = SettingsFeatureRegistry::compile([
        feature(
            "system.roles",
            SettingsFeatureOwnerKind::Core,
            "core",
            &[
                ("POST", "/api/console/roles"),
                ("GET", "/api/console/roles"),
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
            ("GET", "/api/console/roles"),
            ("POST", "/api/console/roles")
        ]
    );
    assert_eq!(
        registry.access_rule("GET", "/api/console/roles"),
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
            &[("GET", "/api/console/roles")],
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
            &[("GET", "/api/console/roles")],
        ),
        feature(
            "system.members",
            SettingsFeatureOwnerKind::Core,
            "core",
            &[("get", "/api/console/roles")],
        ),
    ])
    .expect_err("duplicate method + path ownership must fail closed");

    assert!(error
        .to_string()
        .contains("duplicate Settings API ownership GET /api/console/roles"));
}

#[test]
fn ac_002_missing_owner_fails_closed() {
    let error = SettingsFeatureRegistry::compile([feature(
        "system.roles",
        SettingsFeatureOwnerKind::Core,
        "",
        &[("GET", "/api/console/roles")],
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
