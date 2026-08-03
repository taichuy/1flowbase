use super::*;

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
                "GET",
                "/api/console/data-sources/agent-flow-options",
                "agent_flow.data_source_options.list",
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
        .find(|operation| {
            operation.authorization_profile_id == "data_sources.view"
                && matches!(
                    operation.authorization,
                    ConsoleAuthorization::ResourceAction { .. }
                )
        })
        .unwrap();
    assert_eq!(
        view.authorization,
        ConsoleAuthorization::ResourceAction {
            resource_code: "data_source_instances".to_string(),
            action_code: "view".to_string(),
        }
    );
    for authorization_profile_id in [
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
        let operations = registry
            .inventory()
            .operations
            .iter()
            .filter(|operation| operation.authorization_profile_id == authorization_profile_id)
            .collect::<Vec<_>>();
        assert!(!operations.is_empty(), "{authorization_profile_id}");
        assert!(operations
            .iter()
            .all(|operation| operation.authorization == ConsoleAuthorization::Simple));
    }

    let secret_rotate = registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.authorization_profile_id == "data_sources.secret.rotate")
        .unwrap();
    assert_eq!(
        secret_rotate.policy_group,
        ConsolePolicyGroup::Other("other.data-sources".to_string())
    );
    assert_eq!(secret_rotate.routes.len(), 1);
}
