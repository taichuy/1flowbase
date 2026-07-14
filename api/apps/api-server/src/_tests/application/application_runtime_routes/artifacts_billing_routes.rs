use super::*;

#[tokio::test]
async fn resolve_runtime_debug_artifacts_rejects_unbounded_ref_batches() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let artifact_refs = (0..51)
        .map(|_| Uuid::now_v7().to_string())
        .collect::<Vec<_>>();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/resolve"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "artifact_refs": artifact_refs }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

fn build_answer_only_document(flow_id: &str, answer_text: &str) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id, "name": "Answer Only", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-answer",
                    "type": "answer",
                    "alias": "Answer",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "answer_template": { "kind": "templated_text", "value": answer_text }
                    },
                    "outputs": [{ "key": "answer", "title": "对话输出", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-answer", "source": "node-start", "target": "node-answer", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

async fn seed_answer_only_application(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    answer_text: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Answer Only",
                        "description": "runtime",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
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
    let application_id = payload["data"]["id"].as_str().unwrap().to_string();

    let state = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(state.status(), StatusCode::OK);
    let state_body = to_bytes(state.into_body(), usize::MAX).await.unwrap();
    let state_payload: Value = serde_json::from_slice(&state_body).unwrap();
    let flow_id = state_payload["data"]["draft"]["document"]["meta"]["flowId"]
        .as_str()
        .unwrap();

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/draft"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "document": build_answer_only_document(flow_id, answer_text),
                        "change_kind": "logical",
                        "summary": "seed answer-only flow"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(save.status(), StatusCode::OK);
    application_id
}

async fn wait_for_run_detail_matching(
    app: &axum::Router,
    cookie: &str,
    application_id: &str,
    run_id: &str,
    expected_statuses: &[&str],
    mut matches_detail: impl FnMut(&Value) -> bool,
    reason: &str,
) -> Value {
    let mut last_detail = Value::Null;
    for _ in 0..200 {
        let detail =
            wait_for_run_detail(app, cookie, application_id, run_id, expected_statuses).await;
        if matches_detail(&detail) {
            return detail;
        }
        last_detail = detail;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!("timed out waiting for {reason}: {last_detail}");
}

async fn wait_for_flow_run_status_in_database(
    pool: &sqlx::PgPool,
    flow_run_id: Uuid,
    expected_status: &str,
) {
    let mut last_status = String::new();
    for _ in 0..200 {
        last_status =
            sqlx::query_scalar::<_, String>("select status::text from flow_runs where id = $1")
                .bind(flow_run_id)
                .fetch_one(pool)
                .await
                .unwrap();
        if last_status == expected_status {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    panic!(
        "timed out waiting for run status in database: {expected_status}, last status: {last_status}"
    );
}

#[tokio::test]
async fn application_runtime_routes_start_debug_run_persists_gateway_billing_audit() {
    let (app, database_url) = test_app_with_database_url().await;
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

    let start_status = start.status();
    let start_body = to_bytes(start.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        start_status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&start_body)
    );
    let payload: Value = serde_json::from_slice(&start_body).unwrap();
    let run_id = Uuid::parse_str(payload["data"]["flow_run"]["id"].as_str().unwrap()).unwrap();
    let event_types = payload["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event["event_type"].as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"gateway_billing_session_reserved"));

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let (billing_count,): (i64,) =
        sqlx::query_as("select count(*) from billing_sessions where flow_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (cost_count,): (i64,) =
        sqlx::query_as("select count(*) from runtime_cost_ledger where flow_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (credit_count,): (i64,) =
        sqlx::query_as("select count(*) from runtime_credit_ledger where flow_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (audit_count,): (i64,) =
        sqlx::query_as("select count(*) from runtime_audit_hashes where flow_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(billing_count, 1);
    assert_eq!(cost_count, 1);
    assert_eq!(credit_count, 1);
    assert_eq!(audit_count, 3);
}

#[tokio::test]
async fn application_runtime_routes_runtime_debug_artifact_full_load_returns_original_payload() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let other_application_id =
        seed_agent_flow_application(&app, &cookie, &csrf, &provider_instance_id).await;
    let large_query = "退款政策".repeat(900);
    let debug_session_id = "runtime-debug-artifact-session";

    let response = app
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
                        "debug_session_id": debug_session_id,
                        "input_payload": {
                            "node-start": { "query": large_query }
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
    let run_id = payload["data"]["flow_run"]["id"].as_str().unwrap();

    let snapshot_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-variable-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot_response.status(), StatusCode::OK);
    let snapshot_body = to_bytes(snapshot_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let snapshot_payload: Value = serde_json::from_slice(&snapshot_body).unwrap();
    assert!(snapshot_payload["data"]["variable_cache"]["node-start"].is_null());

    let detail = wait_for_run_detail_matching(
        &app,
        &cookie,
        &application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
        |detail| detail["flow_run"]["input_payload"]["__runtime_debug_artifact"] == true,
        "flow input debug artifact preview",
    )
    .await;
    let preview = &detail["flow_run"]["input_payload"];

    assert_eq!(preview["__runtime_debug_artifact"], true);
    assert_eq!(preview["is_truncated"], true);
    assert!(preview["preview"].as_str().unwrap().len() < large_query.len());
    let artifact_ref = preview["artifact_ref"].as_str().unwrap();

    let unauthorized_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/{artifact_ref}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);

    let wrong_application_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{other_application_id}/orchestration/debug-artifacts/{artifact_ref}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_application_response.status(), StatusCode::NOT_FOUND);

    let artifact_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/{artifact_ref}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(artifact_response.status(), StatusCode::OK);
    assert_eq!(
        artifact_response.headers()["content-type"]
            .to_str()
            .unwrap(),
        "application/json"
    );
    let artifact_body = to_bytes(artifact_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let full_payload: Value = serde_json::from_slice(&artifact_body).unwrap();

    assert_eq!(full_payload["node-start"]["query"], large_query);
}

#[tokio::test]
async fn application_runtime_routes_batch_resolves_runtime_debug_artifacts() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let answer_text = format!("answer:{}", "A".repeat(3_000));
    let application_id = seed_answer_only_application(&app, &cookie, &csrf, &answer_text).await;
    let debug_session_id = "runtime-debug-artifact-batch-session";
    let large_query = "退款政策".repeat(900);

    let response = app
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
                        "debug_session_id": debug_session_id,
                        "input_payload": {
                            "node-start": { "query": large_query }
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
    let run_id = payload["data"]["flow_run"]["id"].as_str().unwrap();

    let detail = wait_for_run_detail_matching(
        &app,
        &cookie,
        &application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
        |detail| {
            detail["flow_run"]["input_payload"]["__runtime_debug_artifact"] == true
                && detail["flow_run"]["output_payload"]["answer"]["__runtime_debug_artifact"]
                    == true
        },
        "flow input and output artifact previews",
    )
    .await;
    let input_artifact_ref = detail["flow_run"]["input_payload"]["artifact_ref"]
        .as_str()
        .expect("flow input artifact ref should exist");
    let answer_artifact_ref = detail["flow_run"]["output_payload"]["answer"]["artifact_ref"]
        .as_str()
        .expect("answer artifact ref should exist");

    let unauthorized_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/resolve"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "artifact_refs": [input_artifact_ref, answer_artifact_ref] })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);

    let batch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/resolve"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "artifact_refs": [
                            input_artifact_ref,
                            answer_artifact_ref,
                            input_artifact_ref
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(batch_response.status(), StatusCode::OK);
    let batch_body = to_bytes(batch_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let batch_payload: Value = serde_json::from_slice(&batch_body).unwrap();
    let artifacts = batch_payload["data"]["artifacts"]
        .as_array()
        .expect("batch response should include artifacts");

    assert_eq!(artifacts.len(), 2);
    let input_artifact = artifacts
        .iter()
        .find(|artifact| artifact["artifact_ref"] == json!(input_artifact_ref))
        .expect("input artifact should be returned once");
    let answer_artifact = artifacts
        .iter()
        .find(|artifact| artifact["artifact_ref"] == json!(answer_artifact_ref))
        .expect("answer artifact should be returned once");
    assert_eq!(input_artifact["content_type"], json!("application/json"));
    assert_eq!(input_artifact["value"]["node-start"]["query"], large_query);
    assert_eq!(answer_artifact["value"], answer_text);

    let missing_artifact_ref = Uuid::new_v4().to_string();
    let missing_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/resolve"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "artifact_refs": [
                            input_artifact_ref,
                            missing_artifact_ref
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
    let missing_body = to_bytes(missing_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let missing_payload: Value = serde_json::from_slice(&missing_body).unwrap();
    assert!(missing_payload["code"].is_string());
    assert!(missing_payload["message"].is_string());
}

#[tokio::test]
async fn application_runtime_routes_flow_output_offloads_answer_field_without_compressing_sys_env()
{
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let answer_text = format!("answer:{}", "A".repeat(3_000));
    let application_id = seed_answer_only_application(&app, &cookie, &csrf, &answer_text).await;
    let debug_session_id = "runtime-debug-answer-artifact-session";

    let response = app
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
                        "debug_session_id": debug_session_id,
                        "input_payload": {
                            "node-start": { "query": "ping" }
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
    let run_id = payload["data"]["flow_run"]["id"].as_str().unwrap();

    let detail = wait_for_run_detail_matching(
        &app,
        &cookie,
        &application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
        |detail| {
            let has_answer_node = detail["nodes"]
                .as_array()
                .and_then(|node_runs| {
                    node_runs
                        .iter()
                        .find(|node_run| node_run["node_id"] == json!("node-answer"))
                })
                .is_some();
            detail["flow_run"]["output_payload"]["answer"]["__runtime_debug_artifact"] == true
                && has_answer_node
        },
        "flow output artifact preview and answer trace node",
    )
    .await;

    assert_eq!(detail["flow_run"]["status"], json!("succeeded"));
    let flow_output = &detail["flow_run"]["output_payload"];
    assert!(flow_output.get("__runtime_debug_artifact").is_none());
    assert_eq!(flow_output["answer"]["__runtime_debug_artifact"], true);
    assert_eq!(flow_output["answer"]["artifact_scope"], json!("field"));
    assert_eq!(flow_output["answer"]["field_path"], json!(["answer"]));
    assert!(flow_output["answer"]["preview"]
        .as_str()
        .expect("answer preview should be a string")
        .contains("answer:"));
    let answer_artifact_ref = flow_output["answer"]["artifact_ref"]
        .as_str()
        .expect("answer artifact ref should exist");
    let answer_artifact_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/debug-artifacts/{answer_artifact_ref}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(answer_artifact_response.status(), StatusCode::OK);
    let answer_artifact_body = to_bytes(answer_artifact_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let full_answer: Value = serde_json::from_slice(&answer_artifact_body).unwrap();
    assert_eq!(
        full_answer
            .as_str()
            .expect("answer artifact should be text"),
        answer_text
    );
    assert_eq!(flow_output["sys"]["workflow_run_id"], json!(run_id));
    assert_eq!(flow_output["env"], json!({}));

    let answer_trace_node_id = detail["nodes"]
        .as_array()
        .expect("trace nodes should be an array")
        .iter()
        .find(|node_run| node_run["node_id"] == json!("node-answer"))
        .expect("answer node should be present")["trace_node_id"]
        .as_str()
        .expect("answer trace node id should exist");
    let answer_node_detail_payload = load_trace_node_detail_payload_for_kind(
        &app,
        &cookie,
        &application_id,
        run_id,
        answer_trace_node_id,
        "node_run",
    )
    .await
    .expect("answer trace node should advertise a node_run detail ref");
    let answer_node_output =
        &answer_node_detail_payload["data"]["payload"]["node_run"]["output_payload"];
    assert_eq!(
        answer_node_output["answer"]["__runtime_debug_artifact"],
        true
    );
    assert_eq!(
        answer_node_output["answer"]["artifact_scope"],
        json!("field")
    );
    assert_eq!(
        answer_node_output["answer"]["field_path"],
        json!(["answer"])
    );
    assert_eq!(answer_node_output["sys"]["workflow_run_id"], json!(run_id));
    assert_eq!(answer_node_output["env"], json!({}));
}

#[tokio::test]
async fn application_runtime_routes_waiting_run_detail_reads_persisted_llm_rounds_as_plain_payload()
{
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider_instance_id = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application_id =
        seed_human_input_application(&app, &cookie, &csrf, &provider_instance_id).await;

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
    let run_id = start_payload["data"]["flow_run"]["id"].as_str().unwrap();
    let flow_run_id = Uuid::parse_str(run_id).unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    wait_for_flow_run_status_in_database(&pool, flow_run_id, "waiting_human").await;

    wait_for_run_detail(&app, &cookie, &application_id, run_id, &["waiting_human"]).await;
    let llm_last_run = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/logs/runs/{run_id}/nodes/node-llm"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(llm_last_run.status(), StatusCode::OK);
    let llm_last_run_body = to_bytes(llm_last_run.into_body(), usize::MAX)
        .await
        .unwrap();
    let llm_last_run_payload: Value = serde_json::from_slice(&llm_last_run_body).unwrap();
    let llm_rounds = &llm_last_run_payload["data"]["node_run"]["debug_payload"]["llm_rounds"];
    assert_eq!(llm_rounds["__runtime_debug_artifact"], Value::Null);

    let llm_rounds = llm_rounds.as_array().unwrap();
    assert!(!llm_rounds.is_empty());
    assert!(llm_rounds[0]["assistant"]["content"]
        .as_str()
        .unwrap()
        .contains("请总结退款政策"));

    let detail =
        wait_for_run_detail(&app, &cookie, &application_id, run_id, &["waiting_human"]).await;
    let llm_trace_node_id = detail["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node_run| node_run["node_id"].as_str() == Some("node-llm"))
        .expect("waiting run detail should include the LLM trace node")["trace_node_id"]
        .as_str()
        .expect("LLM trace node id should exist");
    let llm_detail_payload = load_trace_node_detail_payload_for_kind(
        &app,
        &cookie,
        &application_id,
        run_id,
        llm_trace_node_id,
        "node_run",
    )
    .await
    .expect("LLM trace node should advertise a node_run detail ref");
    let llm_rounds =
        &llm_detail_payload["data"]["payload"]["node_run"]["debug_payload"]["llm_rounds"];
    assert_eq!(llm_rounds, &Value::Null);
}
