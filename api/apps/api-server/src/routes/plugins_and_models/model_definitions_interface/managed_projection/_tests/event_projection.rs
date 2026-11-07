use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/data-models/model-definitions:batchDelete".into(),
    }
}
#[test]
fn model_field_business_json_is_exact_and_documented_wire_names_are_preserved() {
    let op = operation("model_fields.create");
    let metadata = json!({"display":{"secret_label":"Public business metadata","values":(0..100).collect::<Vec<_>>()}});
    let input = ModelDefinitionsInput::CreateField {
        model_id: "model-a".into(),
        body: CreateModelFieldBody {
            code: "field-a".into(),
            title: "Field".into(),
            description: None,
            external_field_key: None,
            field_kind: "text".into(),
            is_required: false,
            api_required: Some(true),
            is_unique: false,
            default_value: Some(json!({"secret_label":"Public default"})),
            display_interface: Some("input".into()),
            display_options: metadata.clone(),
            relation_target_model_id: None,
            relation_options: json!({}),
        },
    };
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["path"], json!({"id":"model-a"}));
    assert_eq!(value["body"]["display_options"], metadata);
    let batch = ModelDefinitionsInput::BatchDelete(BatchDeleteModelDefinitionsBody {
        filter_by_tk: Some(json!(["model-a"])),
        filter: None,
        confirmed: true,
    })
    .project_document_input(&op)
    .unwrap();
    assert_eq!(batch["body"]["filterByTk"], json!(["model-a"]));
    assert!(batch["body"].get("filter_by_tk").is_none());
}
#[test]
fn model_delete_returns_real_deleted_json_and_optional_query_fields_are_absent() {
    let op = operation("model_definitions.delete");
    assert_eq!(
        ModelDefinitionsOutput::Deleted
            .project_document_output(&op)
            .unwrap(),
        json!({"deleted":true})
    );
    let input = ModelDefinitionsInput::List {
        query: ListModelsQuery {
            data_source_id: None,
            filter: None,
        },
        locale: ConsoleLocaleHints::from_headers(&axum::http::HeaderMap::new()),
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"query":{}})
    );
    assert_eq!(
        ModelDefinitionsInput::DeleteField {
            model_id: "model-a".into(),
            field_id: "field-a".into(),
            confirmed: true
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"path":{"id":"model-a","field_id":"field-a"},"query":{"confirmed":true}})
    );
}
