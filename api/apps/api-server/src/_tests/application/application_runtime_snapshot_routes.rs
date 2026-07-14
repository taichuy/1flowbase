use super::application_runtime_routes::{
    create_ready_provider_instance, seed_agent_flow_application,
};
use crate::_tests::support::{
    create_member, login_and_capture_cookie, replace_member_roles, test_app,
    test_app_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

async fn start_preview(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    query: &str,
    debug_session_id: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/nodes/node-llm/debug-runs"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": {
                            "node-start": { "query": query },
                            "node-llm": { "prompt_messages": ["resolved prompt must stay audit-only"] }
                        },
                        "debug_session_id": debug_session_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

async fn start_debug_run(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    query: &str,
    debug_session_id: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-runs"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input_payload": {
                            "node-start": { "query": query }
                        },
                        "debug_session_id": debug_session_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

async fn wait_for_run_status(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    expected_status: &str,
) -> Value {
    let mut last_status = String::new();
    let mut last_error = Value::Null;
    for _ in 0..200 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/console/applications/{application_id}/logs/runs/{run_id}/trace-tree"
                    ))
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let payload: Value = serde_json::from_slice(&body).unwrap();
        last_status = payload["data"]["flow_run"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        last_error = payload["data"]["flow_run"]["error_payload"].clone();
        if last_status == expected_status {
            return payload["data"].clone();
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!(
        "timed out waiting for run status: {expected_status}, last status: {last_status}, last error: {last_error}"
    );
}

async fn wait_for_cache_value(
    pool: &sqlx::PgPool,
    application_id: Uuid,
    node_id: &str,
    variable_key: &str,
) -> serde_json::Value {
    for _ in 0..200 {
        let value = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            select value
            from debug_variable_cache_entries
            where application_id = $1
              and node_id = $2
              and variable_key = $3
            "#,
        )
        .bind(application_id)
        .bind(node_id)
        .bind(variable_key)
        .fetch_optional(pool)
        .await
        .unwrap();

        if let Some(value) = value {
            return value;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!("timed out waiting for debug variable cache value {node_id}.{variable_key}");
}

async fn get_snapshot(app: &axum::Router, cookie: &str, application_id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap()
}

async fn get_snapshot_by_legacy_query(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    query: &str,
) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot?{query}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn debug_variable_snapshot_keeps_actor_run_scope_isolated() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &root_cookie, &root_csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &root_cookie, &root_csrf, &provider_instance_id).await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "snapshot-admin",
        "change-me",
    )
    .await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["admin"]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "snapshot-admin", "change-me").await;

    let root_preview = start_preview(
        &app,
        &root_cookie,
        &root_csrf,
        &application_id,
        "root policy",
        "root-session",
    )
    .await;
    let member_preview = start_preview(
        &app,
        &member_cookie,
        &member_csrf,
        &application_id,
        "member policy",
        "member-session",
    )
    .await;
    let root_run_id = root_preview["data"]["flow_run"]["id"].as_str().unwrap();
    let member_run_id = member_preview["data"]["flow_run"]["id"].as_str().unwrap();

    let snapshot = get_snapshot(&app, &root_cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["latest_run_scope"]["flow_run_id"],
        root_run_id
    );
    assert_ne!(
        snapshot["data"]["latest_run_scope"]["flow_run_id"],
        member_run_id
    );
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-start"]["query"],
        "root policy"
    );
    assert!(snapshot["data"]["source_flow_run_ids"]["node-start"].is_null());
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:root policy"
    );
}

#[tokio::test]
async fn debug_variable_snapshot_ignores_legacy_query_parameters() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "session policy",
        "session-a",
    )
    .await;

    let mismatched =
        get_snapshot_by_legacy_query(&app, &cookie, &application_id, "debug_session_id=session-b")
            .await;
    assert_eq!(mismatched["data"]["debug_session_id"], "");
    assert_eq!(mismatched["data"]["snapshot_completeness"], "complete");
    assert!(mismatched["data"]["latest_run_scope"].is_object());
    assert_eq!(
        mismatched["data"]["variable_cache"]["node-llm"]["text"],
        "reply:session policy"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["debug_session_id"], "");
    assert_eq!(
        payload["data"]["variable_cache"]["node-llm"]["text"],
        "reply:session policy"
    );
}

#[tokio::test]
async fn debug_variable_cache_entry_persists_without_frontend_session() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-cache"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "node_id": "node-llm",
                        "variable_key": "text",
                        "value": "manual durable cache"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(snapshot.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload["data"]["variable_cache"]["node-llm"]["text"],
        "manual durable cache"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-cache"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payload: Value =
        serde_json::from_slice(&to_bytes(snapshot.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["variable_cache"], json!({}));
}

#[tokio::test]
async fn debug_variable_snapshot_returns_persisted_large_cache_value_inline() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let large_text = "persisted cache value ".repeat(128);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-cache"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "node_id": "node-llm",
                        "variable_key": "text",
                        "value": large_text
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        json!(large_text)
    );
    assert_ne!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"]["__runtime_debug_artifact"],
        true
    );

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let application_uuid = Uuid::parse_str(&application_id).unwrap();
    let artifact_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from runtime_debug_artifacts
        where application_id = $1
        "#,
    )
    .bind(application_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(artifact_count, 0);
}

#[tokio::test]
async fn debug_node_preview_persists_variable_cache_as_snapshot_source() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let preview = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "durable preview cache",
        "cache-session",
    )
    .await;
    let application_uuid = Uuid::parse_str(&application_id).unwrap();
    let flow_run_id = Uuid::parse_str(preview["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let persisted_value: serde_json::Value = sqlx::query_scalar(
        r#"
        select value
        from debug_variable_cache_entries
        where application_id = $1
          and node_id = 'node-llm'
          and variable_key = 'text'
        "#,
    )
    .bind(application_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted_value, json!("reply:durable preview cache"));

    sqlx::query("delete from flow_runs where id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-start"]["query"],
        "durable preview cache"
    );
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:durable preview cache"
    );
}

#[tokio::test]
async fn debug_flow_run_persists_variable_cache_as_snapshot_source() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let started = start_debug_run(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "durable flow cache",
        "flow-cache-session",
    )
    .await;
    let application_uuid = Uuid::parse_str(&application_id).unwrap();
    let flow_run_id = Uuid::parse_str(started["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();

    wait_for_run_status(
        &app,
        &cookie,
        &application_id,
        &flow_run_id.to_string(),
        "succeeded",
    )
    .await;

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let persisted_value = wait_for_cache_value(&pool, application_uuid, "node-llm", "text").await;
    assert_eq!(persisted_value, json!("reply:durable flow cache"));

    sqlx::query("delete from flow_runs where id = $1")
        .bind(flow_run_id)
        .execute(&pool)
        .await
        .unwrap();

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:durable flow cache"
    );
}

#[tokio::test]
async fn debug_variable_snapshot_restores_current_cache_after_current_draft_document_changes() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let preview = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "old policy",
        "doc-session",
    )
    .await;
    let draft_id =
        Uuid::parse_str(preview["data"]["flow_run"]["draft_id"].as_str().unwrap()).unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"
        update flow_drafts
        set document = jsonb_set(document, '{meta,name}', to_jsonb('Updated Flow'::text), true),
            updated_at = now() + interval '1 hour'
        where id = $1
        "#,
    )
    .bind(draft_id)
    .execute(&pool)
    .await
    .unwrap();

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(snapshot["data"]["snapshot_completeness"], "complete");
    assert_eq!(
        snapshot["data"]["latest_run_scope"]["flow_run_id"],
        preview["data"]["flow_run"]["id"]
    );
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:old policy"
    );
    assert_eq!(snapshot["data"]["source_flow_run_ids"], json!({}));
    assert!(snapshot["data"]["source_node_run_ids"]["node-llm"]["text"].is_null());
}

#[tokio::test]
async fn debug_variable_snapshot_uses_flow_run_document_scope_after_compiled_plan_upsert() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let first_preview = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "old policy",
        "session-a",
    )
    .await;
    let draft_id = Uuid::parse_str(
        first_preview["data"]["flow_run"]["draft_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"
        update flow_drafts
        set document = jsonb_set(document, '{meta,name}', to_jsonb('New Flow Document'::text), true),
            updated_at = now() + interval '1 hour'
        where id = $1
        "#,
    )
    .bind(draft_id)
    .execute(&pool)
    .await
    .unwrap();

    start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "new policy",
        "session-b",
    )
    .await;

    let old_snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(old_snapshot["data"]["snapshot_completeness"], "complete");
    assert_eq!(
        old_snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:new policy"
    );

    let new_snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        new_snapshot["data"]["variable_cache"]["node-start"]["query"],
        "new policy"
    );
    assert_eq!(new_snapshot["data"]["source_flow_run_ids"], json!({}));
    assert_eq!(
        new_snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:new policy"
    );
}

#[tokio::test]
async fn debug_variable_snapshot_reports_latest_current_user_run_after_saved_draft_changes() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let first_preview = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "latest unsaved policy",
        "session-draft-independent",
    )
    .await;
    let draft_id = Uuid::parse_str(
        first_preview["data"]["flow_run"]["draft_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let flow_run_id = first_preview["data"]["flow_run"]["id"].as_str().unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"
        update flow_drafts
        set document = jsonb_set(
                jsonb_set(document, '{meta,name}', to_jsonb('Saved Draft Changed Later'::text), true),
                '{graph,nodes,1,outputs}',
                '[]'::jsonb,
                true
            ),
            updated_at = now() + interval '1 hour'
        where id = $1
        "#,
    )
    .bind(draft_id)
    .execute(&pool)
    .await
    .unwrap();

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["latest_run_scope"]["flow_run_id"],
        flow_run_id
    );
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:latest unsaved policy"
    );
}

#[tokio::test]
async fn debug_variable_snapshot_uses_durable_cache_instead_of_recomputing_node_runs() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let preview = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "first policy",
        "latest-node-session",
    )
    .await;
    let flow_run_id = Uuid::parse_str(preview["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();
    let replacement_node_run_id = Uuid::now_v7();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
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
            metrics_payload,
            debug_payload,
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
            'node-llm',
            'llm',
            'LLM',
            'succeeded',
            '{}'::jsonb,
            $3,
            '{}'::jsonb,
            '{}'::jsonb,
            now() + interval '1 second',
            now() + interval '1 second'
        )
        "#,
    )
    .bind(replacement_node_run_id)
    .bind(flow_run_id)
    .bind(json!({
        "text": "reply:newest policy",
        "usage": { "total_tokens": 128 },
        "provider_route": { "provider_code": "openai_compatible" }
    }))
    .execute(&pool)
    .await
    .unwrap();

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:first policy"
    );
    assert!(snapshot["data"]["variable_cache"]["node-llm"]["usage"].is_null());
    assert!(snapshot["data"]["variable_cache"]["node-llm"]["provider_route"].is_null());
    assert!(snapshot["data"]["source_node_run_ids"]["node-llm"]["text"].is_null());
    assert!(snapshot["data"]["source_node_run_ids"]["node-llm"]["usage"].is_null());
    assert!(snapshot["data"]["source_node_run_ids"]["node-llm"]["provider_route"].is_null());
}

#[tokio::test]
async fn debug_variable_snapshot_ignores_legacy_run_id_query() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let preview_a = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "policy A",
        "session-a",
    )
    .await;
    let preview_b = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "policy B",
        "session-b",
    )
    .await;

    let run_a = preview_a["data"]["flow_run"]["id"].as_str().unwrap();
    let run_b = preview_b["data"]["flow_run"]["id"].as_str().unwrap();

    let snapshot =
        get_snapshot_by_legacy_query(&app, &cookie, &application_id, &format!("run_id={run_a}"))
            .await;

    assert_eq!(snapshot["data"]["debug_session_id"], "");
    assert_eq!(snapshot["data"]["latest_run_scope"]["flow_run_id"], run_b);
    assert_ne!(snapshot["data"]["latest_run_scope"]["flow_run_id"], run_a);
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-start"]["query"],
        "policy B"
    );
    assert_eq!(snapshot["data"]["source_flow_run_ids"], json!({}));
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:policy B"
    );
}

#[tokio::test]
async fn debug_variable_snapshot_ignores_waiting_and_non_output_payload_buckets() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let preview = start_preview(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "bucket policy",
        "bucket-session",
    )
    .await;
    let flow_run_id = Uuid::parse_str(preview["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
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
            debug_payload,
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
            'node-llm',
            'llm',
            'LLM',
            'waiting_human',
            $3,
            $4,
            $5,
            $6,
            $7,
            now() + interval '1 second',
            null
        )
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(flow_run_id)
    .bind(json!({ "text": "input leak" }))
    .bind(json!({ "text": "waiting leak" }))
    .bind(json!({ "text": "error leak" }))
    .bind(json!({ "text": "metrics leak" }))
    .bind(json!({ "text": "debug leak" }))
    .execute(&pool)
    .await
    .unwrap();

    let snapshot = get_snapshot(&app, &cookie, &application_id).await;
    assert_eq!(
        snapshot["data"]["variable_cache"]["node-llm"]["text"],
        "reply:bucket policy"
    );
    assert_eq!(
        snapshot["data"]["source_node_run_ids"]["node-llm"]["text"],
        serde_json::Value::Null
    );
    assert!(snapshot["data"]["variable_cache"]["node-llm"]["input_payload"].is_null());
    assert!(snapshot["data"]["variable_cache"]["node-llm"]["metrics_payload"].is_null());
    assert!(snapshot["data"]["variable_cache"]["node-llm"]["debug_payload"].is_null());
    assert!(snapshot["data"]["variable_cache"]["node-llm"]["error_payload"].is_null());
}
