use super::*;

async fn deletion_request(
    app: &axum::Router,
    id: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<Value>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/console/applications/{id}/logs"));
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    app.clone()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn agent_logs_delete_console_route_requires_auth_csrf_scope_and_rejects_native() {
    let assembly = crate::routes::application_runtime::route_assembly();
    let bindings: Vec<_> = assembly
        .bindings()
        .iter()
        .filter(|binding| {
            binding.route.method == "DELETE"
                && binding.route.path == "/api/console/applications/:id/logs"
        })
        .collect();
    assert_eq!(bindings.len(), 1);
    assert_eq!(
        bindings[0].ownership,
        access_control::ConsoleRouteOwnership::ConsoleOperation(
            access_control::APPLICATIONS_LOGS_DELETE_OPERATION_ID.into()
        )
    );
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut ids = vec![];
    for application_type in ["agent_logs", "agent_flow"] {
        let response = app.clone().oneshot(Request::builder().method("POST").uri("/api/console/applications")
            .header("cookie",&cookie).header("x-csrf-token",&csrf).header("content-type","application/json")
            .body(Body::from(json!({"application_type":application_type,"name":"Deletion fixture","description":""}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        ids.push(value["data"]["id"].as_str().unwrap().to_owned());
    }
    let id = &ids[0];
    assert_eq!(
        deletion_request(&app, id, None, None, Some(json!({"mode":"all_time"})))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        deletion_request(
            &app,
            id,
            Some(&cookie),
            None,
            Some(json!({"mode":"all_time"}))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        deletion_request(
            &app,
            id,
            Some(&cookie),
            Some("invalid-csrf-token"),
            Some(json!({"mode":"all_time"}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for body in [
        json!({}),
        json!({"mode":"time_range"}),
        json!({"mode":"unknown"}),
        json!({"mode":"all_time","batch_size":0}),
        json!({"mode":"all_time","batch_size":-1}),
        json!({"mode":"all_time","batch_size":1.5}),
    ] {
        assert_eq!(
            deletion_request(&app, id, Some(&cookie), Some(&csrf), Some(body))
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert!(
        !deletion_request(&app, id, Some(&cookie), Some(&csrf), None)
            .await
            .status()
            .is_success()
    );
    for body in [
        json!({"mode":"all_time","ingested_at_before":"invalid"}),
        json!({"mode":"time_range","started_at_from":"bad","started_at_to":"2026-10-08T00:00:00Z"}),
        json!({"mode":"time_range","started_at_from":"2026-10-08T00:00:00Z","started_at_to":"2026-10-08T00:00:00Z"}),
    ] {
        let response = deletion_request(&app, id, Some(&cookie), Some(&csrf), Some(body)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let error: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(error["code"].is_string());
    }
    assert_eq!(
        deletion_request(
            &app,
            &ids[1],
            Some(&cookie),
            Some(&csrf),
            Some(json!({"mode":"all_time"}))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let pool = storage_durable_postgres::connect(&database_url)
        .await
        .unwrap();
    let application_id = Uuid::parse_str(id).unwrap();
    let scope: Uuid = sqlx::query_scalar("select scope_id from applications where id=$1")
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let store = MainDurableStore::new(pool.clone());
    let event:control_plane_contracts::ports::AgentLogEvent=serde_json::from_value(json!({"event_id":"delete-route-event","source_session_id":"session","source_task_id":"turn","sequence":0,"occurred_at":"2026-10-07T00:00:00Z","kind":"user","content":"fixture","raw":{}})).unwrap();
    control_plane::agent_logs::AgentLogsService::new(store.clone())
        .ingest(
            application_id,
            scope,
            Uuid::now_v7(),
            control_plane_contracts::ports::AgentLogsBatch {
                schema_version: control_plane_contracts::ports::AGENT_LOGS_SCHEMA_VERSION.into(),
                source_id: "route-fixture".into(),
                source_client: "codex".into(),
                events: vec![event],
            },
        )
        .await
        .unwrap();
    let response = deletion_request(
        &app,
        id,
        Some(&cookie),
        Some(&csrf),
        Some(json!({"mode":"all_time"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    assert_eq!(value["data"]["deleted_records"], 1);
    assert_eq!(value["data"]["has_more"], false);
    assert!(time::OffsetDateTime::parse(
        value["data"]["ingested_at_before"].as_str().unwrap(),
        &time::format_description::well_known::Rfc3339
    )
    .is_ok());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from application_run_log_tasks where application_id=$1"
        )
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from applications where id=any($1)")
            .bind(
                ids.iter()
                    .map(|id| Uuid::parse_str(id).unwrap())
                    .collect::<Vec<_>>()
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}
