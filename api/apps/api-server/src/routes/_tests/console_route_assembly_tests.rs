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
    let settings = compile_core_settings_feature_registry().unwrap();
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
    let settings = compile_core_settings_feature_registry().unwrap();
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

    registry
        .validate_console_route_coverage(assembly.bindings().iter().cloned())
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
    for (method, path, operation_id) in critical_routes {
        let access = registry.access_for_console_route(method, path).unwrap();
        assert_eq!(access.operation_id, operation_id);
    }

    assert_eq!(
        registry
            .access_for_console_route("POST", "/api/console/applications")
            .unwrap()
            .authorization,
        &ConsoleAuthorization::Simple
    );
    for (operation_id, action_code) in [
        ("applications.view", "view"),
        ("applications.update", "update"),
        ("applications.delete", "delete"),
    ] {
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
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
    assert_eq!(resource.label_ref, "console.resources.applications.label");
    assert!(resource.actions.iter().all(|action| {
        action.label_ref
            == format!(
                "console.resources.applications.actions.{}.label",
                action.action_code
            )
    }));
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
    assert!(
        compile_migrated_core_console_operation_registry(&settings, &duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate console route ownership")
    );

    let missing = assembly
        .bindings()
        .iter()
        .filter(|binding| {
            !(binding.route.method == "DELETE"
                && binding.route.path == "/api/console/applications/:id")
        })
        .cloned()
        .collect::<Vec<_>>();
    let error = compile_migrated_core_console_operation_registry(&settings, &missing).unwrap_err();
    assert!(error
        .to_string()
        .contains("operation applications.delete must own at least one console route"));
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
                "applications.view",
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
                "GET",
                "/api/console/applications/:id/orchestration/template",
                "applications.orchestration.template.export",
            ),
            (
                "POST",
                "/api/console/applications/orchestration/template/preview",
                "authenticated",
            ),
            (
                "POST",
                "/api/console/applications/orchestration/template/import",
                "applications.orchestration.template.import",
            ),
            (
                "GET",
                "/api/console/applications/orchestration/templates/official-catalog",
                "authenticated",
            ),
            (
                "GET",
                "/api/console/applications/orchestration/templates/official/:workflow_id",
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

    for operation_id in [
        "applications.publish",
        "applications.api.set_enabled",
        "applications.orchestration.template.export",
        "applications.orchestration.template.import",
        "applications.orchestration.version.restore",
        "applications.run",
        "applications.logs.export",
        "applications.logs.import",
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
            ConsolePolicyGroup::SettingsFeature("system.applications".to_string())
        );
        assert!(!operation.routes.is_empty());
    }
}

#[test]
fn data_model_docs_and_data_source_routes_compile_exact_operations() {
    let assembly = crate::routes::data_models::route_assembly()
        .merge(crate::routes::docs::route_assembly())
        .merge(crate::routes::data_sources::route_assembly());

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
                "/api/console/settings/data-models/data-sources/catalog",
                "settings_feature.access.system.data-models",
            ),
            (
                "GET",
                "/api/console/settings/data-models/data-sources",
                "data_sources.list",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources",
                "data_sources.create",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/data-sources/:data_source_id/defaults",
                "data_sources.defaults.update",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/:data_source_id/validate",
                "data_sources.validate",
            ),
            (
                "GET",
                "/api/console/settings/data-models/data-sources/:data_source_id/resources",
                "data_sources.view",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/:data_source_id/resources/discover",
                "data_sources.discover",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/:data_source_id/preview-read",
                "data_sources.preview",
            ),
            (
                "POST",
                "/api/console/settings/data-models/data-sources/:data_source_id/resources/map-to-model",
                "data_sources.map_to_model",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions",
                "model_definitions.list",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions",
                "model_definitions.create",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions:batchDelete",
                "model_definitions.delete",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/model-definitions/:id",
                "model_definitions.update",
            ),
            (
                "DELETE",
                "/api/console/settings/data-models/model-definitions/:id",
                "model_definitions.delete",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions/:id/advisor-findings",
                "model_definitions.advisor.view",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions/:id/fields",
                "model_fields.create",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/model-definitions/:id/fields/:field_id",
                "model_fields.update",
            ),
            (
                "DELETE",
                "/api/console/settings/data-models/model-definitions/:id/fields/:field_id",
                "model_fields.delete",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions/:id/scope-grants",
                "model_scope_grants.list",
            ),
            (
                "POST",
                "/api/console/settings/data-models/model-definitions/:id/scope-grants",
                "model_scope_grants.create",
            ),
            (
                "PATCH",
                "/api/console/settings/data-models/model-definitions/:id/scope-grants/:grant_id",
                "model_scope_grants.update",
            ),
            (
                "GET",
                "/api/console/settings/data-models/model-definitions/:model_id/openapi.json",
                "model_definitions.openapi.view",
            ),
            (
                "GET",
                "/api/console/docs/catalog",
                "settings_feature.access.system.docs",
            ),
            (
                "GET",
                "/api/console/docs/categories/:category_id/operations",
                "settings_feature.access.system.docs",
            ),
            (
                "GET",
                "/api/console/docs/categories/:category_id/openapi.json",
                "settings_feature.access.system.docs",
            ),
            (
                "GET",
                "/api/console/docs/operations/:operation_id/openapi.json",
                "settings_feature.access.system.docs",
            ),
            (
                "POST",
                "/api/console/data-sources/:data_source_id/secret/rotate",
                "data_sources.secret.rotate",
            ),
        ]
    );
    assert!(assembly
        .bindings()
        .iter()
        .all(|binding| { binding.ownership != ConsoleRouteOwnership::Authenticated }));

    let settings = compile_core_settings_feature_registry().unwrap();
    let migrated = migrated_core_console_route_assembly();
    let registry =
        compile_migrated_core_console_operation_registry(&settings, migrated.bindings()).unwrap();
    let resource = registry
        .inventory()
        .resources
        .iter()
        .find(|resource| resource.resource_code == "data_source_instances")
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
        vec!["view"]
    );

    let view = registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == "data_sources.view")
        .unwrap();
    assert_eq!(
        view.authorization,
        ConsoleAuthorization::ResourceAction {
            resource_code: "data_source_instances".to_string(),
            action_code: "view".to_string(),
        }
    );
    for operation_id in [
        "data_sources.list",
        "data_sources.create",
        "data_sources.defaults.update",
        "data_sources.validate",
        "data_sources.discover",
        "data_sources.preview",
        "data_sources.map_to_model",
        "model_definitions.list",
        "model_definitions.create",
        "model_definitions.update",
        "model_definitions.delete",
        "model_definitions.advisor.view",
        "model_fields.create",
        "model_fields.update",
        "model_fields.delete",
        "model_scope_grants.list",
        "model_scope_grants.create",
        "model_scope_grants.update",
        "model_definitions.openapi.view",
        "data_sources.secret.rotate",
    ] {
        let operation = registry
            .inventory()
            .operations
            .iter()
            .find(|operation| operation.operation_id == operation_id)
            .unwrap();
        assert_eq!(operation.authorization, ConsoleAuthorization::Simple);
    }

    let secret_rotate = registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == "data_sources.secret.rotate")
        .unwrap();
    assert_eq!(
        secret_rotate.policy_group,
        ConsolePolicyGroup::Other("other.data-sources".to_string())
    );
    assert_eq!(secret_rotate.routes.len(), 1);
}

#[test]
fn infrastructure_mcp_and_user_api_key_routes_compile_exact_operations() {
    let host = crate::routes::host_infrastructure::route_assembly();
    let mcp = crate::routes::mcp_management::route_assembly();
    let user_api_keys = crate::routes::user_api_keys::route_assembly();

    assert_eq!(
        route_bindings(&host),
        vec![
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/stats",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/stats",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries/search",
                "host_infrastructure.memory.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/tree",
                "host_infrastructure.memory.view"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/memory/contracts/:contract_code/entries/reveal",
                "host_infrastructure.memory.reveal"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/cache",
                "host_infrastructure.cache.view"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries",
                "host_infrastructure.cache.view"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries/reveal",
                "host_infrastructure.cache.reveal"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/entries/clear",
                "host_infrastructure.cache.entry.clear"
            ),
            (
                "POST",
                "/api/console/settings/host-infrastructure/cache/domains/:domain_code/clear",
                "host_infrastructure.cache.domain.clear"
            ),
            (
                "GET",
                "/api/console/settings/host-infrastructure/providers",
                "host_infrastructure.providers.view"
            ),
            (
                "PUT",
                "/api/console/settings/host-infrastructure/providers/:installation_id/:provider_code/config",
                "host_infrastructure.providers.configure"
            ),
        ]
    );
    assert_eq!(
        route_bindings(&mcp),
        vec![
            ("GET", "/api/console/mcp/catalog", "mcp.catalog.view"),
            (
                "GET",
                "/api/console/mcp/interface-capabilities",
                "mcp.catalog.view"
            ),
            ("GET", "/api/console/mcp/list", "mcp.catalog.view"),
            ("GET", "/api/console/mcp/export", "mcp.catalog.export"),
            ("GET", "/api/console/mcp/instances", "mcp.instances.view"),
            ("POST", "/api/console/mcp/instances", "mcp.instances.create"),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/copy",
                "mcp.instances.copy"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id",
                "mcp.instances.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/instances/:instance_id",
                "mcp.instances.delete"
            ),
            (
                "GET",
                "/api/console/mcp/instances/:instance_id/client-credential",
                "mcp.client_credential.reveal"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id/client-credential",
                "mcp.client_credential.save"
            ),
            (
                "DELETE",
                "/api/console/mcp/instances/:instance_id/client-credential",
                "mcp.client_credential.delete"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/groups",
                "mcp.groups.upsert"
            ),
            (
                "DELETE",
                "/api/console/mcp/instances/:instance_id/groups",
                "mcp.groups.delete"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/groups/move",
                "mcp.groups.move"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/tool-bindings",
                "mcp.tool_bindings.create"
            ),
            (
                "PUT",
                "/api/console/mcp/tool-bindings/:binding_id",
                "mcp.tool_bindings.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/tool-bindings/:binding_id",
                "mcp.tool_bindings.delete"
            ),
            ("GET", "/api/console/mcp/tools", "mcp.tools.view"),
            ("POST", "/api/console/mcp/tools", "mcp.tools.create"),
            ("GET", "/api/console/mcp/tools/:tool_id", "mcp.tools.view"),
            ("PUT", "/api/console/mcp/tools/:tool_id", "mcp.tools.update"),
            (
                "DELETE",
                "/api/console/mcp/tools/:tool_id",
                "mcp.tools.delete"
            ),
            (
                "POST",
                "/api/console/mcp/tools/:tool_id/description/refresh",
                "mcp.tools.description.refresh"
            ),
            (
                "POST",
                "/api/console/mcp/tools/:tool_id/description-check",
                "mcp.tools.description.check"
            ),
            (
                "POST",
                "/api/console/mcp/debug/execute",
                "mcp.debug.execute"
            ),
            (
                "GET",
                "/api/console/mcp/instances/:instance_id/discovery-policy",
                "mcp.discovery_policy.view"
            ),
            (
                "PUT",
                "/api/console/mcp/instances/:instance_id/discovery-policy",
                "mcp.discovery_policy.update"
            ),
            (
                "GET",
                "/api/console/mcp/bundles/official",
                "mcp.bundles.official.list"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/preview-official",
                "mcp.bundles.preview"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/import-official",
                "mcp.bundles.import"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/export",
                "mcp.bundles.export"
            ),
            (
                "GET",
                "/api/console/mcp/bundles/export-defaults",
                "mcp.bundles.export"
            ),
            (
                "POST",
                "/api/console/mcp/instances/:instance_id/bundles/export",
                "mcp.instances.export"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/preview-upload",
                "mcp.bundles.preview"
            ),
            (
                "POST",
                "/api/console/mcp/bundles/import-upload",
                "mcp.bundles.import"
            ),
            (
                "GET",
                "/api/console/mcp/upstream-connections",
                "mcp.upstream_connections.view"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections",
                "mcp.upstream_connections.create"
            ),
            (
                "PUT",
                "/api/console/mcp/upstream-connections/:connection_id",
                "mcp.upstream_connections.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/upstream-connections/:connection_id",
                "mcp.upstream_connections.delete"
            ),
            (
                "PUT",
                "/api/console/mcp/upstream-connections/:connection_id/credentials",
                "mcp.upstream_credentials.update"
            ),
            (
                "DELETE",
                "/api/console/mcp/upstream-connections/:connection_id/credentials",
                "mcp.upstream_credentials.delete"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/test",
                "mcp.upstream_connections.test"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/:connection_id/test",
                "mcp.upstream_connections.test"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/:connection_id/discover",
                "mcp.upstream_connections.discover"
            ),
            (
                "POST",
                "/api/console/mcp/upstream-connections/:connection_id/imports",
                "mcp.upstream_tools.import"
            ),
            (
                "POST",
                "/api/console/mcp/tools/:tool_id/debug",
                "mcp.upstream_tools.debug"
            ),
        ]
    );
    assert_eq!(
        route_bindings(&user_api_keys),
        vec![
            ("GET", "/api/console/user-api-keys", "user_api_keys.manage"),
            ("POST", "/api/console/user-api-keys", "user_api_keys.manage"),
            (
                "GET",
                "/api/console/user-api-keys/role-options",
                "user_api_keys.manage"
            ),
            (
                "POST",
                "/api/console/user-api-keys/:api_key_id/revoke",
                "user_api_keys.manage"
            ),
        ]
    );
}

#[test]
fn ac_002_ac_013_plugins_and_models_owner_routes_have_explicit_assembly_ownership() {
    let assembly = crate::routes::frontend_block_catalog::route_assembly()
        .merge(crate::routes::js_dependencies::route_assembly())
        .merge(crate::routes::model_definitions::route_assembly())
        .merge(crate::routes::model_providers::route_assembly())
        .merge(crate::routes::node_contributions::route_assembly())
        .merge(crate::routes::plugins::route_assembly());

    assert_eq!(
        route_bindings(&assembly),
        vec![
            (
                "GET",
                "/api/console/frontend-blocks",
                "frontend_blocks.view"
            ),
            (
                "GET",
                "/api/console/js-dependencies",
                "js_dependencies.view"
            ),
            (
                "GET",
                "/api/console/models/agent-flow-options",
                "agent_flow.data_model_options.list"
            ),
            (
                "GET",
                "/api/console/model-providers/providers/:provider_code/icon",
                "model_providers.icons.view"
            ),
            (
                "GET",
                "/api/console/model-providers/options",
                "model_providers.options.view"
            ),
            (
                "GET",
                "/api/console/model-providers/:id/balance",
                "model_providers.balance.view"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/catalog",
                "model_providers.catalog.view"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/request-logs",
                "model_providers.request_logs.view"
            ),
            (
                "DELETE",
                "/api/console/settings/model-providers/request-logs",
                "model_providers.request_logs.delete"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/request-logs/clear",
                "model_providers.request_logs.clear"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/instances",
                "model_providers.instances.view"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/instances",
                "model_providers.instances.create"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/providers/:provider_code/main-instance",
                "model_providers.main_instance.view"
            ),
            (
                "PUT",
                "/api/console/settings/model-providers/providers/:provider_code/main-instance",
                "model_providers.main_instance.update"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/preview-models",
                "model_providers.preview.view"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/options",
                "model_providers.settings_options.view"
            ),
            (
                "PATCH",
                "/api/console/settings/model-providers/instances/:id",
                "model_providers.instances.update"
            ),
            (
                "DELETE",
                "/api/console/settings/model-providers/instances/:id",
                "model_providers.instances.delete"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/instances/:id/validate",
                "model_providers.instances.validate"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/instances/:id/secrets/reveal",
                "model_providers.instances.secrets.reveal"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/instances/:id/models",
                "model_providers.instances.models.view"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/instances/:id/models/refresh",
                "model_providers.instances.models.refresh"
            ),
            (
                "GET",
                "/api/console/node-contributions",
                "node_contributions.view"
            ),
            (
                "GET",
                "/api/console/plugins/catalog",
                "plugins.catalog.view"
            ),
            (
                "GET",
                "/api/console/plugins/families",
                "plugins.families.view"
            ),
            (
                "POST",
                "/api/console/plugins/families/:provider_code/upgrade-latest",
                "plugins.families.upgrade"
            ),
            (
                "POST",
                "/api/console/plugins/families/:provider_code/switch-version",
                "plugins.families.switch"
            ),
            (
                "DELETE",
                "/api/console/plugins/families/:provider_code",
                "plugins.families.delete"
            ),
            (
                "GET",
                "/api/console/plugins/official-catalog",
                "plugins.official_catalog.view"
            ),
            (
                "POST",
                "/api/console/plugins/install-upload",
                "plugins.install.upload"
            ),
            ("POST", "/api/console/plugins/install", "plugins.install"),
            (
                "POST",
                "/api/console/plugins/install-official",
                "plugins.install.official"
            ),
            (
                "POST",
                "/api/console/plugins/:installation_id/catalog-projection/refresh",
                "plugins.catalog_projection.refresh"
            ),
            (
                "POST",
                "/api/console/plugins/:installation_id/artifact/refresh",
                "plugins.artifact.refresh"
            ),
            (
                "POST",
                "/api/console/plugins/:installation_id/artifact/install-current-node",
                "plugins.artifact.install"
            ),
            (
                "POST",
                "/api/console/plugins/:installation_id/enable",
                "plugins.enable"
            ),
            (
                "POST",
                "/api/console/plugins/:installation_id/assign",
                "plugins.assign"
            ),
            ("GET", "/api/console/plugins/tasks", "plugins.tasks.view"),
            (
                "GET",
                "/api/console/plugins/tasks/:task_id",
                "plugins.tasks.view"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/plugins/families",
                "model_provider_plugins.families.view"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/plugins/official-catalog",
                "model_provider_plugins.official_catalog.view"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/install-official",
                "model_provider_plugins.install.official"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/install-upload",
                "model_provider_plugins.install.upload"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/:installation_id/artifact/refresh",
                "model_provider_plugins.artifact.refresh"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/:installation_id/artifact/install-current-node",
                "model_provider_plugins.artifact.install"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/families/:provider_code/upgrade-latest",
                "model_provider_plugins.families.upgrade"
            ),
            (
                "POST",
                "/api/console/settings/model-providers/plugins/families/:provider_code/switch-version",
                "model_provider_plugins.families.switch"
            ),
            (
                "DELETE",
                "/api/console/settings/model-providers/plugins/families/:provider_code",
                "model_provider_plugins.families.delete"
            ),
            (
                "GET",
                "/api/console/settings/model-providers/plugins/tasks/:task_id",
                "model_provider_plugins.tasks.view"
            ),
        ]
    );
}
