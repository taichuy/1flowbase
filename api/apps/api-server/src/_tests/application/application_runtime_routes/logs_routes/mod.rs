use super::*;
use control_plane::ports::{
    AppendRuntimeEventInput, CreateCallbackTaskInput, OrchestrationRuntimeRepository,
    UpdateFlowRunInput,
};
use storage_durable_postgres::MainDurableStore;

async fn get_console_json(app: &axum::Router, cookie: &str, uri: String) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
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

// Fund only this isolated fixture through the existing billing API.
async fn fund_log_fixture(app: &axum::Router, cookie: &str, csrf: &str) {
    let session = get_console_json(app, cookie, "/api/console/session".to_owned()).await;
    let user_id = session["data"]["actor"]["id"].as_str().unwrap();
    let response = app.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/console/settings/billing/credits/{user_id}/grant"))
        .header("cookie", cookie).header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(Body::from(json!({"amount":"100", "reason":"original log fixture", "source_type":"test", "source_id":"original-log", "idempotency_key":"original-log-credit"}).to_string()))
        .unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn start_full_debug_run(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    query: &str,
) -> String {
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
                            "node-start": {
                                "query": query
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["flow_run"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn latest_llm_node_run_id(pool: &sqlx::PgPool, flow_run_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        select id
        from node_runs
        where flow_run_id = $1
          and node_type = 'llm'
        order by started_at desc, id desc
        limit 1
        "#,
    )
    .bind(flow_run_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

mod export;
mod lazy_trace_content;
mod pagination;
mod preview_and_trace;
mod statistics;
mod stitched_history;
mod visible_internal_trace;

mod rework;
