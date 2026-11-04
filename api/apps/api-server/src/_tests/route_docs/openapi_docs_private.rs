#[test]
fn borrowed_catalog_projection_matches_every_materialized_operation() {
    let registry = api_server::openapi_docs::build_default_api_docs_registry().unwrap();
    for category in &registry.catalog().categories {
        for operation in &registry
            .category_operations(&category.id)
            .unwrap()
            .operations
        {
            let spec = registry.operation_spec(&operation.id).unwrap();
            let view = registry.operation_spec_view(&operation.id).unwrap();
            let expected = crate::openapi_interface::catalog_entry_from_operation(operation, &spec);
            let actual = crate::openapi_interface::catalog_entry_from_view(operation, &view);
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                serde_json::to_value(expected).unwrap(),
                "{}",
                operation.id
            );
        }
    }
}
