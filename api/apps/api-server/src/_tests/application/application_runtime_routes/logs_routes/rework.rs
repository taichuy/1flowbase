use super::*;

// AC-006/011/013: exercise original Router, authentication, detail and paging.
#[tokio::test]
async fn issue_2032_rework_original_log_routes_keep_auth_detail_and_retire_gateway_route() {
    let assembly = crate::routes::application_runtime::route_assembly();
    assert!(assembly
        .bindings()
        .iter()
        .any(|binding| binding.route.path.ends_with("/logs/runs")));
    assert!(!assembly
        .bindings()
        .iter()
        .any(|binding| binding.route.path.ends_with("/logs/gateway")));
    let (app, database_url) = test_app_with_database_url().await;
    let missing = Uuid::now_v7();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/applications/{missing}/logs/runs"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    fund_log_fixture(&app, &cookie, &csrf).await;
    let provider = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id = seed_agent_flow_application(&app, &cookie, &csrf, &provider).await;
    let page = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{application_id}/logs/runs?page_size=10000"),
    )
    .await;
    assert_eq!(page["data"]["page_size"], 100);
    let run = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "original log contract",
    )
    .await;
    wait_for_run_detail(&app, &cookie, &application_id, &run, &["succeeded"]).await;
    let messages = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{application_id}/logs/runs/{run}/conversation/messages"),
    )
    .await;
    assert!(!messages["data"]["items"].as_array().unwrap().is_empty());
    for item in messages["data"]["items"].as_array().unwrap() {
        assert!(item["message_id"].is_string());
        assert!(item["run_id"].is_string());
    }
    let overview = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{application_id}/logs/runs/{run}/overview"),
    )
    .await;
    assert_eq!(overview["data"]["statistics"]["invocation_count"], 1);
    let filter = serde_json::json!({"application_id":{"$eq":application_id}}).to_string();
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("filter", &filter)
        .finish();
    let original_records = get_console_json(
        &app,
        &cookie,
        format!("/api/runtime/models/application_run_log_summaries/list?{query}"),
    )
    .await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let scope_rows:Vec<Value>=sqlx::query_scalar("select jsonb_build_object('scope_id',scope_id,'created_by',created_by,'flow_run_id',flow_run_id) from application_run_log_summaries where application_id=$1")
        .bind(Uuid::parse_str(&application_id).unwrap()).fetch_all(&pool).await.unwrap();
    let session = get_console_json(&app, &cookie, "/api/console/session".to_owned()).await;
    assert_eq!(
        original_records["data"]["total"], 1,
        "scope rows={scope_rows:?}, actor_scope={}",
        session["data"]["actor"]["current_workspace_id"]
    );
    assert_eq!(original_records["data"]["items"][0]["id"], run);
    assert_eq!(original_records["data"]["items"][0]["invocation_count"], 1);

    for (uri, expected_status) in [
        (
            format!("/api/console/applications/{application_id}/logs/gateway"),
            StatusCode::FORBIDDEN,
        ),
        (
            format!("/api/console/applications/{missing}/logs/runs/{run}/conversation/messages"),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
    }
}
