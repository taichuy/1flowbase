use super::run_trajectory::get;
use super::*;

#[tokio::test]
async fn workflow_trajectory_routes_preserve_authorization_filters_and_selected_body_scope() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application = seed_agent_flow_application(&app, &cookie, &csrf, &provider).await;
    let preview =
        start_llm_preview(&app, &cookie, &csrf, &application, "workflow event fixture").await;
    let run = preview["data"]["flow_run"]["id"].as_str().unwrap();
    let run_id = Uuid::parse_str(run).unwrap();
    let node = Uuid::parse_str(preview["data"]["node_run"]["id"].as_str().unwrap()).unwrap();
    let base =
        format!("/api/console/applications/{application}/logs/runs/{run}/workflow-trajectory");
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query("update node_runs set node_alias='执行时节点名',raw_json_payloads=jsonb_build_object('input_payload','invalid JSON') where id=$1").bind(node).execute(&pool).await.unwrap();
    seed_flow_run_history_events(&database_url, &[AppendRuntimeEventInput {
        flow_run_id: run_id, node_run_id: Some(node), span_id: None, parent_span_id: None,
        event_type: "provider_semantic_step".into(), layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host, trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None, ledger_ref: None,
        payload: json!({"source":"ai_native","invocation_id":"workflow-route-fixture","provider_attempt_index":0,"step_key":"reply","kind":"model_reply","status":"recorded","preview":"Hello","body":"{\"text\":\"Hello\"}"}),
        visibility: domain::RuntimeEventVisibility::Internal, durability: domain::RuntimeEventDurability::Durable,
    }]).await.unwrap();
    assert_eq!(get(&app, None, &base).await.0, StatusCode::UNAUTHORIZED);
    for query in [
        "category=unknown",
        "cursor=broken",
        "from=not-a-date",
        "from=2026-09-23T00:00:00Z&to=2026-09-22T00:00:00Z",
    ] {
        assert_eq!(
            get(&app, Some(&cookie), &format!("{base}?{query}")).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let (status, page) = get(
        &app,
        Some(&cookie),
        &format!("{base}?category=requests&node_run_id={node}&limit=1"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let first = &page["data"]["items"][0];
    assert_eq!(first["node_alias"], "执行时节点名");
    assert_eq!(first["node_run_id"], node.to_string());
    assert!(first["native_step"]["metadata"].get("body").is_none());
    assert_eq!(
        get(
            &app,
            Some(&cookie),
            &format!("{base}?node_run_id={}", Uuid::now_v7())
        )
        .await
        .1["data"]["items"],
        json!([])
    );
    let (status, nodes) = get(
        &app,
        Some(&cookie),
        &format!("{base}?category=nodes&node_run_id={node}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{nodes}");
    assert!(!nodes["data"]["items"].as_array().unwrap().is_empty());
    // Body is read only on selection, and only from the authorized task closure.
    sqlx::query("update node_runs set raw_json_payloads='{}',input_payload=jsonb_build_object('input','selected only') where id=$1").bind(node).execute(&pool).await.unwrap();
    let event = nodes["data"]["items"][0]["event_id"].as_str().unwrap();
    assert_eq!(
        get(&app, Some(&cookie), &format!("{base}/{event}")).await.0,
        StatusCode::OK
    );
    assert_eq!(
        get(&app, None, &format!("{base}/{event}")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get(
            &app,
            Some(&cookie),
            &format!("{base}/node_started:{}", Uuid::now_v7())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let wrong_application = format!(
        "/api/console/applications/{}/logs/runs/{run}/workflow-trajectory",
        Uuid::now_v7()
    );
    assert_eq!(
        get(&app, Some(&cookie), &wrong_application).await.0,
        StatusCode::NOT_FOUND
    );
}
