use std::collections::HashMap;

use control_plane::resource_crud::{
    filter_by_tk_values, filter_resource_records, matches_resource_filter, parse_resource_filter,
    parse_resource_filter_expr, select_batch_ids, ResourceBatchSelection, ResourceFilterExpr,
    ResourceFilterOperator, ResourceFilterTarget,
};
use serde_json::json;

struct TestResource {
    fields: HashMap<&'static str, String>,
}

impl TestResource {
    fn new(fields: &[(&'static str, &str)]) -> Self {
        Self {
            fields: fields
                .iter()
                .map(|(key, value)| (*key, (*value).to_string()))
                .collect(),
        }
    }
}

impl ResourceFilterTarget for TestResource {
    fn field_value(&self, field: &str) -> Option<String> {
        self.fields.get(field).cloned()
    }
}

#[test]
fn resource_filter_matches_nocobase_style_field_operators() {
    let resource = TestResource::new(&[
        ("code", "customer_profiles"),
        ("title", "Customer Profiles"),
        ("status", "published"),
    ]);
    let filter = json!({
        "$or": [
            { "code": { "$includes": "customer" } },
            { "title.$includes": "customer" }
        ],
        "status": "published"
    });

    assert!(matches_resource_filter(&resource, &filter).unwrap());
}

#[test]
fn resource_filter_rejects_unknown_operator() {
    let resource = TestResource::new(&[("code", "orders")]);
    let error = matches_resource_filter(
        &resource,
        &json!({
            "code": { "$startsWith": "ord" }
        }),
    )
    .unwrap_err();

    assert!(error.to_string().contains("filter"));
}

#[test]
fn parse_resource_filter_accepts_empty_and_json_object() {
    assert!(parse_resource_filter(None).unwrap().is_none());
    assert!(parse_resource_filter(Some("")).unwrap().is_none());
    assert!(parse_resource_filter(Some(" ")).unwrap().is_none());
    assert_eq!(
        parse_resource_filter(Some("{}")).unwrap(),
        Some(ResourceFilterExpr::All(Vec::new()))
    );
    assert_eq!(
        parse_resource_filter(Some(r#"{"status":"published"}"#)).unwrap(),
        Some(ResourceFilterExpr::Field {
            field: "status".to_string(),
            operator: ResourceFilterOperator::Eq,
            value: json!("published"),
        })
    );
}

#[test]
fn parse_resource_filter_expr_builds_shared_ast() {
    let expr = parse_resource_filter_expr(&json!({
        "$and": [
            { "code": { "$includes": "customer" } },
            { "status.$in": ["draft", "published"] }
        ]
    }))
    .unwrap();

    assert_eq!(
        expr,
        ResourceFilterExpr::All(vec![
            ResourceFilterExpr::Field {
                field: "code".to_string(),
                operator: ResourceFilterOperator::Includes,
                value: json!("customer"),
            },
            ResourceFilterExpr::Field {
                field: "status".to_string(),
                operator: ResourceFilterOperator::In,
                value: json!(["draft", "published"]),
            },
        ])
    );
}

#[test]
fn parse_resource_filter_expr_rejects_invalid_logical_operator() {
    let error = parse_resource_filter_expr(&json!({
        "$unknown": []
    }))
    .unwrap_err();

    assert!(error.to_string().contains("filter"));
}

#[test]
fn parse_resource_filter_expr_requires_in_array() {
    let error = parse_resource_filter_expr(&json!({
        "status": { "$in": "published" }
    }))
    .unwrap_err();

    assert!(error.to_string().contains("filter"));
}

#[test]
fn filter_by_tk_accepts_single_or_many_keys() {
    assert_eq!(
        filter_by_tk_values(json!("model-1")).unwrap(),
        vec!["model-1".to_string()]
    );
    assert_eq!(
        filter_by_tk_values(json!(["model-1", "model-2"])).unwrap(),
        vec!["model-1".to_string(), "model-2".to_string()]
    );
}

#[test]
fn filter_resource_records_returns_matching_records() {
    let resources = vec![
        TestResource::new(&[("code", "orders")]),
        TestResource::new(&[("code", "customer_profiles")]),
    ];

    let filter = parse_resource_filter(Some(r#"{"code":{"$includes":"customer"}}"#)).unwrap();
    let filtered = filter_resource_records(resources, filter.as_ref()).unwrap();

    assert_eq!(filtered.len(), 1);
    assert_eq!(
        filtered[0].field_value("code").as_deref(),
        Some("customer_profiles")
    );
}

#[test]
fn select_batch_ids_by_filter_by_tk() {
    let selected = select_batch_ids::<TestResource, String, _, _>(
        Vec::new(),
        ResourceBatchSelection::new(Some(json!(["id-1", "id-2"])), None),
        Ok,
        |resource| resource.field_value("id").unwrap_or_default(),
    )
    .unwrap();

    assert_eq!(selected, vec!["id-1".to_string(), "id-2".to_string()]);
}

#[test]
fn select_batch_ids_by_filter() {
    let resources = vec![
        TestResource::new(&[("id", "1"), ("code", "orders")]),
        TestResource::new(&[("id", "2"), ("code", "customer_profiles")]),
    ];

    let selected = select_batch_ids(
        resources,
        ResourceBatchSelection::new(None, Some(json!({ "code.$includes": "customer" }))),
        Ok,
        |resource| resource.field_value("id").unwrap_or_default(),
    )
    .unwrap();

    assert_eq!(selected, vec!["2".to_string()]);
}

#[test]
fn parse_resource_filter_rejects_invalid_json_shape_and_operators() {
    for filter in [
        "{",
        "[]",
        r#"{"$unknown":[]}"#,
        r#"{"status":{"$in":"published"}}"#,
    ] {
        assert!(matches!(
            parse_resource_filter(Some(filter)),
            Err(control_plane::errors::ControlPlaneError::InvalidInput(
                "filter"
            ))
        ));
    }
}

#[test]
fn select_batch_ids_requires_explicit_selection_and_preserves_key_precedence() {
    use control_plane::errors::ControlPlaneError;

    let select = |selection| {
        select_batch_ids::<TestResource, String, _, _>(Vec::new(), selection, Ok, |resource| {
            resource.field_value("id").unwrap_or_default()
        })
    };
    assert!(matches!(
        select(ResourceBatchSelection::new(None, None)),
        Err(ControlPlaneError::InvalidInput("filterByTk"))
    ));
    for filter in [json!({}), json!([]), json!({"status": {"$unknown": true}})] {
        assert!(matches!(
            select(ResourceBatchSelection::new(None, Some(filter))),
            Err(ControlPlaneError::InvalidInput("filter"))
        ));
    }
    assert_eq!(
        select(ResourceBatchSelection::new(
            Some(json!(["id-1"])),
            Some(json!([]))
        ))
        .unwrap(),
        vec!["id-1".to_string()]
    );
    assert!(matches!(
        select(ResourceBatchSelection::new(
            Some(json!(true)),
            Some(json!({}))
        )),
        Err(ControlPlaneError::InvalidInput("filterByTk"))
    ));
}
