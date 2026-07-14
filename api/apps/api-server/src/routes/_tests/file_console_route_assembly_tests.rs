use access_control::{
    ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteOwnership,
    FILE_STORAGES_CREATE_OPERATION_ID, FILE_STORAGES_DELETE_OPERATION_ID,
    FILE_STORAGES_LIST_OPERATION_ID, FILE_STORAGES_UPDATE_OPERATION_ID,
    FILE_TABLES_CREATE_OPERATION_ID, FILE_TABLES_DELETE_OPERATION_ID,
    FILE_TABLES_LIST_OPERATION_ID, FILE_TABLES_STORAGE_BIND_OPERATION_ID,
    SYSTEM_FILES_SETTINGS_FEATURE_ID,
};

use crate::{
    app_state::compile_core_settings_feature_registry,
    routes::console_route_assembly::{
        compile_migrated_core_console_operation_registry, migrated_core_console_route_assembly,
    },
};

#[test]
fn file_owner_routes_compile_from_exact_assemblies() {
    let assembly = crate::routes::files::route_assembly()
        .merge(crate::routes::file_storages::route_assembly())
        .merge(crate::routes::file_tables::route_assembly());

    let actual = assembly
        .bindings()
        .iter()
        .map(|binding| {
            (
                binding.route.method.as_str(),
                binding.route.path.as_str(),
                match &binding.ownership {
                    ConsoleRouteOwnership::Authenticated => "authenticated",
                    ConsoleRouteOwnership::ConsoleOperation(operation_id) => operation_id.as_str(),
                },
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            ("POST", "/api/console/files/upload", "authenticated",),
            (
                "GET",
                "/api/console/files/:file_table_id/records/:record_id/content",
                "authenticated",
            ),
            (
                "GET",
                "/api/console/settings/files/storages",
                FILE_STORAGES_LIST_OPERATION_ID,
            ),
            (
                "POST",
                "/api/console/settings/files/storages",
                FILE_STORAGES_CREATE_OPERATION_ID,
            ),
            (
                "PUT",
                "/api/console/settings/files/storages/:id",
                FILE_STORAGES_UPDATE_OPERATION_ID,
            ),
            (
                "DELETE",
                "/api/console/settings/files/storages/:id",
                FILE_STORAGES_DELETE_OPERATION_ID,
            ),
            (
                "GET",
                "/api/console/settings/files/tables",
                FILE_TABLES_LIST_OPERATION_ID,
            ),
            (
                "POST",
                "/api/console/settings/files/tables",
                FILE_TABLES_CREATE_OPERATION_ID,
            ),
            (
                "DELETE",
                "/api/console/settings/files/tables/:id",
                FILE_TABLES_DELETE_OPERATION_ID,
            ),
            (
                "PUT",
                "/api/console/settings/files/tables/:id/binding",
                FILE_TABLES_STORAGE_BIND_OPERATION_ID,
            ),
        ]
    );
}

#[test]
fn file_owner_routes_preserve_authenticated_data_acl_and_settings_operations() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();

    for binding in assembly.bindings().iter().filter(|binding| {
        binding.route.path.starts_with("/api/console/files/")
            || binding
                .route
                .path
                .starts_with("/api/console/settings/files/")
    }) {
        let access = registry
            .access_for_console_route(&binding.route.method, &binding.route.path)
            .unwrap();
        match &binding.ownership {
            ConsoleRouteOwnership::Authenticated => {
                assert_eq!(access.operation_id, "core.authenticated");
                assert_eq!(access.authorization, &ConsoleAuthorization::Authenticated);
            }
            ConsoleRouteOwnership::ConsoleOperation(expected_operation_id) => {
                assert_eq!(access.operation_id, expected_operation_id);
            }
        }
    }

    for removed_operation_id in ["files.upload", "files.content.download"] {
        assert!(
            registry
                .inventory()
                .operations
                .iter()
                .all(|operation| operation.operation_id != removed_operation_id)
        );
    }

    for operation_id in [
        FILE_STORAGES_LIST_OPERATION_ID,
        FILE_STORAGES_CREATE_OPERATION_ID,
        FILE_STORAGES_UPDATE_OPERATION_ID,
        FILE_STORAGES_DELETE_OPERATION_ID,
        FILE_TABLES_LIST_OPERATION_ID,
        FILE_TABLES_CREATE_OPERATION_ID,
        FILE_TABLES_DELETE_OPERATION_ID,
        FILE_TABLES_STORAGE_BIND_OPERATION_ID,
    ] {
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .unwrap();
        assert_eq!(operation.authorization, ConsoleAuthorization::Simple);
        assert_eq!(
            operation.policy_group,
            ConsolePolicyGroup::SettingsFeature(SYSTEM_FILES_SETTINGS_FEATURE_ID.to_string())
        );
        assert_eq!(operation.routes.len(), 1);
    }

    assert!(
        registry
            .inventory()
            .operations
            .iter()
            .all(|operation| operation.operation_id != "settings_feature.access.system.files")
    );
    let partially_migrated_feature = registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == "settings_feature.access.system.data-models")
        .unwrap();
    assert_eq!(
        partially_migrated_feature.routes,
        vec![access_control::ConsoleRouteBinding {
            method: "GET".to_string(),
            path: "/api/console/settings/data-models/data-sources/catalog".to_string(),
        }]
    );

    assert!(registry.inventory().resources.iter().all(|resource| {
        !matches!(
            resource.resource_code.as_str(),
            "files" | "file_storages" | "file_tables"
        )
    }));
}
