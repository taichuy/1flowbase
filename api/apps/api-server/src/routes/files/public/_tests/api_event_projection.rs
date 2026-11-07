use super::*;
#[test]
fn temporary_access_key_is_omitted_from_safe_query_and_response_remains_binary() {
    let op = ApiEventOperationContext {
        interface_id: "public.files.temporary.read".into(),
        method: "GET".into(),
        path: "/api/public/files/content".into(),
    };
    let key = control_plane::file_management::FileAccessKey::from_token(&"A".repeat(43)).unwrap();
    assert_eq!(
        TemporaryFileInput(key, true)
            .project_document_input(&op)
            .unwrap(),
        json!({"query":{"preview":true}})
    );
    let schema = TemporaryFileInput::document_input_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("token"));
    assert_eq!(
        TemporaryFileOutput::document_output_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
}
