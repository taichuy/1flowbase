use super::*;
use control_plane::ports::{
    AppendRuntimeEventInput, CompleteCallbackTaskInput, CreateCallbackTaskInput,
    CreateNodeRunInput, OrchestrationRuntimeRepository, UpdateNodeRunInput,
};
use storage_durable_postgres::MainDurableStore;

async fn start_llm_preview(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    query: &str,
) -> Value {
    let preview = app
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
                            "node-start": { "query": query }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = preview.status();
    let body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_node_content_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
    trace_node_id: &str,
) -> Value {
    load_trace_node_content_payload_with_query(
        app,
        cookie,
        application_id,
        flow_run_id,
        trace_node_id,
        "",
    )
    .await
}

async fn load_trace_node_content_payload_with_query(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
    trace_node_id: &str,
    query: &str,
) -> Value {
    let content = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{trace_node_id}/content{query}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = content.status();
    let body = to_bytes(content.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_tree_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
) -> Value {
    let trace_tree = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = trace_tree.status();
    let body = to_bytes(trace_tree.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_node_children_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
    parent_trace_node_id: &str,
) -> Value {
    let children = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes?parent_trace_node_id={parent_trace_node_id}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = children.status();
    let body = to_bytes(children.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_node_detail_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
    trace_node_id: &str,
    detail_ref_id: &str,
) -> Value {
    load_trace_node_detail_payload_with_query(
        app,
        cookie,
        application_id,
        flow_run_id,
        trace_node_id,
        detail_ref_id,
        "",
    )
    .await
}

async fn load_trace_node_detail_payload_with_query(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
    trace_node_id: &str,
    detail_ref_id: &str,
    query: &str,
) -> Value {
    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{flow_run_id}/trace-tree/nodes/{trace_node_id}/details/{detail_ref_id}{query}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = detail.status();
    let body = to_bytes(detail.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    serde_json::from_slice(&body).unwrap()
}

async fn load_trace_node_node_run_detail_payload(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    flow_run_id: &str,
    trace_node_id: &str,
) -> Value {
    let content_payload =
        load_trace_node_content_payload(app, cookie, application_id, flow_run_id, trace_node_id)
            .await;
    assert!(
        content_payload["data"]["payload"].get("node_run").is_none(),
        "trace node content must not duplicate the full node_run in the raw payload"
    );
    let detail_ref_id = content_payload["data"]["detail_refs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|detail_ref| detail_ref["detail_kind"] == json!("node_run"))
        .expect("node_run detail ref should be advertised")["detail_ref_id"]
        .as_str()
        .expect("node_run detail ref id should be a string");
    let detail_payload = load_trace_node_detail_payload(
        app,
        cookie,
        application_id,
        flow_run_id,
        trace_node_id,
        detail_ref_id,
    )
    .await;

    detail_payload["data"]["payload"]["node_run"].clone()
}

mod node_detail_refs;
mod overview_and_snapshot;
mod repeated_llm_groups;
mod route_provider_events;
mod tool_detail_loading;
