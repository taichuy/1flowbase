use super::*;
use crate::openapi_docs::{build_api_docs_registry, paginate_category_operations};

fn validate<T: utoipa::ToSchema>(wire: &Value) {
    let schema = crate::openapi_interface::contract::schema::<T>();
    assert!(
        jsonschema::validator_for(&schema).unwrap().is_valid(wire),
        "actual owner document rejected: {wire}; schema: {schema}"
    );
}
fn registry() -> crate::openapi_docs::ApiDocsRegistry {
    build_api_docs_registry(json!({
        "openapi":"3.1.0","info":{"title":"Scoped documents","version":"1","contact":{"name":"Owner"}},
        "servers":[{"url":"/","variables":{"zone":{"default":"dev","enum":["dev"]}}}],
        "security":[{"sessionCookie":[]}],
        "tags":[{"name":"Fixture","description":"Actual category"}],
        "paths":{"/fixture":{"get":{"operationId":"fixture_operation","tags":["Fixture"],"summary":"Actual summary",
            "responses":{"200":{"description":"A dynamic result","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Result"}}}}}}}},
        "components":{"schemas":{"Result":{"type":"object","properties":{"record":{"anyOf":[{"type":"object"},{"type":"null"}]}}}},
            "securitySchemes":{"sessionCookie":{"type":"apiKey","name":"session","in":"cookie"}}}
    })).unwrap()
}

#[test]
fn docs_openapi_schema_matches_actual_scoped_documents_and_reference_closure() {
    let registry = registry();
    let operation = registry.operation_spec("fixture_operation").unwrap();
    validate::<ConsoleOpenApiDocumentSchema>(&operation);
    let category = &registry.catalog().categories[0].id;
    let category = registry.category_spec(category).unwrap();
    validate::<ConsoleOpenApiDocumentSchema>(&category);
    assert_eq!(
        category["paths"]["/fixture"]["get"]["summary"],
        "Actual summary"
    );
    assert!(operation["components"]["schemas"].get("Result").is_some());
    let mut invalid = operation;
    invalid["paths"] = json!([]);
    let schema = crate::openapi_interface::contract::schema::<ConsoleOpenApiDocumentSchema>();
    assert!(!jsonschema::validator_for(&schema)
        .unwrap()
        .is_valid(&invalid));
}

#[test]
fn docs_catalog_and_operations_page_keep_their_real_serialized_fields() {
    let registry = registry();
    let wire = serde_json::to_value(registry.catalog()).unwrap();
    validate::<crate::openapi_docs::DocsCatalog>(&wire);
    let category = &registry.catalog().categories[0].id;
    let page = paginate_category_operations(registry.category_operations(category).unwrap(), 0, 20);
    let wire = serde_json::to_value(page).unwrap();
    validate::<crate::openapi_docs::DocsCatalogCategoryOperationsPage>(&wire);
    assert_eq!(wire["operations"][0]["id"], "fixture_operation");
    assert!(wire["next_offset"].is_null());
}
