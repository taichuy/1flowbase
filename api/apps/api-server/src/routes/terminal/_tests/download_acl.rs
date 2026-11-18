use super::*;
use control_plane::file_management::{FileUploadService, FileUploadTarget, UploadFileCommand};

#[tokio::test]
async fn terminal_download_scope_denial_is_forbidden_and_preserves_files() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (root, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut headers = HeaderMap::new();
    headers.insert("cookie", root.parse().unwrap());
    let actor = crate::middleware::require_session::require_session(&state, &headers)
        .await
        .unwrap()
        .interface_principal()
        .actor()
        .clone();
    let (status, table) = request(
        &app,
        "POST",
        "/api/console/settings/files/tables",
        &root,
        &csrf,
        json!({"code":"terminal_denied_files","title":"Denied files"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{table}");
    let table_id = Uuid::parse_str(table["data"]["id"].as_str().unwrap()).unwrap();
    let source = FileUploadService::new(
        state.store.clone(),
        state.file_storage_registry.clone(),
        state.runtime_engine.clone(),
    )
    .upload(UploadFileCommand {
        actor,
        target: FileUploadTarget::Table(table_id),
        original_filename: "private.bin".into(),
        content_type: Some("application/octet-stream".into()),
        bytes: b"private source bytes".to_vec(),
    })
    .await
    .unwrap();
    let member = create_member(&app, &root, &csrf, "download-member", "temp-pass").await;
    create_role(&app, &root, &csrf, "download_role").await;
    replace_member_roles(&app, &root, &csrf, &member, &["download_role"]).await;
    grant(
        &app,
        &root,
        &csrf,
        "download_role",
        &["terminal.files.download_file"],
    )
    .await;
    let (cookie, token) = login_and_capture_cookie(&app, "download-member", "temp-pass").await;
    let temp = Temp::new();
    let target = temp.0.join("target.bin");
    for exists in [false, true] {
        if exists {
            std::fs::write(&target, b"existing destination").unwrap();
        }
        let (status, result) = request(
            &app,
            "POST",
            "/api/console/terminal/files/download-file",
            &cookie,
            &token,
            json!({"file_table_id":table_id,"record_id":source.record["id"],"target_path":target,"timeout_ms":5000,"overwrite":true}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{result}");
        assert_eq!(result["code"], "data_model_scope_not_granted");
        assert_eq!(
            std::fs::read_dir(&temp.0).unwrap().count(),
            usize::from(exists)
        );
        if exists {
            assert_eq!(std::fs::read(&target).unwrap(), b"existing destination");
        } else {
            assert!(!target.exists());
        }
    }
}
