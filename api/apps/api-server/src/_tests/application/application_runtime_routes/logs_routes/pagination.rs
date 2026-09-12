use super::*;

#[tokio::test]
async fn application_runtime_routes_logs_are_paginated_and_newest_first() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let mut flow_run_ids = Vec::new();

    for index in 0..25 {
        let create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/console/applications/{application_id}/orchestration/debug-runs"
                    ))
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "input_payload": {
                                "node-start": {
                                    "query": format!("run-{index:02}")
                                }
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create.status(), StatusCode::CREATED);
        let create_body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
        let create_payload: Value = serde_json::from_slice(&create_body).unwrap();
        flow_run_ids.push(
            create_payload["data"]["flow_run"]["id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }

    for flow_run_id in &flow_run_ids {
        wait_for_run_detail(
            &app,
            &cookie,
            &application_id,
            flow_run_id,
            &["succeeded", "failed", "cancelled"],
        )
        .await;
    }

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs?page=1&page_size=20"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list.status(), StatusCode::OK);
    let list_body = to_bytes(list.into_body(), usize::MAX).await.unwrap();
    let list_payload: Value = serde_json::from_slice(&list_body).unwrap();
    let items = list_payload["data"]["items"].as_array().unwrap();

    assert_eq!(list_payload["data"]["page"].as_i64(), Some(1));
    assert_eq!(list_payload["data"]["page_size"].as_i64(), Some(20));
    assert_eq!(list_payload["data"]["total"].as_i64(), Some(25));
    assert_eq!(items.len(), 20);
    assert_eq!(items[0]["id"].as_str(), Some(flow_run_ids[24].as_str()));
    assert_eq!(items[19]["id"].as_str(), Some(flow_run_ids[5].as_str()));
}

#[tokio::test]
async fn application_runtime_routes_logs_default_to_recent_time_window() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let old_run_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "old run outside default window",
    )
    .await;
    let recent_run_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "recent run inside default window",
    )
    .await;

    for flow_run_id in [&old_run_id, &recent_run_id] {
        wait_for_run_detail(
            &app,
            &cookie,
            &application_id,
            flow_run_id,
            &["succeeded", "failed", "cancelled"],
        )
        .await;
    }

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"
        update application_run_log_summaries
        set created_at = now() - interval '8 days',
            started_at = now() - interval '8 days',
            updated_at = now() - interval '8 days'
        where flow_run_id = $1
        "#,
    )
    .bind(Uuid::parse_str(&old_run_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    // The raw edit bypassed the projection writer; refresh the task row the
    // writer would have recomputed so the list reflects the aged summary.
    sqlx::query("select application_run_log_task_refresh($1)")
        .bind(Uuid::parse_str(&old_run_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let default_payload = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{application_id}/logs/runs"),
    )
    .await;
    let default_ids = default_payload["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(default_payload["data"]["total"].as_i64(), Some(1));
    assert!(default_ids.contains(&recent_run_id.as_str()));
    assert!(!default_ids.contains(&old_run_id.as_str()));

    let extended_payload = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{application_id}/logs/runs?time_range_days=30"),
    )
    .await;
    let extended_ids = extended_payload["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(extended_payload["data"]["total"].as_i64(), Some(2));
    assert!(extended_ids.contains(&recent_run_id.as_str()));
    assert!(extended_ids.contains(&old_run_id.as_str()));
}
