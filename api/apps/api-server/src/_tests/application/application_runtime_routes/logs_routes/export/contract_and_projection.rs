use super::*;

#[tokio::test]
async fn application_runtime_routes_logs_archive_import_rejects_tampered_contract_digests() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let source_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "tampered archive").await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &source_run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let before_logs = list_run_logs(&app, &cookie, &application_id).await;
    let before_total = before_logs["data"]["total"].as_i64().unwrap();
    let (export_status, archive_bytes) =
        get_run_archive(&app, &cookie, &application_id, &source_run_id, 1).await;
    assert_eq!(export_status, StatusCode::OK);

    let tampered_cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "archive_checksum",
            tamper_run_archive_bytes(&archive_bytes, |archive| {
                archive["content_digest"] = json!(
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                );
            }),
        ),
        (
            "archive_entry_digest",
            tamper_run_archive_bytes(&archive_bytes, |archive| {
                archive["manifest"]["entries"][0]["content_digest"] = json!(
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                );
            }),
        ),
        (
            "archive_content_sha256",
            tamper_run_archive_bytes(&archive_bytes, |archive| {
                archive["entries"][0]["flow_run_fact"]["debug_session_id"] =
                    json!("tampered-debug-session");
            }),
        ),
    ];

    for (expected_code, tampered_bytes) in tampered_cases {
        let (session_status, session_payload) = create_archive_upload_session(
            &app,
            &cookie,
            &csrf,
            &application_id,
            &tampered_bytes,
            &sha256_bytes_for_test(&tampered_bytes),
        )
        .await;
        assert_eq!(session_status, StatusCode::CREATED, "{}", session_payload);
        let session_id = session_payload["data"]["session_id"].as_str().unwrap();
        let (chunk_status, chunk_payload) = upload_archive_chunk(
            &app,
            &cookie,
            &csrf,
            &application_id,
            session_id,
            0,
            &tampered_bytes,
        )
        .await;
        assert_eq!(chunk_status, StatusCode::OK, "{}", chunk_payload);

        let (complete_status, complete_payload) =
            complete_archive_upload_session(&app, &cookie, &csrf, &application_id, session_id)
                .await;
        assert_eq!(
            complete_status,
            StatusCode::BAD_REQUEST,
            "expected {expected_code}, got {}",
            complete_payload
        );
        assert_eq!(complete_payload["code"], json!(expected_code));
    }

    let after_logs = list_run_logs(&app, &cookie, &application_id).await;
    assert_eq!(after_logs["data"]["total"].as_i64().unwrap(), before_total);
}

#[tokio::test]
async fn application_runtime_routes_logs_archive_rejects_unsupported_version() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let run_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "unsupported archive version",
    )
    .await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let (status, body) = get_run_archive(&app, &cookie, &application_id, &run_id, 2).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["code"], json!("unsupported_archive_version"));

    let (post_status, post_body) =
        post_run_archive_with_version(&app, &cookie, &csrf, &application_id, &[run_id.as_str()], 2)
            .await;
    assert_eq!(
        post_status,
        StatusCode::BAD_REQUEST,
        "{}",
        String::from_utf8_lossy(&post_body)
    );
    let post_payload: Value = serde_json::from_slice(&post_body).unwrap();
    assert_eq!(post_payload["code"], json!("unsupported_archive_version"));
}

#[tokio::test]
async fn application_runtime_routes_logs_export_selected_runs_zip_uses_csrf_and_selected_order() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let first_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "first selected").await;
    let second_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "second selected").await;
    let unselected_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "not selected").await;
    for run_id in [&first_run_id, &second_run_id, &unselected_run_id] {
        wait_for_run_detail(
            &app,
            &cookie,
            &application_id,
            run_id,
            &["succeeded", "failed", "cancelled"],
        )
        .await;
    }

    let body = json!({
        "run_ids": [second_run_id, first_run_id]
    });
    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/export"
                ))
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/export"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "run_ids": [second_run_id, first_run_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert!(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/zip")),
        "selected export should return a zip archive"
    );

    let entries = read_zip_entries(&body);
    assert_eq!(entries.len(), 3, "manifest + two selected run dumps");
    assert_eq!(entries[0].0, "manifest.json");
    assert!(entries[1].0.starts_with("runs/001_"));
    assert!(entries[1].0.ends_with(".json"));
    assert!(entries[2].0.starts_with("runs/002_"));
    assert!(entries[2].0.ends_with(".json"));

    let manifest: Value = serde_json::from_slice(&entries[0].1).unwrap();
    assert_eq!(manifest["export_version"], json!(1));
    assert_eq!(manifest["export_status"], json!("complete"));
    assert_eq!(manifest["run_count"], json!(2));
    assert_eq!(
        manifest["selected_run_ids"],
        json!([second_run_id.as_str(), first_run_id.as_str()])
    );
    assert_eq!(
        manifest["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|run| run["run_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![second_run_id.as_str(), first_run_id.as_str()]
    );
    assert_eq!(manifest["entries"][0]["filename"], json!(entries[1].0));
    assert_eq!(manifest["entries"][1]["filename"], json!(entries[2].0));

    let second_dump: Value = serde_json::from_slice(&entries[1].1).unwrap();
    let first_dump: Value = serde_json::from_slice(&entries[2].1).unwrap();
    assert_eq!(second_dump["run"]["id"], json!(second_run_id));
    assert_eq!(first_dump["run"]["id"], json!(first_run_id));
    assert!(
        !String::from_utf8_lossy(&body).contains(&unselected_run_id),
        "zip archive must not include unselected runs"
    );
}

#[tokio::test]
async fn application_runtime_routes_logs_export_keeps_shape_when_projection_failed() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let run_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "projection failed export",
    )
    .await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let application_uuid = Uuid::parse_str(&application_id).unwrap();
    let run_uuid = Uuid::parse_str(&run_id).unwrap();
    let source_watermark =
        <MainDurableStore as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
            &state.store,
            application_uuid,
            run_uuid,
        )
        .await
        .unwrap()
        .unwrap();
    <MainDurableStore as OrchestrationRuntimeRepository>::upsert_application_run_trace_projection_status(
        &state.store,
        &UpsertApplicationRunTraceProjectionStatusInput {
            flow_run_id: run_uuid,
            projection_version: control_plane::orchestration_runtime::trace_projection::APPLICATION_RUN_TRACE_PROJECTION_VERSION,
            status: domain::ApplicationRunTraceProjectionStatus::Failed,
            source_watermark,
            attempt_count: 2,
            last_attempt_at: Some(time::OffsetDateTime::now_utc()),
            last_success_at: None,
            diagnostic: Some(domain::ApplicationRunTraceProjectionDiagnostic {
                last_error_code: Some("fixture_projection_failed".to_string()),
                last_error_stage: Some("test".to_string()),
                last_error_source_kind: Some("trace_projection".to_string()),
                last_error_source_locator: Some(run_id.clone()),
                last_error_message: Some("projection failed in fixture".to_string()),
                last_error_ref: Some("fixture-error-ref".to_string()),
                retriable: true,
            }),
        },
    )
    .await
    .unwrap();

    let (status, _, body) = get_run_export(&app, &cookie, &application_id, &run_id).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let dump: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(dump["run"]["id"], json!(run_id));
    assert!(dump["statistics"].is_object());
    assert!(dump["detail"].is_object());
    assert!(dump["flow_run"].is_object());
    assert!(dump["node_runs"].as_array().is_some());
    assert_eq!(
        dump["trace_tree"]["projection_status"]["projection_status"],
        json!("failed")
    );
    assert_eq!(dump["trace_tree"]["nodes"], json!([]));
}

#[tokio::test]
async fn application_runtime_routes_logs_archive_import_accepts_zip_format() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let source_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "zip import test").await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &source_run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    // Export as JSON
    let (export_status, archive_json_bytes) =
        get_run_archive(&app, &cookie, &application_id, &source_run_id, 1).await;
    assert_eq!(
        export_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&archive_json_bytes)
    );

    // Wrap the JSON in a ZIP file with archive.json
    use std::io::Write;
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip_writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip_writer.start_file("archive.json", options).unwrap();
    zip_writer.write_all(&archive_json_bytes).unwrap();
    let archive_zip_bytes = zip_writer.finish().unwrap().into_inner();

    // Import the ZIP file
    let job = import_archive_bytes(&app, &cookie, &csrf, &application_id, &archive_zip_bytes).await;
    assert_eq!(job["data"]["status"], json!("succeeded"), "{job}");
    assert_eq!(job["data"]["imported_run_count"], json!(1));

    let target_run_id = job["data"]["source_to_target_run_ids"][0]["target_run_id"]
        .as_str()
        .unwrap();
    assert_ne!(target_run_id, source_run_id);

    // Verify the imported run
    let (overview_status, overview_payload) =
        get_run_overview(&app, &cookie, &application_id, target_run_id).await;
    assert_eq!(overview_status, StatusCode::OK, "{}", overview_payload);
    assert_eq!(
        overview_payload["data"]["flow_run"]["input_payload"]["node-start"]["query"],
        json!("zip import test")
    );
}
