use super::*;
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/files/upload".into(),
    }
}
#[test]
fn file_share_credentials_and_remote_access_url_are_absent_from_event_views() {
    let op = operation("files.access_links.create");
    let response = FileShareLinkResponse {
        url: "https://files.test?token=SHARE-TOKEN-SENTINEL".into(),
        preview_url: Some("https://files.test?token=PREVIEW-TOKEN-SENTINEL".into()),
        expires_at: "2026-11-07".into(),
    };
    assert!(serde_json::to_value(&response)
        .unwrap()
        .to_string()
        .contains("SHARE-TOKEN-SENTINEL"));
    assert_eq!(
        BusinessFilesOutput::ShareLink(response)
            .project_document_output(&op)
            .unwrap(),
        json!({"expires_at":"2026-11-07"})
    );
    let schema = BusinessFilesOutput::document_output_schema(&op)
        .unwrap()
        .to_string();
    assert!(!schema.contains("preview_url"));
    assert!(!schema.contains("\"url\""));
    let op = operation("files.import_remote");
    let input = BusinessFilesInput::ImportRemote {
        file_table_id: None,
        file: RemoteFileReference {
            download_url: "https://remote.test?key=REMOTE-KEY-SENTINEL".into(),
            file_id: "remote-file-a".into(),
            mime_type: Some("text/plain".into()),
            file_name: Some("notes.txt".into()),
        },
    };
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["body"]["file"]["file_id"], "remote-file-a");
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(!BusinessFilesInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("download_url"));
}
#[test]
fn binary_file_phases_do_not_disable_real_uploaded_metadata_json() {
    let op = operation("files.upload");
    assert_eq!(
        BusinessFilesInput::document_input_unavailable(&op),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryRequest)
    );
    assert_eq!(BusinessFilesOutput::document_output_unavailable(&op), None);
    let record = json!({"id":"file-a","business_metadata":{"secret_label":"Public title","values":(0..100).collect::<Vec<_>>()}});
    assert_eq!(
        BusinessFilesOutput::Uploaded(UploadedFileResponse {
            file_table_id: "table-a".into(),
            storage_id: "storage-a".into(),
            record: record.clone()
        })
        .project_document_output(&op)
        .unwrap(),
        json!({"file_table_id":"table-a","storage_id":"storage-a","record":record})
    );
    assert_eq!(
        BusinessFilesOutput::document_output_unavailable(&operation("files.content.download")),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
}
