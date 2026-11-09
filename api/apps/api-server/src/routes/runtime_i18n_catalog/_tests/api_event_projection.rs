use super::*;
#[test]
fn runtime_catalog_keeps_actual_messages_without_payload_wrapper_or_headers() {
    let op = ApiOperationContext {
        interface_id: "i18n.catalog.view".into(),
        method: "GET".into(),
        path: "/api/console/i18n/catalog".into(),
    };
    let input = RuntimeI18nInput {
        locale: "zh_Hans".into(),
        if_none_match: Some("HEADER-SENTINEL".into()),
    };
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"query":{"locale":"zh_Hans"}})
    );
    let payload = RuntimeI18nCatalogResponse {
        catalog_revision: 7,
        locale: "zh_Hans".into(),
        digest: "digest-a".into(),
        messages: (0..100)
            .map(|i| (format!("key-{i}"), "内容".repeat(300)))
            .collect(),
    };
    let expected = serde_json::to_value(&payload).unwrap();
    assert_eq!(
        RuntimeI18nOutput {
            payload: Some(payload),
            etag: "HEADER-SENTINEL".into()
        }
        .project_document_output(&op)
        .unwrap(),
        expected
    );
    assert_eq!(
        RuntimeI18nOutput {
            payload: None,
            etag: "HEADER-SENTINEL".into()
        }
        .project_document_output(&op)
        .unwrap(),
        Value::Null
    );
    assert!(
        RuntimeI18nOutput::document_output_schema(&op).unwrap()["anyOf"]
            .as_array()
            .unwrap()
            .iter()
            .any(|schema| schema == &json!({"type":"null"}))
    );
}
