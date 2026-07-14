use super::*;

#[derive(Debug, PartialEq)]
struct RuntimeReadPayloadSnapshot {
    artifact_count: i64,
    payloads: Value,
}

async fn runtime_read_payload_snapshot(
    pool: &sqlx::PgPool,
    flow_run_id: Uuid,
) -> RuntimeReadPayloadSnapshot {
    let artifact_count = sqlx::query_scalar::<_, i64>(
        "select count(*) from runtime_debug_artifacts where flow_run_id = $1",
    )
    .bind(flow_run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let payloads = sqlx::query_scalar::<_, Value>(
        r#"
        select jsonb_build_object(
            'flow_run',
            (
                select jsonb_build_object(
                    'input_payload', input_payload,
                    'output_payload', output_payload,
                    'error_payload', error_payload
                )
                from flow_runs
                where id = $1
            ),
            'node_runs',
            coalesce(
                (
                    select jsonb_agg(
                        jsonb_build_object(
                            'id', id::text,
                            'input_payload', input_payload,
                            'output_payload', output_payload,
                            'error_payload', error_payload,
                            'metrics_payload', metrics_payload,
                            'debug_payload', debug_payload
                        )
                        order by id
                    )
                    from node_runs
                    where flow_run_id = $1
                ),
                '[]'::jsonb
            ),
            'checkpoints',
            coalesce(
                (
                    select jsonb_agg(
                        jsonb_build_object(
                            'id', id::text,
                            'locator_payload', locator_payload,
                            'variable_snapshot', variable_snapshot,
                            'external_ref_payload', external_ref_payload
                        )
                        order by id
                    )
                    from flow_run_checkpoints
                    where flow_run_id = $1
                ),
                '[]'::jsonb
            ),
            'callback_tasks',
            coalesce(
                (
                    select jsonb_agg(
                        jsonb_build_object(
                            'id', id::text,
                            'request_payload', request_payload,
                            'response_payload', response_payload,
                            'external_ref_payload', external_ref_payload
                        )
                        order by id
                    )
                    from flow_run_callback_tasks
                    where flow_run_id = $1
                ),
                '[]'::jsonb
            ),
            'events',
            coalesce(
                (
                    select jsonb_agg(
                        jsonb_build_object(
                            'id', id::text,
                            'payload', payload
                        )
                        order by sequence, id
                    )
                    from flow_run_events
                    where flow_run_id = $1
                ),
                '[]'::jsonb
            )
        )
        "#,
    )
    .bind(flow_run_id)
    .fetch_one(pool)
    .await
    .unwrap();

    RuntimeReadPayloadSnapshot {
        artifact_count,
        payloads,
    }
}

async fn seed_large_runtime_read_payloads(
    pool: &sqlx::PgPool,
    flow_run_id: Uuid,
    node_run_id: Uuid,
) {
    let large_text = "runtime read payload ".repeat(360);
    let flow_input_payload = json!({ "query": large_text, "model": "fixture_chat" });
    let large_object = json!({ "large": large_text });
    sqlx::query(
        r#"
        update flow_runs
        set input_payload = $2,
            output_payload = $3,
            error_payload = $4
        where id = $1
        "#,
    )
    .bind(flow_run_id)
    .bind(&flow_input_payload)
    .bind(&large_object)
    .bind(Some(large_object.clone()))
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        update node_runs
        set input_payload = $2,
            output_payload = $3,
            error_payload = $4,
            metrics_payload = $5,
            debug_payload = $6
        where id = $1
        "#,
    )
    .bind(node_run_id)
    .bind(&large_object)
    .bind(&large_object)
    .bind(Some(large_object.clone()))
    .bind(&large_object)
    .bind(&large_object)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into flow_run_checkpoints (
            id,
            scope_id,
            flow_run_id,
            node_run_id,
            status,
            reason,
            locator_payload,
            variable_snapshot,
            external_ref_payload
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
            'waiting_human',
            'read payload immutability check',
            $4,
            $4,
            $4
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_run_id)
    .bind(node_run_id)
    .bind(&large_object)
    .execute(pool)
    .await
    .unwrap();

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
            $4,
            $4,
            now()
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_run_id)
    .bind(node_run_id)
    .bind(&large_object)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into flow_run_events (
            id,
            scope_id,
            flow_run_id,
            node_run_id,
            sequence,
            event_type,
            payload
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
            (
                select coalesce(max(sequence), 0) + 1
                from flow_run_events
                where flow_run_id = $2
            ),
            'read_payload_immutability_check',
            $4
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_run_id)
    .bind(node_run_id)
    .bind(&large_object)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn application_runtime_routes_trace_tree_and_node_last_run_do_not_materialize_artifacts() {
    let (state, database_url) = test_api_state_with_database_url().await;
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
    let flow_run_id_string = preview_payload["data"]["flow_run"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let flow_run_id = Uuid::parse_str(&flow_run_id_string).unwrap();
    let node_run_id =
        Uuid::parse_str(preview_payload["data"]["node_run"]["id"].as_str().unwrap()).unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    seed_large_runtime_read_payloads(&pool, flow_run_id, node_run_id).await;
    let before = runtime_read_payload_snapshot(&pool, flow_run_id).await;

    let trace_tree_uri = format!(
        "/api/console/applications/{application_id}/logs/runs/{flow_run_id_string}/trace-tree"
    );
    let trace_tree = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(trace_tree_uri.as_str())
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let trace_tree_status = trace_tree.status();
    let trace_tree_body = to_bytes(trace_tree.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        trace_tree_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&trace_tree_body)
    );
    let trace_tree_payload: Value = serde_json::from_slice(&trace_tree_body).unwrap();
    let trace_node_id = trace_tree_payload["data"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["node_id"] == json!("node-llm"))
        .expect("LLM trace node exists")["trace_node_id"]
        .as_str()
        .unwrap()
        .to_string();
    let trace_node_content_uri = format!(
        "/api/console/applications/{application_id}/logs/runs/{flow_run_id_string}/trace-tree/nodes/{trace_node_id}/content"
    );
    let trace_node_content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(trace_node_content_uri.as_str())
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let trace_node_content_status = trace_node_content.status();
    let trace_node_content_body = to_bytes(trace_node_content.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        trace_node_content_status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&trace_node_content_body)
    );
    let trace_node_content_payload: Value =
        serde_json::from_slice(&trace_node_content_body).unwrap();
    let trace_node_content_data = trace_node_content_payload["data"]
        .as_object()
        .expect("trace node content data is an object");
    assert!(
        !trace_node_content_data.contains_key("node_run"),
        "trace node content must not return the full node_run container by default"
    );
    assert!(
        !trace_node_content_data.contains_key("checkpoints"),
        "trace node content must not return checkpoints by default"
    );
    assert!(
        !trace_node_content_data.contains_key("events"),
        "trace node content must not return events by default"
    );
    assert_eq!(
        trace_node_content_payload["data"]["content_kind"],
        json!("node_run")
    );
    assert_eq!(
        trace_node_content_payload["data"]["payload"]["payload_index"]["node_run_count"],
        json!(1)
    );
    assert!(trace_node_content_payload["data"]["source_refs"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));
    assert!(trace_node_content_payload["data"]["detail_refs"]
        .as_array()
        .is_some_and(|items| !items.is_empty()));

    for uri in [
        trace_tree_uri,
        trace_node_content_uri,
        format!(
            "/api/console/applications/{application_id}/logs/runs/{flow_run_id_string}/nodes/node-llm"
        ),
        format!("/api/console/applications/{application_id}/orchestration/nodes/node-llm/last-run"),
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
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    }

    let after = runtime_read_payload_snapshot(&pool, flow_run_id).await;
    assert_eq!(after, before);
}
