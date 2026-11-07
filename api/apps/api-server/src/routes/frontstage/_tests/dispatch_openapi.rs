use super::super::{
    DispatchFrontstageActionBody, DispatchFrontstageQueryBody,
    FrontstageNavigationPlacementResponse, FrontstagePageContentPresentationResponse,
    FrontstagePageDetailResponse, FrontstagePageResponse, FrontstagePageTabResponse,
    FrontstagePageTreeNodeKind, FrontstageTabDocumentResponse,
};
use super::*;
use serde_json::json;

fn validator<T: ToSchema>() -> jsonschema::Validator {
    jsonschema::validator_for(&crate::event_center::api_projection::schema::<T>()).unwrap()
}

#[test]
fn frontstage_dispatch_schema_accepts_actual_body_defaults_and_serialized_page_detail() {
    let query: DispatchFrontstageQueryBody =
        serde_json::from_value(json!({"query_id":"frontstage.page_tab.get"})).unwrap();
    assert!(query.params.is_null());
    assert!(
        validator::<DispatchFrontstageQueryBody>().is_valid(&serde_json::to_value(query).unwrap())
    );
    let action: DispatchFrontstageActionBody = serde_json::from_value(
        json!({"action_id":"frontstage.page_tab.document.save","params":{"payload":{"blocks":[]}}}),
    )
    .unwrap();
    assert!(validator::<DispatchFrontstageActionBody>()
        .is_valid(&serde_json::to_value(action).unwrap()));
    let detail = FrontstagePageDetailResponse {
        page: FrontstagePageResponse {
            id: "page".into(),
            title: Some("Actual title".into()),
            icon: None,
            tooltip: None,
            is_hidden: false,
            show_title: true,
            kind: FrontstagePageTreeNodeKind::Page,
            parent_id: None,
            rank: "a".into(),
            placement: FrontstageNavigationPlacementResponse::Sidebar,
            content_presentation: FrontstagePageContentPresentationResponse::Tabs,
            slug: Some("fixture".into()),
        },
        tab: FrontstagePageTabResponse {
            id: "tab".into(),
            page_id: "page".into(),
            title: None,
            rank: "a".into(),
            is_default: true,
            route_segment: None,
            document_root_uid: "root".into(),
        },
        document: FrontstageTabDocumentResponse {
            root_uid: "root".into(),
            payload: json!({"blocks":[],"extension_value":[1,true,null]}),
        },
    };
    let wire = serde_json::to_value(detail).unwrap();
    assert!(validator::<FrontstageQueryResultSchema>().is_valid(&wire));
    assert!(validator::<FrontstageActionResultSchema>().is_valid(&wire));
    assert_eq!(wire["page"]["title"], "Actual title");
    let mut invalid = wire;
    invalid["tab"]["is_default"] = json!("yes");
    assert!(!validator::<FrontstageQueryResultSchema>().is_valid(&invalid));
}

#[test]
fn frontstage_dispatch_schema_preserves_model_envelopes_and_dynamic_record_fields() {
    let result = runtime_core::runtime_engine::RuntimeListResult {
        items: vec![json!({"id":"record","model_owned_column":{"nested":[1,true,null]}})],
        total: 1,
    };
    // Same model-owned runtime values returned by register_data_model_query_handlers.
    let wire = json!({"model":"fixture_model","items":result.items,"total":result.total});
    assert!(validator::<FrontstageQueryResultSchema>().is_valid(&wire));
    assert!(!validator::<FrontstageActionResultSchema>().is_valid(&wire));
    let record = json!({"model":"fixture_model","record":{"id":"record","model_owned_column":12}});
    assert!(validator::<FrontstageQueryResultSchema>().is_valid(&record));
    assert!(validator::<FrontstageActionResultSchema>().is_valid(&record));
    assert!(!validator::<FrontstageQueryResultSchema>().is_valid(&json!({"model":"fixture_model"})));
}
