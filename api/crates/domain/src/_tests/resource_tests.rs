use domain::runtime_model_resource_code;

#[test]
fn model_definition_repository_uses_model_code_for_runtime_resource_code() {
    assert_eq!(
        runtime_model_resource_code("orders"),
        "models.runtime.orders"
    );
}

#[test]
fn application_template_category_and_catalog_identity_roundtrip() {
    use crate::{ExtensionCatalogIdentity, ExtensionCategory};
    let category = ExtensionCategory::parse("applications-demo").unwrap();
    assert_eq!(category, ExtensionCategory::ApplicationsDemo);
    assert!(ExtensionCategory::ALL.contains(&category));
    assert_eq!(
        serde_json::to_string(&category).unwrap(),
        "\"applications-demo\""
    );
    assert_eq!(
        serde_json::from_str::<ExtensionCategory>("\"applications-demo\"").unwrap(),
        category
    );
    let identity =
        ExtensionCatalogIdentity::parse(category, "applications-demo:taichuy/gateway-demo")
            .unwrap();
    assert_eq!(
        identity.catalog_id(),
        "applications-demo:taichuy/gateway-demo"
    );
    assert_eq!(identity.organization(), "taichuy");
    assert_eq!(identity.artifact_id(), "gateway-demo");
    assert!(ExtensionCatalogIdentity::parse(category, "agent-flow:taichuy/gateway-demo").is_none());
    assert!(
        ExtensionCatalogIdentity::parse(category, "applications-demo:taichuy/../gateway-demo")
            .is_none()
    );
}
