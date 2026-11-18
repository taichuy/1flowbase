use super::*;
fn context(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/fixture".into(),
    }
}

#[test]
fn memory_reveal_uses_safe_entry_metadata_and_preserves_value_state() {
    let c = context("host_infrastructure.memory.entry.reveal");
    let input = MemoryInspectionInput::Reveal {
        contract_code: "runtime".into(),
        body: MemoryEntryRevealBody {
            entry_ref: "opaque-entry-sentinel".into(),
            reveal_mode: Some("preview".into()),
        },
    };
    assert_eq!(
        input.project_document_input(&c).unwrap(),
        json!({"path":{"contract_code":"runtime"},"body":{"reveal_mode":"preview"}})
    );
    let metadata = MemoryEntryMetadataResponse {
        contract_code: "runtime".into(),
        group_code: None,
        entry_ref: "opaque-entry-sentinel".into(),
        key: "credential-key-sentinel".into(),
        inspection_path: vec!["cache".into()],
        entry_kind: "cache_entry".into(),
        status: "ready".into(),
        owner: Some("owner-sentinel".into()),
        value_size_bytes: 20,
        metadata_size_bytes: 5,
        ttl_seconds: Some(60),
        created_at_unix: None,
        expires_at_unix: None,
        sensitive: true,
        metadata: json!({"password":"credential-sentinel"}),
    };
    let output = MemoryInspectionOutput::Revealed(MemoryEntryValueResponse {
        metadata,
        reveal_mode: "preview".into(),
        value_state: "available".into(),
        value: Some(json!("credential-sentinel")),
        value_preview: Some("credential-sentinel".into()),
        preview_size_bytes: 20,
        full_value_size_bytes: 20,
    });
    let value = output.project_document_output(&c).unwrap();
    assert_eq!(value["value_state"], "available");
    assert_eq!(value["metadata"]["ttl_seconds"], 60);
    assert_eq!(value["metadata"]["sensitive"], true);
    assert!(!value.to_string().contains("sentinel"));
    let schema = MemoryInspectionOutput::document_output_schema(&c).unwrap();
    assert!(schema["properties"].get("value").is_none());
    assert!(schema["properties"].get("value_preview").is_none());
    let metadata = &schema["properties"]["metadata"]["properties"];
    for field in ENTRY_HIDDEN {
        assert!(metadata.get(*field).is_none());
    }
}
