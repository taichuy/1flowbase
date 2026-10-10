use super::*;
use serde_json::json;
fn fixture() -> ManagedServiceRegistration {
    ManagedServiceRegistration {
        installation_id: uuid::Uuid::nil(), plugin_code:"fixture".into(), plugin_version:"1.0.0".into(),
        declaration:serde_json::from_value(json!({"scope":"system","feature":{"feature_id":"fixture.settings","label":"Fixture service","description":"Configure fixture service","route_id":"fixture","path":"/settings/fixture"},"operations":[{
            "interface_id":"fixture.list","contribution_id":"fixture.list","method":"GET","path":"/api/console/managed-services/fixture/items","summary":"List fixture items","description":"List shared fixture items.","input_schema":{"type":"object","properties":{"path":{"type":"object"},"query":{"type":"object"},"body":{"type":"object"}}},"output_schema":{"type":"object"}
        }]})).unwrap(),
        pages:vec![plugin_framework::PluginSettingsPageManifest { feature_id:"fixture.settings".into(), contribution_code:"settings".into(), source_file:"ui/Settings.tsx".into(), language:"tsx".into() }],
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
    assert_eq!(operation.routes.len(), 1);
    assert_eq!(
        operation.routes[0].path,
        "/api/console/managed-services/fixture/items"
    );
    assert!(
        plan.route_assembly
            .bindings()
            .iter()
            .any(|binding| binding.route.path == operation.routes[0].path)
    );
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
