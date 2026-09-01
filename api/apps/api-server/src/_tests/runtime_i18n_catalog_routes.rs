use utoipa::OpenApi;

#[test]
fn runtime_catalog_route_is_authenticated_documented_and_module_free() {
    let bindings = crate::routes::console_route_assembly::migrated_core_console_contract_bindings();
    let binding = bindings
        .iter()
        .find(|binding| {
            binding.route.method == "GET" && binding.route.path == "/api/console/i18n/catalog"
        })
        .unwrap();
    assert_eq!(
        binding.ownership,
        access_control::ConsoleRouteOwnership::ConsoleOperation("i18n.catalog.view".to_string())
    );
    let settings = crate::app_state::compile_core_settings_feature_registry().unwrap();
    let registry = crate::routes::console_route_assembly::compile_complete_migrated_console_operation_registry(
        &settings,
        &bindings,
        &[],
    )
    .unwrap();
    let access = registry
        .access_for_console_route("GET", "/api/console/i18n/catalog")
        .unwrap();
    assert_eq!(access.operation_id, "i18n.catalog.view");
    assert_eq!(
        access.authorization,
        &access_control::ConsoleAuthorization::Authenticated
    );

    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    let operation = &openapi["paths"]["/api/console/i18n/catalog"]["get"];
    assert!(!operation["summary"].as_str().unwrap().is_empty());
    assert!(!operation["description"].as_str().unwrap().is_empty());

    let source = include_str!("../routes/runtime_i18n_catalog.rs");
    assert!(!source.contains("module"));
    assert!(!source.contains("bundles"));
    assert!(source.contains("catalog_revision"));
    assert!(source.contains("messages"));
}
