use super::*;
fn context(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/fixture".into(),
    }
}

#[test]
fn storage_events_exclude_driver_credentials_but_preserve_owner_fields() {
    let body = CreateFileStorageBody {
        code: "archive".into(),
        title: "Original title".into(),
        driver_type: "s3".into(),
        enabled: true,
        is_default: false,
        config_json: json!({"access_key":"credential-sentinel"}),
        rule_json: json!({"max_bytes":1024}),
    };
    let input = FileStoragesInput::Create(body);
    let value = input
        .project_document_input(&context("file_storages.create"))
        .unwrap();
    assert_eq!(value["body"]["title"], "Original title");
    assert_eq!(value["body"]["rule_json"], json!({"max_bytes":1024}));
    assert!(!value.to_string().contains("credential-sentinel"));
    let schema =
        FileStoragesInput::document_input_schema(&context("file_storages.create")).unwrap();
    assert!(schema["properties"]["body"]["properties"]
        .get("config_json")
        .is_none());
    let record = FileStorageResponse {
        id: "storage-id".into(),
        code: "archive".into(),
        title: "Original title".into(),
        driver_type: "s3".into(),
        enabled: true,
        is_default: false,
        config_json: json!({"access_key":"credential-sentinel"}),
        rule_json: json!({"max_bytes":1024}),
        health_status: "ready".into(),
        last_health_error: Some("credential-sentinel".into()),
    };
    assert_eq!(
        serde_json::to_value(&record).unwrap()["config_json"]["access_key"],
        "credential-sentinel"
    );
    let output = FileStoragesOutput::Item(record)
        .project_document_output(&context("file_storages.create"))
        .unwrap();
    assert_eq!(output["health_status"], "ready");
    assert!(output.get("config_json").is_none());
    assert!(output.get("last_health_error").is_none());
    assert!(!output.to_string().contains("credential-sentinel"));
}
