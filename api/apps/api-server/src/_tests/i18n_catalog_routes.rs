use utoipa::OpenApi;

#[test]
fn settings_i18n_routes_expose_state_and_management_without_module_bundle_contract() {
    let assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly();
    assert!(assembly.bindings().iter().any(|binding| {
        binding.route.method == "GET" && binding.route.path == "/api/console/settings/i18n/catalog"
    }));
    assert!(!assembly
        .bindings()
        .iter()
        .any(|binding| binding.route.path.contains("/i18n/modules/")));

    let openapi = serde_json::to_value(crate::openapi::ApiDoc::openapi()).unwrap();
    assert!(openapi["paths"]
        .as_object()
        .unwrap()
        .keys()
        .all(|path| !path.contains("/i18n/modules/")));
    let state_schema = &openapi["components"]["schemas"]["I18nCatalogStateResponse"];
    let properties = state_schema["properties"].as_object().unwrap();
    assert!(properties.get("modules").is_none());
    assert!(properties.get("module").is_none());
    assert!(properties.get("msgid").is_none());
}
