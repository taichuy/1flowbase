use super::*;
use control_plane::file_management::{FileUploadService, FileUploadTarget, UploadFileCommand};

#[tokio::test]
async fn terminal_download_stages_checks_complete_hash_and_never_clobbers_by_default() {
    let temp = Temp::new();
    let target = temp.0.join("target");
    let body = || DownloadInput {
        file_table_id: Uuid::now_v7(),
        record_id: Uuid::now_v7(),
        target_path: target.to_str().unwrap().into(),
        timeout_ms: 1000,
        overwrite: false,
        expected_sha256: None,
    };
    let budget = || local_terminal::Budget::new(1000).unwrap();
    let output = transfer::transfer(&b"complete"[..], 8, body(), budget())
        .await
        .unwrap();
    assert_eq!(output.byte_count, 8);
    assert_eq!(std::fs::read(&target).unwrap(), b"complete");
    assert_eq!(output.sha256.len(), 64);
    assert!(transfer::transfer(&b"changed!"[..], 8, body(), budget())
        .await
        .is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"complete");
    let mut wrong = body();
    wrong.overwrite = true;
    wrong.expected_sha256 = Some("0".repeat(64));
    assert!(transfer::transfer(&b"changed!"[..], 8, wrong, budget())
        .await
        .is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"complete");
    let mut short = body();
    short.overwrite = true;
    assert!(transfer::transfer(&b"partial"[..], 9, short, budget())
        .await
        .is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"complete");
    let mut overwrite = body();
    overwrite.overwrite = true;
    transfer::transfer(&b"changed!"[..], 8, overwrite, budget())
        .await
        .unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"changed!");
    assert_eq!(
        std::fs::read_dir(&temp.0).unwrap().count(),
        1,
        "staging files are removed on success and errors"
    );
}

#[tokio::test]
async fn terminal_import_and_download_use_real_file_tables_and_reject_revoked_source_grants() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut headers = HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    let context = crate::middleware::require_session::require_session(&state, &headers)
        .await
        .unwrap();
    let principal = context.interface_principal();
    let actor = principal.actor().clone();
    let (status, table) = request(
        &app,
        "POST",
        "/api/console/settings/files/tables",
        &cookie,
        &csrf,
        json!({"code":"terminal_transfer","title":"Terminal transfer"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{table}");
    let table_id = Uuid::parse_str(table["data"]["id"].as_str().unwrap()).unwrap();
    let upload = FileUploadService::new(
        state.store.clone(),
        state.file_storage_registry.clone(),
        state.runtime_engine.clone(),
    )
    .upload(UploadFileCommand {
        actor: actor.clone(),
        target: FileUploadTarget::Table(table_id),
        original_filename: "original.bin".into(),
        content_type: Some("application/octet-stream".into()),
        bytes: b"authorized file bytes\0".to_vec(),
    })
    .await
    .unwrap();
    let temp = Temp::new();
    let destination = temp.0.join("download.bin");
    let body = json!({"file_table_id":table_id,"record_id":upload.record["id"],"target_path":destination,"timeout_ms":5000});
    let (status, value) = request(
        &app,
        "POST",
        "/api/console/terminal/files/download-file",
        &cookie,
        &csrf,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"authorized file bytes\0"
    );
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/files/download-file",
        &cookie,
        &csrf,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let source = temp.0.join("import.bin");
    std::fs::write(&source, b"resource import bytes\0").unwrap();
    let (status, value) = request(&app,"POST","/api/console/mcp/instances",&cookie,&csrf,json!({"instance_id":"terminal_fixture","name":"Terminal fixture","status":"enabled","default_entry_path":"/"})).await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    let names = ["path", "timeout_ms", "max_file_bytes", "link_only"];
    let (status, value) = request(&app,"POST","/api/console/mcp/tools",&cookie,&csrf,json!({"tool_id":"terminal_fixture_read","des_id":"terminalfixturev1","name":"Read fixture resource","short_description":"Read local fixture resource.","full_description":"Read local fixture resource through its canonical terminal interface.","execution_target":{"kind":"interface_wrapper","interface_id":"terminal.files.read_resource"},"parameter_schema":{"type":"object","properties":{"path":{"type":"string"},"timeout_ms":{"type":"integer"},"max_file_bytes":{"type":"integer"},"link_only":{"type":"boolean"}},"required":["path","timeout_ms","max_file_bytes"]},"result_schema":{},"input_mapping":{"mappings":names.iter().map(|name|json!({"interface_param":name,"source":{"kind":"mcp_argument","path":name},"required":*name!="link_only"})).collect::<Vec<_>>()},"output_mapping":{"resource_read":true,"resource_read_arguments":{"link_only":false},"mcp_content":[{"type":"resource_link","uri_path":"/uri","name_path":"/name","mime_type_path":"/mime_type"},{"type":"resource","uri_path":"/uri","blob_path":"/data","mime_type_path":"/mime_type","skip_if_null":true}]},"permission_code":null,"risk_level":"low","status":"enabled"})).await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    let (status, value) = request(
        &app,
        "POST",
        "/api/console/mcp/instances/terminal_fixture/tool-bindings",
        &cookie,
        &csrf,
        json!({"group_path":"/","tool_id":"terminal_fixture_read","visible":true,"sort_order":0}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{value}");
    let uri = extension_contracts::ResourceReference {
        v: 1,
        user_id: actor.user_id,
        workspace_id: actor.current_workspace_id,
        instance_id: "terminal_fixture".into(),
        tool_id: "terminal_fixture_read".into(),
        des_id: "terminalfixturev1".into(),
        arguments: json!({"path":source,"timeout_ms":5000,"max_file_bytes":1024,"link_only":true}),
        resource_uri: url::Url::from_file_path(&source).unwrap().to_string(),
    }
    .encode()
    .unwrap();
    let import_body = json!({"resource_uri":uri,"file_table_id":table_id});
    let (status, imported) = request(
        &app,
        "POST",
        "/api/console/terminal/files/import-resource",
        &cookie,
        &csrf,
        import_body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{imported}");
    assert_eq!(imported["record"]["filename"], "import.bin");
    let content = control_plane::file_management::FileAccessService::new(
        state.store.clone(),
        state.file_storage_registry.clone(),
        state.runtime_engine.clone(),
        state.infrastructure.ephemeral_kv_store(),
    )
    .read(
        actor.clone(),
        table_id,
        Uuid::parse_str(imported["record"]["id"].as_str().unwrap()).unwrap(),
    )
    .await
    .unwrap();
    use tokio::io::AsyncReadExt;
    let mut reader = content.reader.into_inner().unwrap();
    let mut imported_bytes = Vec::new();
    reader.read_to_end(&mut imported_bytes).await.unwrap();
    assert_eq!(imported_bytes, b"resource import bytes\0");
    sqlx::query("delete from scope_data_model_grants where scope_kind = 'workspace' and scope_id = $1 and data_model_id = $2").bind(actor.current_workspace_id).bind(Uuid::parse_str(table["data"]["model_definition_id"].as_str().unwrap()).unwrap()).execute(state.store.pool()).await.unwrap();
    let denied_destination = temp.0.join("denied.bin");
    let mut denied_body = body;
    denied_body["target_path"] = json!(denied_destination);
    let (status, value) = request(
        &app,
        "POST",
        "/api/console/terminal/files/download-file",
        &cookie,
        &csrf,
        denied_body,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
    assert!(!denied_destination.exists());
    let (status, value) = request(
        &app,
        "POST",
        "/api/console/terminal/files/import-resource",
        &cookie,
        &csrf,
        import_body,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
}
