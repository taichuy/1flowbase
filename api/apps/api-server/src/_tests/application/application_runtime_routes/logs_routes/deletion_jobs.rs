use super::*;

async fn request(
    app: &axum::Router,
    id: &str,
    path: &str,
    method: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<Value>,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/console/applications/{id}/logs/{path}"));
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        builder = builder.header("x-csrf-token", csrf);
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    app.clone()
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn value(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
#[tokio::test]
async fn agent_logs_delete_jobs_console_auth_csrf_idempotent_start_and_query() {
    let (app, _) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"application_type":"agent_logs","name":"Job fixture","description":""})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = value(response).await;
    let id = created["data"]["id"].as_str().unwrap();
    let job_id = Uuid::now_v7();
    let input = json!({"job_id":job_id,"scope":{"mode":"all_time","batch_size":100}});
    for (path, method, body) in [
        (
            "deletion-preview".to_string(),
            "POST",
            Some(json!({"mode":"all_time"})),
        ),
        ("deletion-jobs".to_string(), "POST", Some(input.clone())),
        ("deletion-jobs/latest".to_string(), "GET", None),
        (format!("deletion-jobs/{job_id}"), "GET", None),
        (format!("deletion-jobs/{job_id}/stop"), "POST", None),
    ] {
        assert_eq!(
            request(&app, id, &path, method, None, None, body)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    for path in [
        "deletion-jobs".to_string(),
        format!("deletion-jobs/{job_id}/stop"),
    ] {
        assert_eq!(
            request(
                &app,
                id,
                &path,
                "POST",
                Some(&cookie),
                Some("invalid"),
                if path == "deletion-jobs" {
                    Some(input.clone())
                } else {
                    None
                }
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    let preview = request(
        &app,
        id,
        "deletion-preview",
        "POST",
        Some(&cookie),
        None,
        Some(json!({"mode":"all_time"})),
    )
    .await;
    assert_eq!(preview.status(), StatusCode::OK);
    assert_eq!(value(preview).await["data"]["total_records"], 0);
    assert!(value(
        request(
            &app,
            id,
            "deletion-jobs/latest",
            "GET",
            Some(&cookie),
            None,
            None
        )
        .await
    )
    .await["data"]["job"]
        .is_null());
    let first = request(
        &app,
        id,
        "deletion-jobs",
        "POST",
        Some(&cookie),
        Some(&csrf),
        Some(input.clone()),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first = value(first).await;
    assert_eq!(first["data"]["job"]["job_id"], job_id.to_string());
    assert_eq!(first["data"]["job"]["status"], "succeeded");
    let duplicate = request(
        &app,
        id,
        "deletion-jobs",
        "POST",
        Some(&cookie),
        Some(&csrf),
        Some(input),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::OK);
    assert_eq!(value(duplicate).await, first);
    for path in [
        "deletion-jobs/latest".to_string(),
        format!("deletion-jobs/{job_id}"),
    ] {
        let response = request(&app, id, &path, "GET", Some(&cookie), None, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            value(response).await["data"]["job"]["job_id"],
            job_id.to_string()
        );
    }
    let stop = request(
        &app,
        id,
        &format!("deletion-jobs/{job_id}/stop"),
        "POST",
        Some(&cookie),
        Some(&csrf),
        None,
    )
    .await;
    assert_eq!(stop.status(), StatusCode::OK);
    assert_eq!(value(stop).await["data"]["job"]["status"], "succeeded");
    let absent = request(
        &app,
        id,
        &format!("deletion-jobs/{}", Uuid::now_v7()),
        "GET",
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(absent.status(), StatusCode::NOT_FOUND);
    let missing_batch = request(
        &app,
        id,
        "deletion-jobs",
        "POST",
        Some(&cookie),
        Some(&csrf),
        Some(json!({"job_id":Uuid::now_v7(),"scope":{"mode":"all_time"}})),
    )
    .await;
    assert_eq!(missing_batch.status(), StatusCode::BAD_REQUEST);
}
