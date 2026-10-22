use super::*;
use crate::openapi_docs::{DocsCatalogCategory, DocsCatalogOperation};
use serde_json::json;

#[test]
fn docs_projection_preserves_navigation_but_never_exposes_document_content() {
    let catalog = DocsCatalog {
        title: "private title".into(),
        version: "1".into(),
        categories: vec![DocsCatalogCategory {
            id: "applications".into(),
            label: "private label".into(),
            operation_count: 1,
        }],
    };
    let catalog_view = DocsOutputProjection::Catalog(&catalog)
        .project_for_managed_hook()
        .unwrap();
    assert_eq!(catalog_view["0"]["categories"][0]["id"], "applications");
    assert_eq!(catalog_view["0"]["title"], json!({"byte_count": 13}));
    assert!(!catalog_view.to_string().contains("private"));

    let page = DocsCatalogCategoryOperationsPage {
        id: "applications".into(),
        label: "private label".into(),
        operations: vec![DocsCatalogOperation {
            id: "get_application".into(),
            method: "GET".into(),
            path: "/private/path".into(),
            summary: Some("private summary".into()),
            description: None,
            tags: vec!["private tag".into()],
            group: "private group".into(),
            deprecated: false,
        }],
        total: 3,
        offset: 1,
        limit: 1,
        has_more: true,
        next_offset: Some(2),
    };
    let view = DocsOutputProjection::CategoryOperations(&page)
        .project_for_managed_hook()
        .unwrap();
    assert_eq!(view["0"]["operations"][0]["id"], "get_application");
    assert_eq!(view["0"]["operations"][0]["description"], Value::Null);
    assert_eq!(view["0"]["next_offset"], 2);
    assert_eq!(view["0"]["has_more"], true);
    assert!(!view.to_string().contains("private"));

    let document = json!({"private_schema": {"secret": "not a hook field"}});
    assert_eq!(
        DocsOutputProjection::OpenApi(&document).project_for_managed_hook(),
        Some(json!({"variant": "OpenApi", "0": {
            "kind": "object", "item_count": 1, "byte_count": 0
        }}))
    );
}

#[test]
fn docs_projection_keeps_existing_metadata_bounds() {
    let mut catalog = DocsCatalog {
        title: "title".into(),
        version: "1".into(),
        categories: vec![
            DocsCatalogCategory {
                id: "category".into(),
                label: "label".into(),
                operation_count: 1,
            };
            32
        ],
    };
    assert!(DocsOutputProjection::Catalog(&catalog)
        .project_for_managed_hook()
        .is_some());
    catalog.categories.push(catalog.categories[0].clone());
    assert!(DocsOutputProjection::Catalog(&catalog)
        .project_for_managed_hook()
        .is_none());
    catalog.categories.clear();
    catalog.version = "v".repeat(257);
    assert!(DocsOutputProjection::Catalog(&catalog)
        .project_for_managed_hook()
        .is_none());
}
