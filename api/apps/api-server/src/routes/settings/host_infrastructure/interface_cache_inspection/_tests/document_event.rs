use super::*;
fn context(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/fixture".into(),
    }
}

#[test]
fn cache_reveal_retains_metadata_and_omits_raw_key_and_value() {
    let output = CacheInspectionOutput::Revealed(CacheEntryValueResponse {
        metadata: CacheEntryMetadataResponse {
            domain_code: "sessions".into(),
            key: "credential-key-sentinel".into(),
            value_size_bytes: 200,
            ttl_seconds: Some(30),
            created_at_unix: Some(100),
            expires_at_unix: Some(130),
        },
        value: json!({"password":"credential-value-sentinel"}),
    });
    let c = context("host_infrastructure.cache.entry.reveal");
    let value = output.project_document_output(&c).unwrap();
    assert_eq!(value["metadata"]["ttl_seconds"], 30);
    assert_eq!(value["metadata"]["value_size_bytes"], 200);
    assert!(value.get("value").is_none());
    assert!(value["metadata"].get("key").is_none());
    assert!(!value.to_string().contains("sentinel"));
    let schema = CacheInspectionOutput::document_output_schema(&c).unwrap();
    assert!(schema["properties"].get("value").is_none());
    assert!(schema["properties"]["metadata"]["properties"]
        .get("key")
        .is_none());
    let input = CacheInspectionInput::Reveal {
        domain_code: "sessions".into(),
        body: CacheEntryKeyBody {
            key: "credential-key-sentinel".into(),
        },
    };
    assert_eq!(
        input.project_document_input(&c).unwrap(),
        json!({"path":{"domain_code":"sessions"},"body":{}})
    );
}
