use super::*;

#[tokio::test]
async fn workflow_schedule_tick_creates_and_executes_async_run() {
    let (app, state) = test_app_with_state().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_workflow_application_with_trigger(
        &app,
        &cookie,
        &csrf,
        "Tick Scheduled Workflow",
        "schedule",
    )
    .await;
    publish_workflow_extension_with_enabled(
        &app,
        &cookie,
        &csrf,
        &application_id,
        WorkflowExtensionPublishOptions::new("tick-scheduled-workflow", "async", false),
    )
    .await;

    let replace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": true,
                        "cron": "* * * * *",
                        "timezone": "UTC",
                        "input_payload": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace_response.status(), StatusCode::OK);

    let service = control_plane::application_public_api::workflow_schedule::WorkflowScheduleTriggerService::new(
        state.store.clone(),
    );
    let task_queue = state.infrastructure.registered_task_queue();
    let entries = service
        .dispatch_due_schedules(time::OffsetDateTime::now_utc(), task_queue.as_deref())
        .await
        .unwrap();

    let dispatched = entries
        .iter()
        .filter_map(|entry| match &entry.outcome {
            control_plane::application_public_api::workflow_schedule::WorkflowScheduleDispatchOutcome::Dispatched(result) => Some(result.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(dispatched.len(), 1);
    let (run_mode, api_key_id, compatibility_mode): (String, Option<Uuid>, Option<String>) =
        sqlx::query_as(
            "select run_mode, api_key_id, compatibility_mode from flow_runs where id = $1",
        )
        .bind(dispatched[0].run_id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(run_mode, "workflow_schedule_run");
    assert_eq!(api_key_id, None);
    assert_eq!(compatibility_mode, None);

    let outcome = crate::workers::workflow_schedule::consume_one_workflow_schedule_run(
        state.clone(),
        "workflow-schedule-tick-test",
        time::Duration::seconds(30),
    )
    .await
    .unwrap();
    let flow_run_id = match outcome {
        crate::workers::workflow_schedule::WorkflowScheduleWorkerOutcome::Executed {
            flow_run_id,
            ..
        } => flow_run_id,
        other => panic!("expected schedule run execution, got {other:?}"),
    };
    assert_eq!(flow_run_id, dispatched[0].run_id);

    wait_for_flow_run_status(&state, flow_run_id, "succeeded").await;
}

#[tokio::test]
async fn workflow_extension_route_rejects_schedule_trigger_application() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_workflow_application_with_trigger(
        &app,
        &cookie,
        &csrf,
        "schedule-typed-extension",
        "schedule",
    )
    .await;
    save_workflow_document(&app, &cookie, &csrf, &application_id).await;
    let token = create_user_api_key(&app, &cookie, &csrf).await;
    publish_workflow_extension(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "schedule-typed-extension",
        "async",
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/schedule-typed-extension")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(response).await["code"],
        json!("workflow_extension_not_found")
    );
}

#[tokio::test]
async fn workflow_extension_openapi_and_settings_docs_exclude_schedule_trigger_applications() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_workflow_application_with_trigger(
        &app,
        &cookie,
        &csrf,
        "schedule-typed-docs",
        "schedule",
    )
    .await;
    save_workflow_document(&app, &cookie, &csrf, &application_id).await;
    publish_workflow_extension(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "schedule-typed-docs",
        "async",
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert!(payload["paths"]
        .as_object()
        .unwrap()
        .get("/api/ex/schedule-typed-docs")
        .is_none());

    let catalog_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/catalog")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog = response_json(catalog_response).await;
    assert!(catalog["data"]["categories"]
        .as_array()
        .unwrap()
        .iter()
        .all(|category| category["id"] != json!("workflow_extensions")));
}
