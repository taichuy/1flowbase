use access_control::{
    ConsoleAuthorization, ConsoleOperationRegistry, ConsolePolicyGroup,
    ConsoleRouteAssemblyBinding, ConsoleRouteBinding, ConsoleRouteOwnership,
    ResourceAccessScopeKind,
};

use crate::{
    app_state::compile_core_settings_feature_registry,
    routes::console_route_assembly::{
        compile_migrated_core_console_operation_registry, migrated_core_console_route_assembly,
        ConsoleRouteAssembly,
    },
};

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

fn route_bindings<S>(assembly: &ConsoleRouteAssembly<S>) -> Vec<(&str, &str, &str)>
where
    S: Clone + Send + Sync + 'static,
{
    assembly
        .bindings()
        .iter()
        .map(|binding| {
            (
                binding.route.method.as_str(),
                binding.route.path.as_str(),
                match &binding.ownership {
                    ConsoleRouteOwnership::ConsoleOperation(operation_id) => operation_id.as_str(),
                    ConsoleRouteOwnership::Authenticated => "authenticated",
                },
            )
        })
        .collect()
}

#[test]
fn console_route_assembly_unclassified_route_fails_coverage() {
    let settings = access_control::SettingsFeatureRegistry::compile([]).unwrap();
    let registry = ConsoleOperationRegistry::compile(&settings, [], []).unwrap();
    let error = registry
        .validate_console_route_coverage([assembled_route(
            "GET",
            "/api/console/session",
            ConsoleRouteOwnership::Authenticated,
        )])
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("missing compiled ownership: GET /api/console/session"));
}

#[test]
fn console_route_assembly_duplicate_ownership_fails_coverage() {
    let settings = access_control::SettingsFeatureRegistry::compile([]).unwrap();
    let registry = ConsoleOperationRegistry::compile(&settings, [], []).unwrap();
    let error = registry
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

    assert!(error
        .to_string()
        .contains("duplicate assembled console route ownership"));
}

#[test]
fn migrated_real_core_console_route_assembly_has_compiled_coverage() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();

    let compiled_routes = registry
        .inventory()
        .operations
        .iter()
        .flat_map(|operation| {
            operation
                .routes
                .iter()
                .cloned()
                .map(|route| ConsoleRouteAssemblyBinding {
                    route,
                    ownership: if matches!(
                        operation.authorization,
                        ConsoleAuthorization::Authenticated
                    ) {
                        ConsoleRouteOwnership::Authenticated
                    } else {
                        ConsoleRouteOwnership::ConsoleOperation(operation.operation_id.clone())
                    },
                })
        });
    registry
        .validate_console_route_coverage(compiled_routes)
        .unwrap();
    assert!(assembly.bindings().iter().any(|binding| {
        binding.route.path == "/api/console/settings/applications"
            && binding.ownership
                == ConsoleRouteOwnership::ConsoleOperation(
                    "settings_feature.access.system.applications".to_string(),
                )
    }));
}

#[test]
fn applications_routes_compile_exact_operations_and_resource_metadata() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, assembly.bindings()).unwrap();
    let application_bindings = assembly
        .bindings()
        .iter()
        .filter(|binding| binding.route.path.starts_with("/api/console/applications"))
        .collect::<Vec<_>>();
    let owner_assembly = crate::routes::applications::route_assembly()
        .merge(crate::routes::application_api::route_assembly())
        .merge(crate::routes::application_orchestration::route_assembly())
        .merge(crate::routes::application_runtime::route_assembly());
    assert_eq!(
        application_bindings,
        owner_assembly.bindings().iter().collect::<Vec<_>>(),
        "the migrated assembly must consume every application owner assembly without a copied route list"
    );

    let critical_routes = [
        ("GET", "/api/console/applications", "applications.view"),
        ("POST", "/api/console/applications", "applications.create"),
        (
            "GET",
            "/api/console/applications/00000000-0000-0000-0000-000000000001",
            "applications.view",
        ),
        (
            "PATCH",
            "/api/console/applications/00000000-0000-0000-0000-000000000001",
            "applications.update",
        ),
        (
            "DELETE",
            "/api/console/applications/00000000-0000-0000-0000-000000000001",
            "applications.delete",
        ),
        (
            "POST",
            "/api/console/applications/00000000-0000-0000-0000-000000000001/api-publications",
            "applications.publish",
        ),
        (
            "POST",
            "/api/console/applications/00000000-0000-0000-0000-000000000001/orchestration/debug-runs",
            "applications.run",
        ),
    ];
    for (method, path, authorization_profile_id) in critical_routes {
        let access = registry.access_for_console_route(method, path).unwrap();
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == access.operation_id)
            .expect("compiled access must reference an inventory operation");
        assert_eq!(operation.authorization_profile_id, authorization_profile_id);
        assert!(registry.inventory().interfaces.iter().any(|interface| {
            interface.authorization_operation_id.as_deref() == Some(access.operation_id)
        }));
    }

    assert_eq!(
        registry
            .access_for_console_route("POST", "/api/console/applications")
            .unwrap()
            .authorization,
        &ConsoleAuthorization::Simple
    );
    for (authorization_profile_id, action_code) in [
        ("applications.view", "view"),
        ("applications.update", "update"),
        ("applications.delete", "delete"),
    ] {
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| {
                operation.authorization_profile_id == authorization_profile_id
                    && operation.authorization
                        == ConsoleAuthorization::ResourceAction {
                            resource_code: "applications".to_string(),
                            action_code: action_code.to_string(),
                        }
            })
            .unwrap();
        assert_eq!(
            operation.authorization,
            ConsoleAuthorization::ResourceAction {
                resource_code: "applications".to_string(),
                action_code: action_code.to_string(),
            }
        );
    }

    let resource = registry
        .inventory()
        .resources
        .iter()
        .find(|resource| resource.resource_code == "applications")
        .unwrap();
    assert_eq!(resource.scope_kind, ResourceAccessScopeKind::Workspace);
    assert_eq!(resource.identity_field, "id");
    assert_eq!(resource.scope_field.as_deref(), Some("scope_id"));
    assert_eq!(resource.owner_field.as_deref(), Some("created_by"));
    assert_eq!(
        resource
            .actions
            .iter()
            .map(|action| action.action_code.as_str())
            .collect::<Vec<_>>(),
        vec!["create", "delete", "update", "view"]
    );
    assert_eq!(resource.label_ref, "Applications");
    assert_eq!(
        resource
            .actions
            .iter()
            .map(|action| (action.action_code.as_str(), action.label_ref.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("create", "Create"),
            ("delete", "Delete"),
            ("update", "Update"),
            ("view", "View"),
        ]
    );
}

#[test]
fn applications_closed_set_rejects_duplicate_or_missing_binding() {
    let settings = compile_core_settings_feature_registry().unwrap();
    let assembly = migrated_core_console_route_assembly();
    let application_bindings = assembly
        .bindings()
        .iter()
        .filter(|binding| binding.route.path.starts_with("/api/console/applications"))
        .cloned()
        .collect::<Vec<_>>();

    let mut duplicate = assembly.bindings().to_vec();
    duplicate.push(application_bindings[0].clone());
    let duplicate_error =
        compile_migrated_core_console_operation_registry(&settings, &duplicate).unwrap_err();
    assert!(duplicate_error.to_string().contains("duplicate"));

    let missing = assembly
        .bindings()
        .iter()
        .filter(|binding| {
            binding.ownership
                != ConsoleRouteOwnership::ConsoleOperation(
                    "applications.orchestration.template.export".to_string(),
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    let error = compile_migrated_core_console_operation_registry(&settings, &missing).unwrap_err();
    assert!(error.to_string().contains(
        "operation applications.orchestration.template.export must own at least one console route"
    ));
}

#[test]
fn application_api_orchestration_runtime_routes_compile_exact_operations() {
    let assembly = crate::routes::application_api::route_assembly()
        .merge(crate::routes::application_orchestration::route_assembly())
        .merge(crate::routes::application_runtime::route_assembly());

    let actual = assembly
        .bindings()
        .iter()
        .map(|binding| {
            (
                binding.route.method.as_str(),
                binding.route.path.as_str(),
                match &binding.ownership {
                    ConsoleRouteOwnership::ConsoleOperation(operation_id) => operation_id.as_str(),
                    ConsoleRouteOwnership::Authenticated => "authenticated",
                },
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                "GET",
                "/api/console/applications/:application_id/api-keys",
                "applications.view",
            ),
            (
                "POST",
                "/api/console/applications/:application_id/api-keys",
                "applications.update",
            ),
            (
                "DELETE",
                "/api/console/applications/:application_id/api-keys/:key_id",
                "applications.update",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/api-mapping",
                "applications.view",
            ),
            (
                "PUT",
                "/api/console/applications/:application_id/api-mapping",
                "applications.update",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/api-publication",
                "applications.view",
            ),
            (
                "DELETE",
                "/api/console/applications/:application_id/api-publication",
                "applications.publish",
            ),
            (
                "POST",
                "/api/console/applications/:application_id/api-publications",
                "applications.publish",
            ),
            (
                "PATCH",
                "/api/console/applications/:application_id/api-status",
                "applications.api.set_enabled",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/workflow-schedule-trigger",
                "applications.view",
            ),
            (
                "PUT",
                "/api/console/applications/:application_id/workflow-schedule-trigger",
                "applications.update",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/api-docs/catalog",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/api-docs/categories/:category_id/operations",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/api-docs/categories/:category_id/openapi.json",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:application_id/api-docs/operations/:operation_id/openapi.json",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/orchestration",
                "applications.view",
            ),
            (
                "PUT",
                "/api/console/applications/:id/orchestration/draft",
                "applications.update",
            ),
            (
                "POST",
                "/api/console/applications/archive/export",
                "applications.orchestration.template.export",
            ),
            (
                "POST",
                "/api/console/applications/archive/preview",
                "authenticated",
            ),
            (
                "POST",
                "/api/console/applications/archive/import",
                "applications.orchestration.template.import",
            ),
            (
                "GET",
                "/api/console/applications/archive/installed-extension/:installation_id/preview",
                "authenticated",
            ),
            (
                "POST",
                "/api/console/applications/archive/installed-extension/:installation_id/import",
                "authenticated",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/versions/:version_id/restore",
                "applications.orchestration.version.restore",
            ),
            (
                "PATCH",
                "/api/console/applications/:id/orchestration/versions/:version_id",
                "applications.update",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/debug-runs",
                "applications.run",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/debug-runs/stream",
                "applications.run",
            ),
            (
                "GET",
                "/api/console/applications/:id/orchestration/runs/:run_id/debug-stream",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/orchestration/runs/:run_id/debug-snapshot",
                "applications.view",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/runs/:run_id/resume",
                "applications.run",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/runs/:run_id/cancel",
                "applications.run",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/callback-tasks/:callback_task_id/complete",
                "applications.run",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/nodes/:node_id/debug-runs",
                "applications.run",
            ),
            (
                "GET",
                "/api/console/applications/:id/orchestration/debug-variable-snapshot",
                "applications.view",
            ),
            (
                "PUT",
                "/api/console/applications/:id/orchestration/debug-variable-cache",
                "applications.update",
            ),
            (
                "DELETE",
                "/api/console/applications/:id/orchestration/debug-variable-cache",
                "applications.update",
            ),
            (
                "POST",
                "/api/console/applications/:id/orchestration/debug-artifacts/resolve",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/orchestration/debug-artifacts/:artifact_id",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs",
                "applications.view",
            ),
            (
                "POST",
                "/api/console/applications/:id/logs/runs/export",
                "applications.logs.export",
            ),
            (
                "POST",
                "/api/console/applications/:id/logs/runs/archive",
                "applications.logs.export",
            ),
            (
                "POST",
                "/api/console/applications/:id/logs/runs/archive/import-sessions",
                "applications.logs.import",
            ),
            (
                "PUT",
                "/api/console/applications/:id/logs/runs/archive/import-sessions/:session_id/chunks/:chunk_index",
                "applications.logs.import",
            ),
            (
                "POST",
                "/api/console/applications/:id/logs/runs/archive/import-sessions/:session_id/complete",
                "applications.logs.import",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/archive/import-jobs/:job_id",
                "applications.logs.import",
            ),
            (
                "GET",
                "/api/console/applications/:id/monitoring/run-metrics",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/monitoring/runtime-activity",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/conversations/:conversation_id/messages",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/conversation/messages",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/overview",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/trace-tree",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/export",
                "applications.logs.export",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/archive",
                "applications.logs.export",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/content",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/details/:detail_ref_id",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes/:trace_node_id/tool-callbacks/:tool_call_id/content",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/resume-timeline",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/nodes/:node_id",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/logs/runs/:run_id/debug-stream",
                "applications.view",
            ),
            (
                "GET",
                "/api/console/applications/:id/orchestration/nodes/:node_id/last-run",
                "applications.view",
            ),
        ]
    );

    let settings = compile_core_settings_feature_registry().unwrap();
    let migrated = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, migrated.bindings()).unwrap();

    for authorization_profile_id in [
        "applications.publish",
        "applications.api.set_enabled",
        "applications.orchestration.template.export",
        "applications.orchestration.template.import",
        "applications.orchestration.version.restore",
        "applications.run",
        "applications.logs.export",
        "applications.logs.import",
    ] {
        let operations = registry
            .inventory()
            .operations
            .iter()
            .filter(|operation| operation.authorization_profile_id == authorization_profile_id)
            .collect::<Vec<_>>();
        assert!(!operations.is_empty());
        assert!(operations.iter().all(|operation| {
            operation.authorization == ConsoleAuthorization::Simple
                && operation.policy_group
                    == ConsolePolicyGroup::SettingsFeature("system.applications".to_string())
                && operation.routes.len() == 1
        }));
    }
}

mod data_model_and_docs;
mod infrastructure_and_mcp;
mod plugins_and_models;
