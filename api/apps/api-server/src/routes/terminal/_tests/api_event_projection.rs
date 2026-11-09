use super::*;
fn operation() -> ApiOperationContext {
    ApiOperationContext {
        interface_id: "terminal.files.read_text".into(),
        method: "POST".into(),
        path: "/api/console/terminal/files/read-text".into(),
    }
}
#[test]
fn terminal_input_is_real_body_and_long_text_result_is_complete() {
    let op = operation();
    let input = TerminalInput::Text(TextInput {
        path: "notes.txt".into(),
        timeout_ms: 5000,
        offset: 10,
        max_bytes: 100000,
        expected_version: Some("version-a".into()),
    });
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"body":{"path":"notes.txt","timeout_ms":5000,"offset":10,"max_bytes":100000,"expected_version":"version-a"}})
    );
    let text = "完整文本\n".repeat(15000);
    let response = TextOutput {
        path: "notes.txt".into(),
        offset: 0,
        text: text.clone(),
        next_offset: text.len() as u64,
        eof: true,
        version: "version-a".into(),
    };
    let expected = serde_json::to_value(&response).unwrap();
    assert_eq!(
        TerminalOutput::Text(response)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
}
#[test]
fn terminal_media_is_actual_json_and_not_a_raw_binary_response() {
    let op = operation();
    let response = MediaOutput {
        data: "YWJj".repeat(10000),
        mime_type: "image/png".into(),
        metadata: FileMetadata {
            name: "image.png".into(),
            path: "image.png".into(),
            kind: "file".into(),
            size: 30000,
            modified_at: None,
            permissions: None,
            version: "version-a".into(),
        },
    };
    let expected = serde_json::to_value(&response).unwrap();
    assert_eq!(
        TerminalOutput::Media(response)
            .project_document_output(&op)
            .unwrap(),
        expected
    );
    assert_eq!(TerminalOutput::document_output_unavailable(&op), None);
}
