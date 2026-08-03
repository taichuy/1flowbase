use super::*;

#[tokio::test]
async fn application_runtime_routes_logs_archive_import_restores_visible_target_runs() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let query = "archive import round trip";
    let source_run_id = start_full_debug_run(&app, &cookie, &csrf, &application_id, query).await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &source_run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let (export_status, archive_bytes) =
        get_run_archive(&app, &cookie, &application_id, &source_run_id, 1).await;
    assert_eq!(
        export_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&archive_bytes)
    );

    let first_job =
        import_archive_bytes(&app, &cookie, &csrf, &application_id, &archive_bytes).await;
    assert_eq!(
        first_job["data"]["status"],
        json!("succeeded"),
        "{first_job}"
    );
    assert_eq!(first_job["data"]["imported_run_count"], json!(1));
    let first_target_run_id = first_job["data"]["source_to_target_run_ids"][0]["target_run_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(first_target_run_id, source_run_id);

    let (overview_status, overview_payload) =
        get_run_overview(&app, &cookie, &application_id, &first_target_run_id).await;
    assert_eq!(overview_status, StatusCode::OK, "{}", overview_payload);
    assert_eq!(
        overview_payload["data"]["flow_run"]["id"],
        json!(first_target_run_id)
    );
    assert_eq!(
        overview_payload["data"]["flow_run"]["input_payload"]["node-start"]["query"],
        json!(query)
    );

    let logs_payload = list_run_logs(&app, &cookie, &application_id).await;
    let listed_run_ids = logs_payload["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(
        listed_run_ids.contains(&first_target_run_id.as_str()),
        "imported target run should be visible in official logs"
    );

    let second_job =
        import_archive_bytes(&app, &cookie, &csrf, &application_id, &archive_bytes).await;
    assert_eq!(
        second_job["data"]["status"],
        json!("succeeded"),
        "{second_job}"
    );
    let second_target_run_id = second_job["data"]["source_to_target_run_ids"][0]["target_run_id"]
        .as_str()
        .unwrap();
    assert_ne!(
        second_target_run_id, first_target_run_id,
        "repeat import must create a fresh target run"
    );
}

#[tokio::test]
async fn application_runtime_routes_logs_archive_import_rejects_checksum_mismatch() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let source_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "checksum mismatch").await;
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
    let (session_status, session_payload) = create_archive_upload_session(
        &app,
        &cookie,
        &csrf,
        &application_id,
        &archive_bytes,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
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
        &archive_bytes,
    )
    .await;
    assert_eq!(chunk_status, StatusCode::OK, "{}", chunk_payload);

    let (complete_status, complete_payload) =
        complete_archive_upload_session(&app, &cookie, &csrf, &application_id, session_id).await;
    assert_eq!(
        complete_status,
        StatusCode::BAD_REQUEST,
        "{}",
        complete_payload
    );
    assert_eq!(complete_payload["code"], json!("archive_sha256"));
    let after_logs = list_run_logs(&app, &cookie, &application_id).await;
    assert_eq!(after_logs["data"]["total"].as_i64().unwrap(), before_total);
}

#[tokio::test]
async fn application_runtime_routes_logs_archive_upload_enforces_checksum_limits_and_cleanup() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let other_application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let source_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "upload staging").await;
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
    let archive_sha256 = sha256_bytes_for_test(&archive_bytes);
    let archive_len = archive_bytes.len();

    let valid_payload = json!({
        "filename": "archive.json",
        "total_size_bytes": archive_len,
        "expected_sha256": archive_sha256.clone(),
        "chunk_size_bytes": archive_len
    });
    let (missing_csrf_status, _) = create_archive_upload_session_from_payload(
        &app,
        Some(&cookie),
        None,
        &application_id,
        valid_payload.clone(),
    )
    .await;
    assert_eq!(missing_csrf_status, StatusCode::UNAUTHORIZED);

    let missing_session_id = Uuid::now_v7().to_string();
    let (missing_session_upload_status, _) = upload_archive_chunk(
        &app,
        &cookie,
        &csrf,
        &application_id,
        &missing_session_id,
        0,
        &archive_bytes,
    )
    .await;
    assert_eq!(missing_session_upload_status, StatusCode::NOT_FOUND);
    let (missing_session_complete_status, _) =
        complete_archive_upload_session(&app, &cookie, &csrf, &application_id, &missing_session_id)
            .await;
    assert_eq!(missing_session_complete_status, StatusCode::NOT_FOUND);

    for (payload, expected_code) in [
        (
            json!({
                "filename": "archive.json",
                "total_size_bytes": archive_len,
                "chunk_size_bytes": archive_len
            }),
            "expected_sha256",
        ),
        (
            json!({
                "filename": "archive.json",
                "total_size_bytes": archive_len,
                "expected_sha256": archive_sha256.clone()
            }),
            "chunk_size_bytes",
        ),
        (
            json!({
                "filename": "archive.json",
                "total_size_bytes": 104857601_i64,
                "expected_sha256": archive_sha256.clone(),
                "chunk_size_bytes": 1024
            }),
            "archive_size",
        ),
        (
            json!({
                "filename": "archive.json",
                "total_size_bytes": archive_len,
                "expected_sha256": "not-a-sha",
                "chunk_size_bytes": archive_len
            }),
            "expected_sha256",
        ),
    ] {
        let (status, payload) = create_archive_upload_session_from_payload(
            &app,
            Some(&cookie),
            Some(&csrf),
            &application_id,
            payload,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{payload}");
        assert_eq!(payload["code"], json!(expected_code));
    }

    let (session_status, session_payload) = create_archive_upload_session_from_payload(
        &app,
        Some(&cookie),
        Some(&csrf),
        &application_id,
        valid_payload,
    )
    .await;
    assert_eq!(session_status, StatusCode::CREATED, "{session_payload}");
    let session_id = session_payload["data"]["session_id"].as_str().unwrap();

    let (other_application_upload_status, _) = upload_archive_chunk(
        &app,
        &cookie,
        &csrf,
        &other_application_id,
        session_id,
        0,
        &archive_bytes,
    )
    .await;
    assert_eq!(other_application_upload_status, StatusCode::NOT_FOUND);
    let (other_application_complete_status, _) =
        complete_archive_upload_session(&app, &cookie, &csrf, &other_application_id, session_id)
            .await;
    assert_eq!(other_application_complete_status, StatusCode::NOT_FOUND);

    let (missing_upload_csrf_status, _) =
        upload_archive_chunk_with_headers(UploadArchiveChunkRequest {
            app: &app,
            cookie: Some(&cookie),
            csrf: None,
            application_id: &application_id,
            session_id,
            chunk_index: 0,
            chunk: &archive_bytes,
            chunk_sha256: Some(&sha256_bytes_for_test(&archive_bytes)),
        })
        .await;
    assert_eq!(missing_upload_csrf_status, StatusCode::UNAUTHORIZED);

    let (missing_chunk_sha_status, missing_chunk_sha_payload) =
        upload_archive_chunk_with_headers(UploadArchiveChunkRequest {
            app: &app,
            cookie: Some(&cookie),
            csrf: Some(&csrf),
            application_id: &application_id,
            session_id,
            chunk_index: 0,
            chunk: &archive_bytes,
            chunk_sha256: None,
        })
        .await;
    assert_eq!(
        missing_chunk_sha_status,
        StatusCode::BAD_REQUEST,
        "{missing_chunk_sha_payload}"
    );
    assert_eq!(missing_chunk_sha_payload["code"], json!("chunk_sha256"));

    let (wrong_chunk_sha_status, wrong_chunk_sha_payload) =
        upload_archive_chunk_with_headers(UploadArchiveChunkRequest {
            app: &app,
            cookie: Some(&cookie),
            csrf: Some(&csrf),
            application_id: &application_id,
            session_id,
            chunk_index: 0,
            chunk: &archive_bytes,
            chunk_sha256: Some(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            ),
        })
        .await;
    assert_eq!(
        wrong_chunk_sha_status,
        StatusCode::BAD_REQUEST,
        "{wrong_chunk_sha_payload}"
    );
    assert_eq!(wrong_chunk_sha_payload["code"], json!("chunk_sha256"));

    let (overflow_index_status, overflow_index_payload) = upload_archive_chunk(
        &app,
        &cookie,
        &csrf,
        &application_id,
        session_id,
        1,
        &archive_bytes,
    )
    .await;
    assert_eq!(
        overflow_index_status,
        StatusCode::BAD_REQUEST,
        "{overflow_index_payload}"
    );
    assert_eq!(overflow_index_payload["code"], json!("archive_chunk_count"));

    let (chunk_status, chunk_payload) = upload_archive_chunk(
        &app,
        &cookie,
        &csrf,
        &application_id,
        session_id,
        0,
        &archive_bytes,
    )
    .await;
    assert_eq!(chunk_status, StatusCode::OK, "{chunk_payload}");
    assert_eq!(
        list_run_logs(&app, &cookie, &application_id).await["data"]["total"],
        json!(before_total),
        "uploaded archive should remain staging-only before complete/import"
    );

    let (missing_complete_csrf_status, _) =
        complete_archive_upload_session_with_csrf(&app, &cookie, None, &application_id, session_id)
            .await;
    assert_eq!(missing_complete_csrf_status, StatusCode::UNAUTHORIZED);

    let (complete_status, complete_payload) =
        complete_archive_upload_session(&app, &cookie, &csrf, &application_id, session_id).await;
    assert_eq!(complete_status, StatusCode::OK, "{complete_payload}");
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    assert_eq!(count_archive_upload_chunks(&pool, session_id).await, 0);

    let split_at = archive_bytes.len().div_ceil(2);
    let chunked_payload = json!({
        "filename": "archive.json",
        "total_size_bytes": archive_len,
        "expected_sha256": archive_sha256,
        "chunk_size_bytes": split_at
    });
    let (chunked_session_status, chunked_session_payload) =
        create_archive_upload_session_from_payload(
            &app,
            Some(&cookie),
            Some(&csrf),
            &application_id,
            chunked_payload,
        )
        .await;
    assert_eq!(
        chunked_session_status,
        StatusCode::CREATED,
        "{chunked_session_payload}"
    );
    let chunked_session_id = chunked_session_payload["data"]["session_id"]
        .as_str()
        .unwrap();
    let first_chunk = &archive_bytes[..split_at];
    let second_chunk = &archive_bytes[split_at..];
    let (second_chunk_status, second_chunk_payload) = upload_archive_chunk(
        &app,
        &cookie,
        &csrf,
        &application_id,
        chunked_session_id,
        1,
        second_chunk,
    )
    .await;
    assert_eq!(
        second_chunk_status,
        StatusCode::OK,
        "{second_chunk_payload}"
    );
    let (missing_first_complete_status, missing_first_complete_payload) =
        complete_archive_upload_session(&app, &cookie, &csrf, &application_id, chunked_session_id)
            .await;
    assert_eq!(
        missing_first_complete_status,
        StatusCode::BAD_REQUEST,
        "{missing_first_complete_payload}"
    );
    assert_eq!(
        missing_first_complete_payload["code"],
        json!("archive_chunks")
    );

    let (first_chunk_status, first_chunk_payload) = upload_archive_chunk(
        &app,
        &cookie,
        &csrf,
        &application_id,
        chunked_session_id,
        0,
        first_chunk,
    )
    .await;
    assert_eq!(first_chunk_status, StatusCode::OK, "{first_chunk_payload}");
    let (chunked_complete_status, chunked_complete_payload) =
        complete_archive_upload_session(&app, &cookie, &csrf, &application_id, chunked_session_id)
            .await;
    assert_eq!(
        chunked_complete_status,
        StatusCode::OK,
        "{chunked_complete_payload}"
    );
    assert_eq!(
        count_archive_upload_chunks(&pool, chunked_session_id).await,
        0
    );
}
