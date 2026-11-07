use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/settings/data-models/data-sources".into(),
    }
}
#[test]
fn data_source_secret_json_is_separate_from_complete_public_config() {
    let op = operation("data_sources.create");
    let public = json!({"table":"records","business_metadata":{"secret_label":"Public hint","rows":(0..90).collect::<Vec<_>>()}});
    let input = DataSourcesInput::Create(CreateDataSourceBody {
        installation_id: "installation-a".into(),
        source_code: "source-a".into(),
        display_name: "Source".into(),
        config_json: public.clone(),
        secret_json: json!({"password":"DATA-SOURCE-PASSWORD-SENTINEL"}),
    });
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["body"]["config_json"], public);
    assert!(value["body"].get("secret_json").is_none());
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(!DataSourcesInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("secret_json"));
    let DataSourcesInput::Create(body) = &input else {
        unreachable!()
    };
    assert_eq!(
        body.secret_json["password"],
        "DATA-SOURCE-PASSWORD-SENTINEL"
    );
}
#[test]
fn data_source_backend_secret_reference_is_not_an_event_field() {
    let op = operation("data_sources.create");
    let response = DataSourceResponse {
        id: "source-a".into(),
        display_name: "Source".into(),
        status: "ready".into(),
        enabled: true,
        fixed: false,
        default_data_model_status: "draft".into(),
        capabilities: DataSourceCapabilitiesResponse {
            can_update_defaults: true,
            can_create_data_model: true,
            can_validate: true,
            can_discover_resources: true,
            can_preview_resources: true,
            can_map_resources: true,
        },
        backend: DataSourceBackendResponse::RuntimeExtension {
            installation_id: "installation-a".into(),
            source_code: "external".into(),
            config_json: json!({"schema":"public"}),
            secret_ref: Some("SECRET-REFERENCE-SENTINEL".into()),
            secret_version: Some(3),
            catalog_refresh_status: None,
            catalog_last_error_message: None,
            catalog_refreshed_at: None,
        },
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap()["backend"]["secret_ref"],
        "SECRET-REFERENCE-SENTINEL"
    );
    let value = DataSourcesOutput::DataSource(response)
        .project_document_output(&op)
        .unwrap();
    assert!(value["backend"].get("secret_ref").is_none());
    assert_eq!(value["backend"]["config_json"], json!({"schema":"public"}));
    assert_eq!(value["backend"]["secret_version"], 3);
    assert!(!DataSourcesOutput::document_output_schema(&op)
        .unwrap()
        .to_string()
        .contains("secret_ref"));
}
#[test]
fn data_preview_keeps_business_session_identity_rows_and_cursor_without_summary_limits() {
    let op = operation("data_sources.preview");
    let rows = (0..80).map(|i|json!({"id":i,"metadata":{"secret_label":"Public business value"},"text":"行".repeat(200)})).collect::<Vec<_>>();
    let response = PreviewDataSourceReadResponse {
        preview_session_id: "business-preview-id".into(),
        expires_at: "2026-11-07".into(),
        output: DataSourcePreviewOutputResponse {
            rows,
            next_cursor: Some("next-page".into()),
        },
    };
    let expected = serde_json::to_value(&response).unwrap();
    assert_eq!(
        DataSourcesOutput::Preview(response)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}
