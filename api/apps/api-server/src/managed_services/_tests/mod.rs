use super::*;
use serde_json::json;
fn fixture() -> ManagedServiceRegistration {
    ManagedServiceRegistration {
        display_name: "Fixture plugin".into(), description: "Fixture plugin operations".into(),
        installation_id: uuid::Uuid::nil(), plugin_code:"fixture".into(), plugin_version:"1.0.0".into(),
        declaration:serde_json::from_value(json!({"scope":"system","feature":{"feature_id":"fixture.settings","label":"Fixture service","description":"Configure fixture service","route_id":"fixture","path":"/settings/fixture"},"operations":[{
            "interface_id":"fixture.list","contribution_id":"fixture.list","method":"GET","path":"/api/console/managed-services/fixture/items","summary":"List fixture items","description":"List shared fixture items.","input_schema":{"type":"object","properties":{"path":{"type":"object"},"query":{"type":"object"},"body":{"type":"object"}}},"output_schema":{"type":"object"}
        }]})).unwrap(),
        pages:vec![plugin_framework::PluginSettingsPageManifest { code_modules: Vec::new(),
feature_id:"fixture.settings".into(), contribution_code:"settings".into(), source_file:"ui/Settings.tsx".into(), language:"tsx".into() }],
    }
}
#[test]
fn service_boot_registry_registers_one_operation_per_route_and_managed_page() {
    let service = fixture();
    let plan = crate::app_state::compile_console_boot_plan_with_managed_services(
        Vec::new(),
        None,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
        vec![service],
    )
    .unwrap();
    let operation = plan
        .console_operation_registry
        .inventory()
        .operations
        .iter()
        .find(|op| op.authorization_profile_id == "fixture.list")
        .unwrap();
    assert_eq!(
        operation.owner.kind,
        access_control::SettingsFeatureOwnerKind::ManagedService
    );
    assert_eq!(
        operation.policy_group,
        access_control::ConsolePolicyGroup::Plugin("fixture".into())
    );
    assert_eq!(operation.routes.len(), 1);
    assert_eq!(
        operation.routes[0].path,
        "/api/console/managed-services/fixture/items"
    );
    assert!(plan
        .route_assembly
        .bindings()
        .iter()
        .any(|binding| binding.route.path == operation.routes[0].path));
    assert_eq!(
        plan.console_surface_registry
            .page("fixture")
            .unwrap()
            .plugin_code,
        "fixture"
    );
    assert_eq!(
        plan.console_operation_registry
            .inventory()
            .interfaces
            .iter()
            .find(|i| i.interface_id == operation.operation_id)
            .unwrap()
            .summary,
        "List fixture items"
    );
}
#[test]
fn service_duplicate_route_rejects_whole_boot_snapshot() {
    let service = fixture();
    assert!(
        crate::app_state::compile_console_boot_plan_with_managed_services(
            Vec::new(),
            None,
            crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
            vec![service.clone(), service]
        )
        .is_err()
    );
}
#[test]
fn service_openapi_projects_real_path_query_body_and_result_contract() {
    let mut doc = json!({"paths":{}});
    let mut service = fixture();
    service.declaration.operations[0].output_schema = json!({
        "type":"object", "properties":{"items":{"type":"array","items":{"type":"string"}}},
        "required":["items"], "additionalProperties":false
    });
    append_openapi(&mut doc, &[service.clone()]);
    let operation = &doc["paths"]["/api/console/managed-services/fixture/items"]["get"];
    assert_eq!(operation["operationId"], "fixture.list");
    assert_eq!(
        operation["responses"]["200"]["content"]["application/json"]["schema"],
        service.declaration.operations[0].output_schema
    );
    let catalog = crate::openapi_interface::catalog_entry_from_operation(
        &crate::openapi_docs::DocsCatalogOperation {
            id: "fixture.list".into(),
            method: "GET".into(),
            path: "/api/console/managed-services/fixture/items".into(),
            summary: None,
            description: None,
            tags: vec![],
            group: "fixture".into(),
            deprecated: false,
        },
        &doc,
    )
    .unwrap();
    let validator = jsonschema::validator_for(&catalog.response_schema).unwrap();
    assert!(validator.is_valid(&json!({"items":["fixture"]})));
    assert!(!validator.is_valid(&json!({"data":{"items":["fixture"]}})));
}
#[test]
fn business_failures_expose_only_classification_codes() {
    use runtime_core::runtime_backend::RuntimeBackendError;
    let safe = anyhow::Error::new(RuntimeBackendError::from(
        extension_contracts::error::ExtensionContractError::invalid_provider_package(
            "host_key_changed",
        ),
    ));
    assert_eq!(
        ManagedServiceFailure::from_runtime(&safe).unwrap().0,
        "host_key_changed"
    );
    let raw = anyhow::Error::new(RuntimeBackendError::from(
        extension_contracts::error::ExtensionContractError::invalid_provider_package(
            "password=secret",
        ),
    ));
    assert!(ManagedServiceFailure::from_runtime(&raw).is_none());
}

mod hot_generation;

mod stream_ticket;

#[test]
fn duplex_upgrade_is_protocol_metadata_and_keeps_exact_original_post_permission() {
    let mut service = fixture();
    let op = &mut service.declaration.operations[0];
    op.method = "POST".into();
    op.stream = Some(plugin_framework::ManagedServiceStream {
        input_schema: json!({"type":"object"}),
        event_schema: json!({"type":"object"}),
    });
    let plan = crate::app_state::compile_console_boot_plan_with_managed_services(
        Vec::new(),
        None,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
        vec![service.clone()],
    )
    .unwrap();
    let operation = plan
        .console_operation_registry
        .inventory()
        .operations
        .iter()
        .find(|op| op.authorization_profile_id == "fixture.list")
        .unwrap();
    assert_eq!(operation.routes.len(), 1);
    assert_eq!(operation.routes[0].method, "POST");
    let metadata = service.metadata();
    assert!(metadata
        .iter()
        .any(|m| m.interface_id == "fixture.list.stream-upgrade" && m.route.method == "GET"));
    assert!(plan
        .route_assembly
        .bindings()
        .iter()
        .any(|b| b.route.method == "GET"
            && b.route.path == operation.routes[0].path
            && matches!(
                b.ownership,
                access_control::ConsoleRouteOwnership::Authenticated
            )));
    let mut document = json!({"paths":{}});
    append_openapi(&mut document, &[service]);
    assert_eq!(
        document["paths"][&operation.routes[0].path]["get"]["operationId"],
        "fixture.list.stream-upgrade"
    );
}

#[test]
fn parallel_operation_keeps_post_authorization_and_route() {
    let mut service = fixture();
    let op = &mut service.declaration.operations[0];
    op.method = "POST".into();
    op.execution_concurrency = plugin_framework::ManagedServiceExecutionConcurrency::Parallel;
    assert!(!op.requires_mutation_lock());
    let plan = crate::app_state::compile_console_boot_plan_with_managed_services(
        Vec::new(),
        None,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
        vec![service],
    )
    .unwrap();
    let operation = plan
        .console_operation_registry
        .inventory()
        .operations
        .iter()
        .find(|op| op.authorization_profile_id == "fixture.list")
        .unwrap();
    assert_eq!(operation.routes.len(), 1);
    assert_eq!(operation.routes[0].method, "POST");
    assert!(plan
        .route_assembly
        .bindings()
        .iter()
        .any(|b| b.route.method == "POST" && b.route.path == operation.routes[0].path));
}

#[test]
fn backend_only_plugin_has_permissions_without_a_settings_surface() {
    for legacy_feature in [true, false] {
        let mut service = fixture();
        service.pages.clear();
        if !legacy_feature {
            service.declaration.feature = None;
        }
        let plan = crate::app_state::compile_console_boot_plan_with_managed_services(
            Vec::new(),
            None,
            crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
            vec![service],
        )
        .unwrap();
        let access = plan
            .console_operation_registry
            .access_for_console_route("GET", "/api/console/managed-services/fixture/items")
            .unwrap();
        assert_eq!(
            access.policy_group,
            &access_control::ConsolePolicyGroup::Plugin("fixture".into())
        );
        assert!(plan.console_surface_registry.page("fixture").is_none());
        assert!(!plan
            .settings_feature_registry
            .inventory()
            .features
            .iter()
            .any(|f| f.owner.owner_id == "fixture"));
        assert!(plan
            .console_operation_registry
            .inventory()
            .locale_catalog
            .as_ref()
            .unwrap()
            .policy_group_display(access.policy_group, "en_US")
            .is_ok());
    }
}

#[test]
fn plugin_settings_navigation_and_api_use_the_same_canonical_grant() {
    use domain::{
        ConsoleOperationId, ConsoleOperationPolicy, ConsolePolicyGroup, ConsolePolicyStrategy,
        RoleConsoleGroupPolicy, RoleConsolePolicy,
    };
    let plan = crate::app_state::compile_console_boot_plan_with_managed_services(
        Vec::new(),
        None,
        crate::config::DEFAULT_PLUGIN_UPLOAD_MAX_BYTES,
        vec![fixture()],
    )
    .unwrap();
    let mut actor = domain::ActorContext::root(uuid::Uuid::nil(), uuid::Uuid::nil(), "root");
    actor.is_root = false;
    let access = plan
        .console_operation_registry
        .access_for_console_route("GET", "/api/console/managed-services/fixture/items")
        .unwrap();
    for (group, enabled, operation_enabled, expected) in [
        (
            ConsolePolicyGroup::plugin("fixture").unwrap(),
            true,
            true,
            true,
        ),
        (
            ConsolePolicyGroup::plugin("fixture").unwrap(),
            true,
            false,
            false,
        ),
        (
            ConsolePolicyGroup::plugin("fixture").unwrap(),
            false,
            true,
            false,
        ),
        (
            ConsolePolicyGroup::settings_feature("fixture.settings").unwrap(),
            true,
            true,
            false,
        ),
        (
            ConsolePolicyGroup::plugin("another-plugin").unwrap(),
            true,
            true,
            false,
        ),
    ] {
        let policies = [RoleConsolePolicy::new(
            uuid::Uuid::nil(),
            vec![RoleConsoleGroupPolicy::new(
                group,
                enabled,
                ConsolePolicyStrategy::Custom,
                vec![ConsoleOperationPolicy::simple(
                    ConsoleOperationId::try_from("fixture.list").unwrap(),
                    operation_enabled,
                )],
            )],
        )];
        let permissions =
            crate::middleware::require_session::settings_feature_navigation_permissions(
                plan.console_operation_registry.inventory(),
                &policies,
                &plan.settings_feature_registry.inventory().features,
            );
        assert_eq!(
            permissions.contains("settings_feature.access.fixture.settings"),
            expected
        );
        assert_eq!(crate::middleware::require_settings_feature_permission::authorize_compiled_console_access(&access, &actor, &policies), expected);
    }
}
