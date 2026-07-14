use crate::{
    ConsoleAuthorization, ConsoleOperationOwner, ConsoleOperationRegistration,
    ConsoleOperationRegistry, ConsolePolicyGroup, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, ResourceAccessAction, ResourceAccessRegistration,
    ResourceAccessScopeKind, SettingsApiRoute, SettingsFeatureConsoleSurface,
    SettingsFeatureLifecycle, SettingsFeatureOwner, SettingsFeatureOwnerKind,
    SettingsFeatureRegistration, SettingsFeatureRegistry,
};

fn owner(kind: SettingsFeatureOwnerKind, owner_id: &str) -> ConsoleOperationOwner {
    ConsoleOperationOwner {
        kind,
        owner_id: owner_id.to_string(),
        version: "1.0.0".to_string(),
    }
}

fn settings_feature(feature_id: &str, method: &str, path: &str) -> SettingsFeatureRegistration {
    SettingsFeatureRegistration {
        feature_id: feature_id.to_string(),
        owner: SettingsFeatureOwner {
            kind: SettingsFeatureOwnerKind::Core,
            owner_id: "boot-core".to_string(),
            version: "1.0.0".to_string(),
        },
        lifecycle: SettingsFeatureLifecycle::Active,
        console_surface: SettingsFeatureConsoleSurface {
            route_id: format!("settings.{feature_id}"),
            surface_key: feature_id.to_string(),
            path: format!("/settings/{feature_id}"),
            label_key: format!("settings.{feature_id}.label"),
            order: 100,
        },
        api_routes: vec![SettingsApiRoute {
            method: method.to_string(),
            path: path.to_string(),
        }],
    }
}

fn route(method: &str, path: &str) -> ConsoleRouteBinding {
    ConsoleRouteBinding {
        method: method.to_string(),
        path: path.to_string(),
    }
}

fn assembled_route(
    method: &str,
    path: &str,
    ownership: ConsoleRouteOwnership,
) -> ConsoleRouteAssemblyBinding {
    ConsoleRouteAssemblyBinding {
        route: route(method, path),
        ownership,
    }
}

fn operation(
    operation_id: &str,
    policy_group: ConsolePolicyGroup,
    authorization: ConsoleAuthorization,
    routes: Vec<ConsoleRouteBinding>,
) -> ConsoleOperationRegistration {
    ConsoleOperationRegistration {
        operation_id: operation_id.to_string(),
        owner: owner(SettingsFeatureOwnerKind::Core, "boot-core"),
        lifecycle: SettingsFeatureLifecycle::Active,
        policy_group,
        label_ref: format!("console.operations.{operation_id}.label"),
        description_ref: Some(format!("console.operations.{operation_id}.description")),
        order: 100,
        routes,
        authorization,
    }
}

fn applications_settings_registry() -> SettingsFeatureRegistry {
    SettingsFeatureRegistry::compile([settings_feature(
        "system.applications",
        "GET",
        "/api/console/settings/applications",
    )])
    .expect("settings fixture must compile")
}

#[test]
fn explicit_operation_claim_only_removes_its_route_from_legacy_feature_projection() {
    let mut feature = settings_feature(
        "system.files",
        "GET",
        "/api/console/settings/files/storages",
    );
    feature.api_routes.push(SettingsApiRoute {
        method: "POST".to_string(),
        path: "/api/console/settings/files/storages".to_string(),
    });
    let settings_registry =
        SettingsFeatureRegistry::compile([feature]).expect("settings fixture must compile");

    let registry = ConsoleOperationRegistry::compile(
        &settings_registry,
        [operation(
            "file_storages.create",
            ConsolePolicyGroup::SettingsFeature("system.files".to_string()),
            ConsoleAuthorization::Simple,
            vec![route("POST", "/api/console/settings/files/storages")],
        )],
        [],
    )
    .expect("partial stable operation claim must compile");

    let legacy = registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == "settings_feature.access.system.files")
        .expect("unclaimed legacy route must remain projected");
    assert_eq!(
        legacy.routes,
        vec![route("GET", "/api/console/settings/files/storages")]
    );
    assert_eq!(
        registry
            .access_for_console_route("POST", "/api/console/settings/files/storages")
            .unwrap()
            .operation_id,
        "file_storages.create"
    );
    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/settings/files/storages")
            .unwrap()
            .operation_id,
        "settings_feature.access.system.files"
    );
}

#[test]
fn ac_001_core_and_host_extension_compile_one_operation_resource_route_inventory() {
    let settings_registry = applications_settings_registry();
    let mut host_scan = operation(
        "file-security.scan",
        ConsolePolicyGroup::Other("other.general".to_string()),
        ConsoleAuthorization::ResourceAction {
            resource_code: "file-security.secured-files".to_string(),
            action_code: "scan".to_string(),
        },
        vec![route("POST", "/api/console/secured-files/{file_id}/scan")],
    );
    host_scan.owner = owner(SettingsFeatureOwnerKind::HostExtension, "file-security");

    let resources = [ResourceAccessRegistration {
        resource_code: "file-security.secured-files".to_string(),
        owner: owner(SettingsFeatureOwnerKind::HostExtension, "file-security"),
        lifecycle: SettingsFeatureLifecycle::Active,
        scope_kind: ResourceAccessScopeKind::Workspace,
        identity_field: "id".to_string(),
        scope_field: Some("scope_id".to_string()),
        owner_field: Some("created_by".to_string()),
        label_ref: "resources.secured_files.label".to_string(),
        description_ref: Some("resources.secured_files.description".to_string()),
        actions: vec![ResourceAccessAction {
            action_code: "scan".to_string(),
            label_ref: "resources.secured_files.actions.scan.label".to_string(),
            description_ref: None,
        }],
    }];

    let registry = ConsoleOperationRegistry::compile(
        &settings_registry,
        [
            operation(
                "applications.publish",
                ConsolePolicyGroup::SettingsFeature("system.applications".to_string()),
                ConsoleAuthorization::Simple,
                vec![route(
                    "POST",
                    "/api/console/applications/{application_id}/publish",
                )],
            ),
            host_scan,
        ],
        resources,
    )
    .expect("shared Core/HostExtension registry must compile");

    let operation_ids = registry
        .inventory()
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        operation_ids,
        vec![
            "applications.publish",
            "file-security.scan",
            "settings_feature.access.system.applications",
        ]
    );
    assert_eq!(registry.inventory().resources[0].identity_field, "id");
    assert_eq!(
        registry.inventory().resources[0].scope_field.as_deref(),
        Some("scope_id")
    );
    assert_eq!(
        registry.inventory().resources[0].owner_field.as_deref(),
        Some("created_by")
    );

    let settings_access = registry
        .access_for_console_route("GET", "/api/console/settings/applications")
        .expect("#1256 Settings API ownership must be projected, not copied");
    assert_eq!(
        settings_access.operation_id,
        "settings_feature.access.system.applications"
    );
    assert_eq!(
        settings_access.policy_group,
        &ConsolePolicyGroup::SettingsFeature("system.applications".to_string())
    );
}

#[test]
fn ac_002_console_route_assembly_literal_specificity_resolves_before_parameter_route() {
    let settings_registry = applications_settings_registry();
    let registry = ConsoleOperationRegistry::compile(
        &settings_registry,
        [
            operation(
                "applications.read",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Authenticated,
                vec![route("GET", "/api/console/applications/{id}")],
            ),
            operation(
                "applications.create",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Simple,
                vec![route("GET", "/api/console/applications/catalog")],
            ),
        ],
        [],
    )
    .expect("a literal route must be allowed to specialize a parameter route");

    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/applications/catalog")
            .unwrap()
            .operation_id,
        "applications.create"
    );
    assert_eq!(
        registry
            .access_for_console_route(
                "GET",
                "/api/console/applications/00000000-0000-0000-0000-000000000001",
            )
            .unwrap()
            .operation_id,
        "applications.read"
    );
}

#[test]
fn ac_002_console_route_assembly_fails_closed_for_ambiguous_or_unregistered_routes() {
    let settings_registry = applications_settings_registry();
    let duplicate_templates = ConsoleOperationRegistry::compile(
        &settings_registry,
        [
            operation(
                "applications.read",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Authenticated,
                vec![route("GET", "/api/console/applications/{id}")],
            ),
            operation(
                "applications.inspect",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Simple,
                vec![route("GET", "/api/console/applications/{application_id}")],
            ),
        ],
        [],
    )
    .unwrap_err();
    assert!(duplicate_templates
        .to_string()
        .contains("duplicate console route ownership"));

    let crossing_specificity = ConsoleOperationRegistry::compile(
        &settings_registry,
        [
            operation(
                "applications.read-catalog",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Authenticated,
                vec![route("GET", "/api/console/applications/{id}/catalog")],
            ),
            operation(
                "applications.inspect-section",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Simple,
                vec![route("GET", "/api/console/applications/special/{section}")],
            ),
        ],
        [],
    )
    .unwrap_err();
    assert!(crossing_specificity
        .to_string()
        .contains("duplicate console route ownership"));

    let dangling_feature = ConsoleOperationRegistry::compile(
        &settings_registry,
        [operation(
            "missing-feature.operation",
            ConsolePolicyGroup::SettingsFeature("system.missing".to_string()),
            ConsoleAuthorization::Simple,
            vec![route("POST", "/api/console/missing-feature/run")],
        )],
        [],
    )
    .unwrap_err();
    assert!(dangling_feature
        .to_string()
        .contains("unknown settings feature"));

    let dangling_action = ConsoleOperationRegistry::compile(
        &settings_registry,
        [operation(
            "applications.update",
            ConsolePolicyGroup::Other("other.general".to_string()),
            ConsoleAuthorization::ResourceAction {
                resource_code: "applications".to_string(),
                action_code: "update".to_string(),
            },
            vec![route("PATCH", "/api/console/applications/{id}")],
        )],
        [],
    )
    .unwrap_err();
    assert!(dangling_action
        .to_string()
        .contains("unknown resource action"));

    let mut inactive = operation(
        "inactive.operation",
        ConsolePolicyGroup::Other("other.general".to_string()),
        ConsoleAuthorization::Simple,
        vec![route("POST", "/api/console/inactive/run")],
    );
    inactive.lifecycle = SettingsFeatureLifecycle::Inactive;
    let inactive_owner =
        ConsoleOperationRegistry::compile(&settings_registry, [inactive], []).unwrap_err();
    assert!(inactive_owner
        .to_string()
        .contains("inactive operation inactive.operation"));

    let registry = ConsoleOperationRegistry::compile(&settings_registry, [], []).unwrap();
    assert!(registry
        .access_for_console_route("GET", "/api/console/unregistered")
        .unwrap_err()
        .to_string()
        .contains("unregistered console route"));
    assert!(registry
        .validate_console_route_coverage([assembled_route(
            "GET",
            "/api/console/unregistered",
            ConsoleRouteOwnership::Authenticated,
        )])
        .unwrap_err()
        .to_string()
        .contains("missing compiled ownership"));
}

#[test]
fn ac_002_console_route_assembly_rejects_duplicate_ownership() {
    let settings_registry = applications_settings_registry();
    let registry = ConsoleOperationRegistry::compile(&settings_registry, [], []).unwrap();
    let duplicate = registry
        .validate_console_route_coverage([
            assembled_route(
                "GET",
                "/api/console/settings/applications",
                ConsoleRouteOwnership::ConsoleOperation(
                    "settings_feature.access.system.applications".to_string(),
                ),
            ),
            assembled_route(
                "GET",
                "/api/console/settings/applications",
                ConsoleRouteOwnership::Authenticated,
            ),
        ])
        .unwrap_err();

    assert!(duplicate
        .to_string()
        .contains("duplicate assembled console route ownership"));
}

#[test]
fn ac_002_console_route_assembly_rejects_unmounted_compiled_route() {
    let settings_registry = applications_settings_registry();
    let registry = ConsoleOperationRegistry::compile(&settings_registry, [], []).unwrap();
    let unmounted = registry.validate_console_route_coverage([]).unwrap_err();

    assert!(unmounted
        .to_string()
        .contains("compiled console route ownership is not mounted"));
    assert!(unmounted
        .to_string()
        .contains("GET /api/console/settings/applications"));
}

#[test]
fn ac_003_other_to_settings_feature_changes_only_compiled_grouping() {
    let settings_registry = applications_settings_registry();
    let other = operation(
        "applications.publish",
        ConsolePolicyGroup::Other("other.general".to_string()),
        ConsoleAuthorization::Simple,
        vec![route(
            "POST",
            "/api/console/applications/{application_id}/publish",
        )],
    );
    let baseline = ConsoleOperationRegistry::compile(&settings_registry, [other.clone()], [])
        .expect("Other registration must compile");
    let mut grouped = other;
    grouped.policy_group = ConsolePolicyGroup::SettingsFeature("system.applications".to_string());
    let current = ConsoleOperationRegistry::compile(&settings_registry, [grouped], [])
        .expect("SettingsFeature registration must compile");

    let before = baseline
        .access_for_console_route(
            "POST",
            "/api/console/applications/01J00000000000000000000000/publish",
        )
        .unwrap();
    let after = current
        .access_for_console_route(
            "POST",
            "/api/console/applications/01J00000000000000000000000/publish",
        )
        .unwrap();
    assert_eq!(before.operation_id, after.operation_id);
    assert_eq!(before.authorization, after.authorization);

    let diff = current.diff(&baseline);
    assert!(diff.added_operations.is_empty());
    assert!(diff.removed_operations.is_empty());
    assert!(diff.changed_operations.is_empty());
    assert_eq!(diff.policy_group_changes.len(), 1);
    assert_eq!(
        diff.policy_group_changes[0].operation_id,
        "applications.publish"
    );
}

#[test]
fn ac_007_non_crud_simple_operation_and_authenticated_route_are_explicit() {
    let settings_registry = applications_settings_registry();
    let registry = ConsoleOperationRegistry::compile(
        &settings_registry,
        [
            operation(
                "applications.publish",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Simple,
                vec![route(
                    "POST",
                    "/api/console/applications/{application_id}/publish",
                )],
            ),
            operation(
                "session.profile.read",
                ConsolePolicyGroup::Other("other.general".to_string()),
                ConsoleAuthorization::Authenticated,
                vec![route("GET", "/api/console/session/profile")],
            ),
        ],
        [],
    )
    .unwrap();

    assert_eq!(
        registry
            .access_for_console_route(
                "POST",
                "/api/console/applications/01J00000000000000000000000/publish",
            )
            .unwrap()
            .authorization,
        &ConsoleAuthorization::Simple
    );
    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/session/profile")
            .unwrap()
            .authorization,
        &ConsoleAuthorization::Authenticated
    );
}
