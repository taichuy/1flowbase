use std::collections::BTreeSet;

use access_control::{
    ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteAssemblyBinding, ConsoleRouteBinding,
    ConsoleRouteOwnership, SYSTEM_ROLES_SETTINGS_FEATURE_ID,
};
use api_server::{
    app,
    app_state::compile_core_settings_feature_registry,
    routes::console_route_assembly::{
        compile_migrated_core_console_operation_registry, migrated_core_console_route_assembly,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

fn assembled_route(
    method: &str,
    path: &str,
    ownership: ConsoleRouteOwnership,
) -> ConsoleRouteAssemblyBinding {
    ConsoleRouteAssemblyBinding {
        route: ConsoleRouteBinding {
            method: method.to_string(),
            path: path.to_string(),
        },
        ownership,
    }
}

#[tokio::test]
async fn ac_002_console_health_is_compiled_and_not_exposed_by_the_base_router() {
    let base_router = app();
    let public_health = base_router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let console_health = base_router
        .oneshot(
            Request::builder()
                .uri("/api/console/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(public_health.status(), StatusCode::OK);
    assert_eq!(console_health.status(), StatusCode::NOT_FOUND);
    assert!(migrated_core_console_route_assembly()
        .bindings()
        .iter()
        .any(|binding| {
            binding.route.method == "GET"
                && binding.route.path == "/api/console/health"
                && binding.ownership == ConsoleRouteOwnership::Authenticated
        }));
}

#[test]
fn ac_003_unknown_core_operation_metadata_fails_instead_of_becoming_other() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let mut bindings = migrated_core_console_route_assembly().bindings().to_vec();
    bindings.push(assembled_route(
        "GET",
        "/api/console/unknown-core-operation",
        ConsoleRouteOwnership::ConsoleOperation("core.unknown".to_string()),
    ));

    let error = compile_migrated_core_console_operation_registry(&settings, &bindings)
        .expect_err("an operation without an explicit Core specification must fail closed");

    assert!(error
        .to_string()
        .contains("no explicit Core or HostExtension operation specification for core.unknown"));
}

#[test]
fn ac_003_every_compiled_core_operation_has_declared_non_empty_i18n_metadata() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();
    let mut expected_operation_ids = BTreeSet::new();

    for binding in assembly.bindings() {
        match &binding.ownership {
            ConsoleRouteOwnership::Authenticated => {
                expected_operation_ids.insert("core.authenticated");
            }
            ConsoleRouteOwnership::ConsoleOperation(operation_id) => {
                expected_operation_ids.insert(operation_id.as_str());
            }
        }
    }

    let compiled_operation_ids = registry
        .inventory()
        .operations
        .iter()
        .map(|operation| operation.operation_id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(compiled_operation_ids, expected_operation_ids);
    assert!(registry.inventory().operations.iter().all(|operation| {
        !operation.label_ref.trim().is_empty()
            && operation
                .description_ref
                .as_deref()
                .is_some_and(|reference| !reference.trim().is_empty())
    }));
    let label_refs = registry
        .inventory()
        .operations
        .iter()
        .map(|operation| operation.label_ref.as_str())
        .collect::<BTreeSet<_>>();
    let description_refs = registry
        .inventory()
        .operations
        .iter()
        .filter_map(|operation| operation.description_ref.as_deref())
        .collect::<BTreeSet<_>>();
    assert_eq!(label_refs.len(), registry.inventory().operations.len());
    assert_eq!(
        description_refs.len(),
        registry.inventory().operations.len()
    );
}

#[test]
fn ac_009_core_compiled_catalog_resolves_every_active_display_reference_in_both_locales() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();
    let catalog = registry
        .inventory()
        .locale_catalog
        .as_ref()
        .expect("Core registry must compile its locale catalog");

    for locale in ["en_US", "zh_Hans"] {
        for operation in &registry.inventory().operations {
            assert!(catalog
                .text(locale, &operation.label_ref)
                .is_some_and(|text| !text.trim().is_empty()));
            let description_ref = operation
                .description_ref
                .as_deref()
                .expect("Core operation descriptions are compiled");
            assert!(catalog
                .text(locale, description_ref)
                .is_some_and(|text| !text.trim().is_empty()));
            assert_ne!(
                catalog.text(locale, &operation.label_ref),
                Some(operation.operation_id.as_str())
            );
            assert!(catalog
                .policy_group_display(&operation.policy_group, locale)
                .is_ok());
        }
        for resource in &registry.inventory().resources {
            assert!(catalog
                .text(locale, &resource.label_ref)
                .is_some_and(|text| !text.trim().is_empty()));
            let description_ref = resource
                .description_ref
                .as_deref()
                .expect("Core resource descriptions are compiled");
            assert!(catalog
                .text(locale, description_ref)
                .is_some_and(|text| !text.trim().is_empty()));
            for action in &resource.actions {
                assert!(catalog
                    .text(locale, &action.label_ref)
                    .is_some_and(|text| !text.trim().is_empty()));
                let description_ref = action
                    .description_ref
                    .as_deref()
                    .expect("Core resource action descriptions are compiled");
                assert!(catalog
                    .text(locale, description_ref)
                    .is_some_and(|text| !text.trim().is_empty()));
            }
        }
        assert_eq!(
            catalog
                .group_mode_options(locale)
                .unwrap()
                .into_iter()
                .map(|option| option.value)
                .collect::<Vec<_>>(),
            vec!["disabled", "full", "custom"]
        );
        assert_eq!(
            catalog
                .row_scope_options(locale)
                .unwrap()
                .into_iter()
                .map(|option| option.value)
                .collect::<Vec<_>>(),
            vec!["disabled", "own", "scope_all"]
        );
    }
}

// AC-002 / AC-009: routes with distinct authorization meanings must not be collapsed before
// legacy grants are mapped. The compiled inventory, rather than a URL heuristic, is the source
// of truth for these classifications.
#[test]
fn ac_002_009_operation_semantics_are_explicit_before_legacy_mapping() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();

    for (method, path) in [
        ("GET", "/api/console/workspace"),
        ("GET", "/api/console/workspaces"),
        ("POST", "/api/console/files/upload"),
        (
            "GET",
            "/api/console/files/:file_table_id/records/:record_id/content",
        ),
    ] {
        let access = registry.access_for_console_route(method, path).unwrap();
        assert_eq!(access.operation_id, "core.authenticated", "{method} {path}");
        assert_eq!(access.authorization, &ConsoleAuthorization::Authenticated);
    }

    for (method, path) in [
        ("GET", "/api/console/user-api-keys"),
        ("POST", "/api/console/user-api-keys"),
        ("GET", "/api/console/user-api-keys/role-options"),
        ("POST", "/api/console/user-api-keys/:api_key_id/revoke"),
    ] {
        let access = registry.access_for_console_route(method, path).unwrap();
        assert_eq!(
            access.operation_id, "user_api_keys.manage",
            "{method} {path}"
        );
        assert_eq!(access.authorization, &ConsoleAuthorization::Simple);
        assert_eq!(
            access.policy_group,
            &ConsolePolicyGroup::SettingsFeature("system.api-key-authentication".to_string())
        );
    }

    for (method, path, operation_id) in [
        (
            "GET",
            "/api/console/settings/data-models/model-definitions",
            "model_definitions.list",
        ),
        (
            "GET",
            "/api/console/models/agent-flow-options",
            "agent_flow.data_model_options.list",
        ),
        (
            "GET",
            "/api/console/settings/model-providers/options",
            "model_providers.settings_options.view",
        ),
        (
            "GET",
            "/api/console/model-providers/options",
            "model_providers.options.view",
        ),
    ] {
        assert_eq!(
            registry
                .access_for_console_route(method, path)
                .unwrap()
                .operation_id,
            operation_id,
            "{method} {path}"
        );
    }
    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/settings/data-models/model-definitions",)
            .unwrap()
            .policy_group,
        &ConsolePolicyGroup::SettingsFeature("system.data-models".to_string())
    );
    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/models/agent-flow-options")
            .unwrap()
            .policy_group,
        &ConsolePolicyGroup::Other("other.agent-flow".to_string())
    );
    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/settings/model-providers/options",)
            .unwrap()
            .policy_group,
        &ConsolePolicyGroup::SettingsFeature("system.model-providers".to_string())
    );
    assert_eq!(
        registry
            .access_for_console_route("GET", "/api/console/model-providers/options")
            .unwrap()
            .policy_group,
        &ConsolePolicyGroup::Other("other.model-providers".to_string())
    );

    for removed_operation_id in [
        "workspace.view",
        "workspaces.list",
        "files.upload",
        "files.content.download",
    ] {
        assert!(registry
            .inventory()
            .operations
            .iter()
            .all(|operation| operation.operation_id != removed_operation_id));
    }
}

#[test]
fn ac_013_role_console_policy_workers_have_explicit_metadata() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();

    for (operation_id, method, path) in [
        (
            "roles.console_policy_catalog.view",
            "GET",
            "/api/console/settings/roles/console-policy-catalog",
        ),
        (
            "roles.console_policy.view",
            "GET",
            "/api/console/settings/roles/:id/console-policy",
        ),
        (
            "roles.console_policy.replace",
            "PUT",
            "/api/console/settings/roles/:id/console-policy",
        ),
    ] {
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .expect("role console policy worker must be registered");
        let expected_label = format!("console.operations.{operation_id}.label");
        let expected_description = format!("console.operations.{operation_id}.description");

        assert_eq!(
            operation.policy_group,
            ConsolePolicyGroup::SettingsFeature(SYSTEM_ROLES_SETTINGS_FEATURE_ID.to_string())
        );
        assert_eq!(operation.authorization, ConsoleAuthorization::Simple);
        assert_eq!(operation.label_ref, expected_label);
        assert_eq!(
            operation.description_ref.as_deref(),
            Some(expected_description.as_str())
        );
        assert!(operation
            .routes
            .iter()
            .any(|route| route.method == method && route.path == path));
    }
}
