use super::*;
use control_plane::ports::{UpdateNodeRunInput, UpsertApplicationRunTraceProjectionStatusInput};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};

async fn get_run_export(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/export"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
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

    (status, headers, body)
}

async fn get_run_archive(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    archive_version: i32,
) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/archive?archive_version={archive_version}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();

    (status, body)
}

async fn post_run_archive(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    run_ids: &[&str],
) -> (StatusCode, Vec<u8>) {
    post_run_archive_with_version(app, cookie, csrf, application_id, run_ids, 1).await
}

async fn post_run_archive_with_version(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    run_ids: &[&str],
    archive_version: i32,
) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/archive"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "archive_version": archive_version,
                        "run_ids": run_ids
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();

    (status, body)
}

async fn create_archive_upload_session(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    archive_bytes: &[u8],
    expected_sha256: &str,
) -> (StatusCode, Value) {
    create_archive_upload_session_with_chunk_size(
        app,
        cookie,
        csrf,
        application_id,
        archive_bytes,
        expected_sha256,
        archive_bytes.len(),
    )
    .await
}

async fn create_archive_upload_session_with_chunk_size(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    archive_bytes: &[u8],
    expected_sha256: &str,
    chunk_size_bytes: usize,
) -> (StatusCode, Value) {
    create_archive_upload_session_from_payload(
        app,
        Some(cookie),
        Some(csrf),
        application_id,
        json!({
            "filename": "archive.json",
            "total_size_bytes": archive_bytes.len(),
            "expected_sha256": expected_sha256,
            "chunk_size_bytes": chunk_size_bytes
        }),
    )
    .await
}

async fn create_archive_upload_session_from_payload(
    app: &axum::Router,
    cookie: Option<&str>,
    csrf: Option<&str>,
    application_id: &str,
    payload: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/console/applications/{application_id}/logs/runs/archive/import-sessions"
        ))
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(payload.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn upload_archive_chunk(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    session_id: &str,
    chunk_index: usize,
    chunk: &[u8],
) -> (StatusCode, Value) {
    upload_archive_chunk_with_headers(UploadArchiveChunkRequest {
        app,
        cookie: Some(cookie),
        csrf: Some(csrf),
        application_id,
        session_id,
        chunk_index,
        chunk,
        chunk_sha256: Some(&sha256_bytes_for_test(chunk)),
    })
    .await
}

struct UploadArchiveChunkRequest<'a> {
    app: &'a axum::Router,
    cookie: Option<&'a str>,
    csrf: Option<&'a str>,
    application_id: &'a str,
    session_id: &'a str,
    chunk_index: usize,
    chunk: &'a [u8],
    chunk_sha256: Option<&'a str>,
}

async fn upload_archive_chunk_with_headers(
    input: UploadArchiveChunkRequest<'_>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method("PUT").uri(format!(
        "/api/console/applications/{}/logs/runs/archive/import-sessions/{}/chunks/{}",
        input.application_id, input.session_id, input.chunk_index
    ));
    if let Some(cookie) = input.cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = input.csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    if let Some(chunk_sha256) = input.chunk_sha256 {
        builder = builder.header("x-chunk-sha256", chunk_sha256);
    }
    let response = input
        .app
        .clone()
        .oneshot(builder.body(Body::from(input.chunk.to_vec())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn complete_archive_upload_session(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    session_id: &str,
) -> (StatusCode, Value) {
    complete_archive_upload_session_with_csrf(app, cookie, Some(csrf), application_id, session_id)
        .await
}

async fn complete_archive_upload_session_with_csrf(
    app: &axum::Router,
    cookie: &str,
    csrf: Option<&str>,
    application_id: &str,
    session_id: &str,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method("POST").uri(format!(
        "/api/console/applications/{application_id}/logs/runs/archive/import-sessions/{session_id}/complete"
    ));
    builder = builder.header("cookie", cookie);
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn get_archive_import_job(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    job_id: &str,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/archive/import-jobs/{job_id}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn wait_for_archive_import_job(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    job_id: &str,
) -> Value {
    let mut last_payload = json!({});
    for _ in 0..80 {
        let (status, payload) = get_archive_import_job(app, cookie, application_id, job_id).await;
        assert_eq!(status, StatusCode::OK, "{payload}");
        let job_status = payload["data"]["status"].as_str().unwrap_or_default();
        if matches!(job_status, "succeeded" | "failed") {
            return payload;
        }
        last_payload = payload;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("archive import job did not finish: {last_payload}");
}

async fn import_archive_bytes(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    archive_bytes: &[u8],
) -> Value {
    let archive_sha256 = sha256_bytes_for_test(archive_bytes);
    let (session_status, session_payload) = create_archive_upload_session(
        app,
        cookie,
        csrf,
        application_id,
        archive_bytes,
        &archive_sha256,
    )
    .await;
    assert_eq!(session_status, StatusCode::CREATED, "{}", session_payload);
    let session_id = session_payload["data"]["session_id"].as_str().unwrap();
    let split_at = archive_bytes.len();
    for (chunk_index, chunk) in archive_bytes.chunks(split_at).enumerate() {
        let (chunk_status, chunk_payload) = upload_archive_chunk(
            app,
            cookie,
            csrf,
            application_id,
            session_id,
            chunk_index,
            chunk,
        )
        .await;
        assert_eq!(chunk_status, StatusCode::OK, "{}", chunk_payload);
    }

    let (complete_status, complete_payload) =
        complete_archive_upload_session(app, cookie, csrf, application_id, session_id).await;
    assert_eq!(complete_status, StatusCode::OK, "{}", complete_payload);
    assert!(
        matches!(
            complete_payload["data"]["status"].as_str(),
            Some("queued" | "processing" | "succeeded" | "failed")
        ),
        "{complete_payload}"
    );
    let job_id = complete_payload["data"]["job_id"].as_str().unwrap();
    wait_for_archive_import_job(app, cookie, application_id, job_id).await
}

async fn get_run_overview(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/overview"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn list_run_logs(app: &axum::Router, cookie: &str, application_id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs?time_range_days=30&page_size=100"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn count_archive_upload_chunks(pool: &sqlx::PgPool, session_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        r#"
        select count(*)::bigint
        from run_archive_upload_chunks
        where session_id = $1
        "#,
    )
    .bind(Uuid::parse_str(session_id).unwrap())
    .fetch_one(pool)
    .await
    .unwrap()
}

fn sha256_bytes_for_test(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn read_zip_entries(body: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(body)).unwrap();
    let mut entries = Vec::with_capacity(archive.len());

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        entries.push((file.name().to_string(), contents));
    }

    entries
}

fn tamper_run_archive_bytes<F>(archive_bytes: &[u8], mut tamper: F) -> Vec<u8>
where
    F: FnMut(&mut Value),
{
    let mut archive: Value = serde_json::from_slice(archive_bytes).unwrap();
    tamper(&mut archive);
    serde_json::to_vec_pretty(&archive).unwrap()
}

async fn wait_for_node_run_error_code(
    pool: &sqlx::PgPool,
    node_run_id: Uuid,
    expected_code: &str,
) -> Value {
    let mut last_payload = Value::Null;
    for _ in 0..200 {
        let payload = sqlx::query_scalar::<_, Value>(
            r#"
            select coalesce(error_payload, 'null'::jsonb)
            from node_runs
            where id = $1
              and status = 'failed'
            "#,
        )
        .bind(node_run_id)
        .fetch_optional(pool)
        .await
        .unwrap();

        if let Some(payload) = payload {
            if payload.get("code").and_then(Value::as_str) == Some(expected_code) {
                return payload;
            }
            last_payload = payload;
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!(
        "timed out waiting for node run {node_run_id} failed error code {expected_code}, last payload: {last_payload}"
    );
}

#[tokio::test]
async fn application_runtime_routes_logs_export_json_trace_dump_preserves_detail_and_error_payload()
{
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "导出失败节点错误").await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let run_uuid = Uuid::parse_str(&run_id).unwrap();
    let llm_node_run_id = latest_llm_node_run_id(&pool, run_uuid).await;
    <MainDurableStore as OrchestrationRuntimeRepository>::update_node_run(
        &state.store,
        &UpdateNodeRunInput {
            node_run_id: llm_node_run_id,
            status: domain::NodeRunStatus::Failed,
            output_payload: json!({ "text": "partial answer before failure" }),
            error_payload: Some(json!({
                "code": "provider_failed",
                "message": "fixture provider failure"
            })),
            metrics_payload: json!({ "usage": { "total_tokens": 12 } }),
            debug_payload: json!({
                "provider": "fixture_provider",
                "request_id": "req-export-failed-node"
            }),
            finished_at: Some(time::OffsetDateTime::now_utc()),
        },
    )
    .await
    .unwrap();
    let expected_error_payload =
        wait_for_node_run_error_code(&pool, llm_node_run_id, "provider_failed").await;

    let (status, headers, body) = get_run_export(&app, &cookie, &application_id, &run_id).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert!(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json")),
        "export should return a JSON download"
    );
    let disposition = headers
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(disposition.contains("attachment"));
    assert!(
        disposition.contains(".json"),
        "single-run export filename should end with .json: {disposition}"
    );

    let dump: Value = serde_json::from_slice(&body).unwrap();
    for field in [
        "export_version",
        "exported_at",
        "export_status",
        "export_warnings",
        "run",
        "statistics",
        "detail",
        "flow_run",
        "node_runs",
        "trace_tree",
    ] {
        assert!(
            dump.get(field).is_some(),
            "export dump must include root field {field}"
        );
    }
    assert_eq!(dump["run"]["id"], json!(run_id));
    assert_eq!(dump["flow_run"]["id"], json!(run_id));
    assert_eq!(dump["export_version"], json!(1));
    assert_eq!(dump["export_status"], json!("complete"));
    assert!(dump["exported_at"].is_string());
    assert!(dump["export_warnings"].as_array().is_some());
    assert!(dump["trace_tree"]["projection_status"]["projection_status"].is_string());
    assert!(dump["trace_tree"]["nodes"].as_array().is_some());

    let failed_node = dump["node_runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node_run| node_run["id"] == json!(llm_node_run_id.to_string()))
        .expect("failed node run should be exported");
    let failed_node_object = failed_node.as_object().unwrap();
    for field in [
        "input_payload",
        "input_payload_view",
        "debug_payload",
        "output_payload",
        "error_payload",
        "metrics_payload",
    ] {
        assert!(
            failed_node_object.contains_key(field),
            "node_runs[] must preserve NodeRunResponse field {field}"
        );
    }
    assert!(
        !failed_node_object.contains_key("summary"),
        "node_runs[] must not wrap NodeRunResponse in a summary object"
    );
    assert_eq!(
        failed_node["error_payload"], expected_error_payload,
        "failed node error payload should be exported"
    );
}

#[tokio::test]
async fn application_runtime_routes_logs_archive_returns_v1_manifest_and_restore_facts() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let large_query = format!("run archive full payload {}", "X".repeat(3_000));
    let run_id = start_full_debug_run(&app, &cookie, &csrf, &application_id, &large_query).await;
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;

    let (status, body) = get_run_archive(&app, &cookie, &application_id, &run_id, 1).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let archive: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(archive["archive_version"], json!(1));
    assert_eq!(archive["manifest"]["archive_version"], json!(1));
    assert_eq!(
        archive["manifest"]["archive_semantics"],
        json!("application_run_archive_v1")
    );
    assert_eq!(archive["manifest"]["run_count"], json!(1));
    assert_eq!(
        archive["manifest"]["selected_run_ids"],
        json!([run_id.as_str()])
    );
    assert_eq!(archive["source"]["application_id"], json!(application_id));
    assert_eq!(archive["source"]["source_kind"], json!("application_run"));
    assert!(archive["source"]["workspace_id"].is_string());
    assert!(archive["source"]["exported_by_user_id"].is_string());
    assert!(archive["exported_at"].is_string());
    assert!(archive["manifest"]["content_sha256"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert!(archive["manifest"]["checksum"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert!(archive["content_digest"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        archive["content_digest"], archive["manifest"]["content_sha256"],
        "root content_digest must match manifest content_sha256"
    );
    assert_eq!(
        archive["manifest"]["checksum"], archive["manifest"]["content_sha256"],
        "manifest checksum must match manifest content_sha256"
    );
    assert_eq!(
        archive["manifest"]["entries"][0]["source_run_id"],
        json!(run_id)
    );
    assert!(archive["manifest"]["entries"][0]["content_sha256"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert!(archive["manifest"]["entries"][0]["content_digest"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));

    let entries = archive["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry["source_run_id"], json!(run_id));
    assert!(entry["content_digest"]
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        entry["content_digest"], archive["manifest"]["entries"][0]["content_digest"],
        "entry content_digest must match manifest entry content_digest"
    );
    assert_eq!(entry["flow_run"]["id"], json!(run_id));
    assert_eq!(
        entry["flow_run"]["input_payload"]["node-start"]["query"],
        json!(large_query)
    );
    assert!(entry["flow_run_fact"]["debug_session_id"].is_string());
    assert!(entry["compiled_plan"].is_object());
    assert!(entry["node_runs"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(entry["events"].as_array().is_some());
    assert!(entry["checkpoints"].as_array().is_some());
    assert!(entry["callback_tasks"].as_array().is_some());
    assert!(entry["runtime_spans"].as_array().is_some());
    assert!(entry["runtime_items"].as_array().is_some());
    assert!(entry["context_projections"].as_array().is_some());
    assert!(entry["model_failover_attempts"].as_array().is_some());
    assert!(entry["capability_invocations"].as_array().is_some());
    assert!(entry["runtime_events"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(entry["usage_ledger"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(entry["usage_ledger"][0]["id"].is_string());
    assert!(entry["usage_ledger"][0]["usage_status"].is_string());
    assert!(entry["usage_ledger"][0]["raw_usage"].is_object());
    assert!(entry["usage_ledger"][0]["normalized_usage"].is_object());
    assert!(entry["trace_tree"]["projection_status"]["projection_version"].is_number());
    assert!(entry["trace_tree"]["nodes"].as_array().is_some());
    assert!(entry["export_warnings"].as_array().is_some());

    let (multi_status, multi_body) =
        post_run_archive(&app, &cookie, &csrf, &application_id, &[run_id.as_str()]).await;
    assert_eq!(
        multi_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&multi_body)
    );
    let multi_archive: Value = serde_json::from_slice(&multi_body).unwrap();
    assert_eq!(multi_archive["archive_version"], json!(1));
    assert_eq!(multi_archive["manifest"]["run_count"], json!(1));
    assert_eq!(multi_archive["entries"][0]["source_run_id"], json!(run_id));
    assert_eq!(
        multi_archive["manifest"]["entries"][0]["content_sha256"],
        archive["manifest"]["entries"][0]["content_sha256"],
        "single-run and multi-run archive endpoints must use the same entry builder"
    );
}

mod archive_import;
mod contract_and_projection;
