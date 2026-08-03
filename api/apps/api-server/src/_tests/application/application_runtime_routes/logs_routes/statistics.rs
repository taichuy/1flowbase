use super::*;

#[tokio::test]
async fn application_runtime_routes_logs_report_run_statistics() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let start = app
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
                            "node-start": { "query": "请总结退款政策" }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::CREATED);
    let start_body = to_bytes(start.into_body(), usize::MAX).await.unwrap();
    let start_payload: Value = serde_json::from_slice(&start_body).unwrap();
    let flow_run_id_string = start_payload["data"]["flow_run"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let flow_run_id = Uuid::parse_str(&flow_run_id_string).unwrap();
    wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        &flow_run_id_string,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();

    sqlx::query("delete from application_run_log_summaries where flow_run_id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("delete from flow_run_callback_tasks where flow_run_id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("delete from flow_run_checkpoints where flow_run_id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("delete from flow_run_events where flow_run_id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("delete from node_runs where flow_run_id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();

    let llm_run_id = Uuid::now_v7();
    for (node_run_id, node_id, node_type, node_alias, metrics_payload) in [
        (
            llm_run_id,
            "node-llm",
            "llm",
            "LLM",
            json!({ "usage": { "total_tokens": 12, "input_cache_hit_tokens": 49 } }),
        ),
        (
            Uuid::now_v7(),
            "node-llm",
            "llm",
            "LLM",
            json!({ "usage": { "total_tokens": 8 } }),
        ),
        (
            Uuid::now_v7(),
            "node-summary",
            "llm",
            "Summary",
            json!({ "usage": { "input_tokens": 10, "output_tokens": 20 } }),
        ),
        (Uuid::now_v7(), "node-answer", "answer", "Answer", json!({})),
    ] {
        sqlx::query(
            r#"
            insert into node_runs (
                id,
                scope_id,
                flow_run_id,
                node_id,
                node_type,
                node_alias,
                status,
                input_payload,
                output_payload,
                error_payload,
                metrics_payload,
                started_at,
                finished_at
            ) values (
                $1,
                (
                    select applications.workspace_id
                    from flow_runs
                    join applications on applications.id = flow_runs.application_id
                    where flow_runs.id = $2
                ),
                $2,
                $3,
                $4,
                $5,
                'succeeded',
                '{}'::jsonb,
                '{}'::jsonb,
                null,
                $6,
                now(),
                now()
            )
            "#,
        )
        .bind(node_run_id)
        .bind(flow_run_id)
        .bind(node_id)
        .bind(node_type)
        .bind(node_alias)
        .bind(metrics_payload)
        .execute(&pool)
        .await
        .unwrap();
    }

    let tool_calls = (0..20)
        .map(|index| {
            json!({
                "id": format!("call-{index}"),
                "name": "lookup_policy"
            })
        })
        .collect::<Vec<_>>();
    sqlx::query(
        r#"
        insert into flow_run_callback_tasks (
            id,
            scope_id,
            flow_run_id,
            node_run_id,
            callback_kind,
            status,
            request_payload,
            response_payload,
            external_ref_payload,
            completed_at
        ) values (
            $1,
            (
                select applications.workspace_id
                from flow_runs
                join applications on applications.id = flow_runs.application_id
                where flow_runs.id = $2
            ),
            $2,
            $3,
            'llm_tool_calls',
            'completed',
            $4,
            '{}'::jsonb,
            '{}'::jsonb,
            now()
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_run_id)
    .bind(llm_run_id)
    .bind(json!({ "tool_calls": tool_calls }))
    .execute(&pool)
    .await
    .unwrap();

    let flow_run = <MainDurableStore as OrchestrationRuntimeRepository>::get_flow_run(
        &state.store,
        Uuid::parse_str(&application_id).unwrap(),
        flow_run_id,
    )
    .await
    .unwrap()
    .unwrap();
    <MainDurableStore as OrchestrationRuntimeRepository>::update_flow_run(
        &state.store,
        &UpdateFlowRunInput {
            flow_run_id,
            status: flow_run.status,
            output_payload: flow_run.output_payload,
            error_payload: flow_run.error_payload,
            finished_at: flow_run.finished_at,
        },
    )
    .await
    .unwrap();

    let expected_statistics = json!({
        "total_tokens": 50,
        "input_tokens": 10,
        "output_tokens": 20,
        "count_tokens_input_tokens": null,
        "input_cache_hit_tokens": 49,
        "input_cache_hit_rate": 0.98,
        "unique_node_count": 3,
        "tool_callback_count": 20
    });
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
    assert_eq!(
        list_payload["data"]["items"][0]["statistics"],
        expected_statistics
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
        trace_tree_payload["data"]["statistics"],
        expected_statistics
    );
}

#[tokio::test]
async fn application_runtime_routes_monitoring_non_positive_window_uses_default() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let old_run_id =
        start_full_debug_run(&app, &cookie, &csrf, &application_id, "old monitoring run").await;
    let recent_run_id = start_full_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "recent monitoring run",
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
        set started_at = now() - interval '8 days',
            finished_at = now() - interval '8 days',
            created_at = now() - interval '8 days',
            updated_at = now() - interval '8 days'
        where flow_run_id = $1
        "#,
    )
    .bind(Uuid::parse_str(&old_run_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let default_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/monitoring/run-metrics?time_range_days=0"
        ),
    )
    .await;

    assert_eq!(
        default_payload["data"]["meta"]["bucket"].as_str(),
        Some("day")
    );
    assert_eq!(
        default_payload["data"]["overview"]["total_count"].as_i64(),
        Some(1)
    );

    let extended_payload = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{application_id}/monitoring/run-metrics?time_range_days=30"
        ),
    )
    .await;

    assert_eq!(
        extended_payload["data"]["overview"]["total_count"].as_i64(),
        Some(2)
    );
}
