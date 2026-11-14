use super::*;
fn context(id: &str) -> ApiOperationContext {
    let declaration = DECLARATIONS
        .iter()
        .find(|declaration| declaration.interface_id == id)
        .expect("declared docs operation");
    ApiOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: declaration.path.into(),
    }
}

#[test]
fn document_catalog_keeps_actual_content_beyond_managed_limit() {
    let output = DocsOutput::Catalog(DocsCatalog {
        title: "actual title".into(),
        version: "1".into(),
        categories: (0..40)
            .map(|i| crate::openapi_docs::DocsCatalogCategory {
                id: format!("category-{i}"),
                label: format!("Actual label {i}"),
                operation_count: i,
            })
            .collect(),
    });
    let value = output
        .project_document_output(&context("docs.catalog.view"))
        .unwrap();
    assert_eq!(value["title"], "actual title");
    assert_eq!(value["categories"].as_array().unwrap().len(), 40);
    assert_eq!(value["categories"][39]["label"], "Actual label 39");
    assert!(output.project_for_managed_hook().is_none());
}
#[test]
fn document_query_omits_absent_parameters_without_replacing_path_names() {
    let input = DocsInput::CategoryOperations {
        category_id: "core".into(),
        query: DocsCategoryOperationsQuery {
            offset: None,
            limit: Some(10),
            q: Some("actual search".into()),
        },
    };
    assert_eq!(
        input
            .project_document_input(&context("docs.categories.operations.list"))
            .unwrap(),
        json!({"path":{"category_id":"core"},"query":{"limit":10,"q":"actual search"}})
    );
}
