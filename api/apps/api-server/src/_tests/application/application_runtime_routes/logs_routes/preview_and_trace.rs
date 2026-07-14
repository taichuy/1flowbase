use super::*;

#[tokio::test]
async fn application_runtime_routes_start_node_preview_and_query_logs() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/nodes/node-llm/debug-runs"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": {
                            "node-start": { "query": "总结退款政策" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let preview_status = preview.status();
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        preview_status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&preview_body)
    );
    let preview_payload: Value = serde_json::from_slice(&preview_body).unwrap();
    let flow_run_id = preview_payload["data"]["flow_run"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    assert_eq!(
        preview_payload["data"]["flow_run"]["run_mode"].as_str(),
        Some("debug_node_preview")
    );
    assert_eq!(
        preview_payload["data"]["node_run"]["node_id"].as_str(),
        Some("node-llm")
    );
    assert_eq!(
        preview_payload["data"]["node_run"]["output_payload"]["text"],
        json!("reply:总结退款政策")
    );
    for hidden_key in [
        "resolved_inputs",
        "rendered_templates",
        "output_contract",
        "metrics_payload",
        "debug_payload",
        "provider_events",
    ] {
        assert!(
            preview_payload["data"]["node_run"]["output_payload"]
                .get(hidden_key)
                .is_none(),
            "{hidden_key} must not leak into node output"
        );
        assert!(
            preview_payload["data"]["flow_run"]["output_payload"]
                .get(hidden_key)
                .is_none(),
            "{hidden_key} must not leak into flow output"
        );
    }
    assert_eq!(
        preview_payload["data"]["events"][0]["event_type"].as_str(),
        Some("node_preview_started")
    );
    let event_types = preview_payload["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event["event_type"].as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"text_delta"));
    assert!(event_types.contains(&"usage_snapshot"));
    assert!(event_types.contains(&"finish"));
    assert!(event_types.contains(&"node_preview_completed"));

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs"
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
    assert_eq!(list_payload["data"]["page"].as_i64(), Some(1));
    assert_eq!(list_payload["data"]["page_size"].as_i64(), Some(20));
    assert_eq!(list_payload["data"]["total"].as_i64(), Some(1));
    assert_eq!(list_payload["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        list_payload["data"]["items"][0]["id"].as_str(),
        Some(flow_run_id.as_str())
    );
    assert_eq!(
        list_payload["data"]["items"][0]["application_id"].as_str(),
        Some(application_id.as_str())
    );
    assert_eq!(
        list_payload["data"]["items"][0]["application_type"].as_str(),
        Some("agent_flow")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["run_object_kind"].as_str(),
        Some("application_run")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["subject"]["kind"].as_str(),
        Some("agent_flow")
    );
    assert_eq!(
        list_payload["data"]["items"][0]["title"].as_str(),
        Some("总结退款政策")
    );
    let list_item = list_payload["data"]["items"][0].as_object().unwrap();
    assert!(
        !list_item.contains_key("input_payload"),
        "run list summary must not return full raw input payload"
    );
    assert!(
        !list_item.contains_key("output_payload"),
        "run list summary must not return full raw output payload"
    );
    assert!(
        !list_item.contains_key("error_payload"),
        "run list summary must not return full raw error payload"
    );
    assert!(list_payload["data"]["items"][0]["created_at"].is_string());
    assert!(list_payload["data"]["items"][0]["updated_at"].is_string());

    let runtime_activity = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/monitoring/runtime-activity"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(runtime_activity.status(), StatusCode::OK);
    let runtime_activity_body = to_bytes(runtime_activity.into_body(), usize::MAX)
        .await
        .unwrap();
    let runtime_activity_payload: Value = serde_json::from_slice(&runtime_activity_body).unwrap();
    assert_eq!(
        runtime_activity_payload["data"]["meta"]["storage"].as_str(),
        Some("memory")
    );
    assert_eq!(
        runtime_activity_payload["data"]["meta"]["scope"].as_str(),
        Some("current_instance")
    );
    assert!(
        runtime_activity_payload["data"]["peaks"]["process_peak_concurrency"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert!(
        runtime_activity_payload["data"]["rolling_minute"]["completed"]
            .as_u64()
            .unwrap()
            >= 1
    );

    let trace_tree = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(trace_tree.status(), StatusCode::OK);
    let trace_tree_body = to_bytes(trace_tree.into_body(), usize::MAX).await.unwrap();
    let trace_tree_payload: Value = serde_json::from_slice(&trace_tree_body).unwrap();
    assert_eq!(
        trace_tree_payload["data"]["flow_run"]["id"].as_str(),
        Some(flow_run_id.as_str())
    );
    assert_eq!(
        trace_tree_payload["data"]["run"]["id"].as_str(),
        Some(flow_run_id.as_str())
    );
    assert_eq!(
        trace_tree_payload["data"]["run"]["application_type"].as_str(),
        Some("agent_flow")
    );
    assert_eq!(
        trace_tree_payload["data"]["run"]["run_object_kind"].as_str(),
        Some("application_run")
    );
    assert_eq!(
        trace_tree_payload["data"]["flow_run"]["title"].as_str(),
        Some("总结退款政策")
    );
    assert_eq!(
        trace_tree_payload["data"]["nodes"][0]["node_alias"].as_str(),
        Some("LLM")
    );
    let cache_entries = state
        .infrastructure
        .cache_store()
        .list_cache_entries("application-logs")
        .await
        .unwrap();
    assert!(
        cache_entries
            .iter()
            .any(|entry| entry.key.contains(":summary-page:")),
        "application log summary-page cache entry missing: {cache_entries:?}"
    );
    let summary_cache_key = cache_entries
        .iter()
        .find(|entry| entry.key.contains(":summary-page:"))
        .expect("summary-page cache entry must exist")
        .key
        .clone();
    let mut stale_page = list_payload["data"].clone();
    stale_page["items"][0]["title"] = json!("stale cache title");
    state
        .infrastructure
        .cache_store()
        .set_json(
            &summary_cache_key,
            stale_page,
            Some(time::Duration::minutes(5)),
        )
        .await
        .unwrap();
    let cached_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cached_list.status(), StatusCode::OK);
    let cached_list_body = to_bytes(cached_list.into_body(), usize::MAX).await.unwrap();
    let cached_list_payload: Value = serde_json::from_slice(&cached_list_body).unwrap();
    assert_eq!(
        cached_list_payload["data"]["items"][0]["title"].as_str(),
        Some("stale cache title")
    );
    let refreshed_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs?cache_mode=refresh"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refreshed_list.status(), StatusCode::OK);
    let refreshed_list_body = to_bytes(refreshed_list.into_body(), usize::MAX)
        .await
        .unwrap();
    let refreshed_list_payload: Value = serde_json::from_slice(&refreshed_list_body).unwrap();
    assert_eq!(
        refreshed_list_payload["data"]["items"][0]["title"].as_str(),
        Some("总结退款政策")
    );
    assert!(
        !cache_entries.iter().any(|entry| {
            entry.key.contains(":run-detail:") && entry.key.contains(flow_run_id.as_str())
        }),
        "application log run-detail must not be cached: {cache_entries:?}"
    );
    let scoped_node_run = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/nodes/node-llm"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(scoped_node_run.status(), StatusCode::OK);
    let scoped_node_run_body = to_bytes(scoped_node_run.into_body(), usize::MAX)
        .await
        .unwrap();
    let scoped_node_run_payload: Value = serde_json::from_slice(&scoped_node_run_body).unwrap();
    assert_eq!(
        scoped_node_run_payload["data"]["node_run"]["node_id"].as_str(),
        Some("node-llm")
    );
    assert!(scoped_node_run_payload["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|event| event["node_run_id"].as_str()
            == Some(
                scoped_node_run_payload["data"]["node_run"]["id"]
                    .as_str()
                    .unwrap()
            )));

    let last_run = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/nodes/node-llm/last-run"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(last_run.status(), StatusCode::OK);
    let last_run_body = to_bytes(last_run.into_body(), usize::MAX).await.unwrap();
    let last_run_payload: Value = serde_json::from_slice(&last_run_body).unwrap();
    assert_eq!(
        last_run_payload["data"]["node_run"]["node_id"].as_str(),
        Some("node-llm")
    );
    assert_eq!(
        last_run_payload["data"]["flow_run"]["id"].as_str(),
        Some(flow_run_id.as_str())
    );
}
