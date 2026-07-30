use utoipa::OpenApi;

#[test]
fn runtime_catalog_route_is_authenticated_documented_and_module_free() {
    let assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly();
    let binding = assembly
        .bindings()
        .iter()
        .find(|binding| {
            binding.route.method == "GET" && binding.route.path == "/api/console/i18n/catalog"
        })
        .unwrap();
    assert_eq!(
        binding.ownership,
        access_control::ConsoleRouteOwnership::Authenticated
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
