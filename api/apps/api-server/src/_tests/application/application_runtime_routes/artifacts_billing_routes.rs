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
    let reservation_events = payload["data"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["event_type"] == "gateway_billing_session_reserved")
        .collect::<Vec<_>>();
    assert_eq!(reservation_events.len(), 1, "{payload:#}");
    let reservation = &reservation_events[0]["payload"];
    assert_eq!(reservation["billing_session"]["status"], "reserved");
    assert_eq!(reservation["cost_ledger"]["cost_status"], "pending_usage");
    assert_eq!(reservation["credit_ledger"]["transaction_type"], "reserve");
    for field in ["billing_session", "cost_ledger", "credit_ledger"] {
        Uuid::parse_str(reservation[field]["id"].as_str().unwrap()).unwrap();
    }

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    // The route returns the reservation snapshot while the real provider continues
    // asynchronously. Inspect the complete run, not a timing-dependent ledger total.
    wait_for_flow_run_status_in_database(&pool, run_id, "succeeded").await;
    let evidence: Value = sqlx::query_scalar(
        r#"select jsonb_build_object(
            'sessions', (select coalesce(jsonb_agg(to_jsonb(s)), '[]'::jsonb)
                from billing_sessions s where s.flow_run_id = $1),
            'costs', (select coalesce(jsonb_agg(to_jsonb(c)), '[]'::jsonb)
                from runtime_cost_ledger c where c.flow_run_id = $1),
            'credits', (select coalesce(jsonb_agg(to_jsonb(c)), '[]'::jsonb)
                from runtime_credit_ledger c where c.flow_run_id = $1),
            'audits', (select coalesce(jsonb_agg(to_jsonb(a)), '[]'::jsonb)
                from runtime_audit_hashes a where a.flow_run_id = $1),
            'usage', (select coalesce(jsonb_agg(to_jsonb(u)), '[]'::jsonb)
                from runtime_usage_ledger u where u.flow_run_id = $1),
            'attempts', (select coalesce(jsonb_agg(to_jsonb(a)), '[]'::jsonb)
                from model_failover_attempt_ledger a where a.flow_run_id = $1),
            'nodes', (select coalesce(jsonb_agg(to_jsonb(n)), '[]'::jsonb)
                from node_runs n where n.flow_run_id = $1)
        )"#,
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let diagnostics = format!("reservation={reservation:#}\ncompleted_run={evidence:#}");
    let sessions = evidence["sessions"].as_array().unwrap();
    let costs = evidence["costs"].as_array().unwrap();
    let credits = evidence["credits"].as_array().unwrap();
    let audits = evidence["audits"].as_array().unwrap();
    let usage = evidence["usage"].as_array().unwrap();
    let attempts = evidence["attempts"].as_array().unwrap();

    // Keep exact gateway reservation obligations, including identity and hash links.
    assert_eq!(sessions.len(), 1, "{diagnostics}");
    let session = &sessions[0];
    assert_eq!(
        session["id"], reservation["billing_session"]["id"],
        "{diagnostics}"
    );
    assert_eq!(
        session["reserved_credit_ledger_id"], reservation["credit_ledger"]["id"],
        "{diagnostics}"
    );
    assert_eq!(
        session["idempotency_key"],
        json!(format!("gateway:{run_id}:reserve")),
        "{diagnostics}"
    );
    // Default-free root execution must not introduce a debit or another reservation.
    assert_eq!(credits.len(), 1, "{diagnostics}");
    let credit = &credits[0];
    assert_eq!(
        credit["id"], reservation["credit_ledger"]["id"],
        "{diagnostics}"
    );
    assert_eq!(
        credit["cost_ledger_id"], reservation["cost_ledger"]["id"],
        "{diagnostics}"
    );
    assert_eq!(credit["transaction_type"], "reserve", "{diagnostics}");
    assert_eq!(
        credit["reason"], "gateway_billing_session_reserved",
        "{diagnostics}"
    );
    assert_eq!(credit["amount"], json!(0), "{diagnostics}");
    assert_eq!(credit["credit_unit"], "credit", "{diagnostics}");
    assert_eq!(
        credit["idempotency_key"], session["idempotency_key"],
        "{diagnostics}"
    );
    assert!(
        session["settled_credit_ledger_id"].is_null(),
        "{diagnostics}"
    );
    assert!(
        session["refund_credit_ledger_id"].is_null(),
        "{diagnostics}"
    );

    let gateway_costs = costs
        .iter()
        .filter(|cost| cost["cost_source"] == "gateway_reservation")
        .collect::<Vec<_>>();
    assert_eq!(gateway_costs.len(), 1, "{diagnostics}");
    let gateway_cost = gateway_costs[0];
    assert_eq!(
        gateway_cost["id"], reservation["cost_ledger"]["id"],
        "{diagnostics}"
    );
    assert_eq!(
        gateway_cost["cost_status"], "pending_usage",
        "{diagnostics}"
    );
    assert_eq!(gateway_cost["raw_cost"], json!(0), "{diagnostics}");
    assert_eq!(gateway_cost["normalized_cost"], json!(0), "{diagnostics}");
    assert_eq!(
        gateway_cost["settlement_currency"], "credit",
        "{diagnostics}"
    );
    assert_eq!(
        gateway_cost["gateway_route_id"], reservation["route_trace"]["route_id"],
        "{diagnostics}"
    );
    assert_eq!(
        session["route_id"], gateway_cost["gateway_route_id"],
        "{diagnostics}"
    );
    // The reservation snapshots the model route at preflight; execution selects
    // the provider instance later. Require explicit nulls, not missing fields.
    assert_eq!(
        gateway_cost.get("provider_instance_id"),
        Some(&Value::Null),
        "{diagnostics}"
    );
    assert_eq!(
        reservation["route_trace"].get("provider_instance_id"),
        Some(&Value::Null),
        "{diagnostics}"
    );
    assert!(gateway_cost["usage_ledger_id"].is_null(), "{diagnostics}");
    assert!(
        gateway_cost["billing_session_id"].is_null(),
        "{diagnostics}"
    );
    assert_eq!(audits.len(), 3, "{diagnostics}");
    let mut audit_ids = audits
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let mut event_audit_ids = reservation["audit_hashes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect::<Vec<_>>();
    audit_ids.sort_unstable();
    event_audit_ids.sort_unstable();
    assert_eq!(audit_ids, event_audit_ids, "{diagnostics}");
    for (table, id) in [
        ("runtime_cost_ledger", &gateway_cost["id"]),
        ("runtime_credit_ledger", &credit["id"]),
        ("billing_sessions", &session["id"]),
    ] {
        assert_eq!(
            audits
                .iter()
                .filter(|row| row["fact_table"] == table && &row["fact_id"] == id)
                .count(),
            1,
            "{diagnostics}"
        );
    }

    // Exactly one provider observation accounts for the second cost; duplicate
    // invocation/writes and missing execution must all fail independently.
    assert_eq!(costs.len(), 2, "{diagnostics}");
    assert_eq!(usage.len(), 1, "{diagnostics}");
    assert_eq!(attempts.len(), 1, "{diagnostics}");
    let observation_costs = costs
        .iter()
        .filter(|cost| cost["cost_source"] == "local_token_pricing")
        .collect::<Vec<_>>();
    assert_eq!(observation_costs.len(), 1, "{diagnostics}");
    let observation = observation_costs[0];
    let usage = &usage[0];
    let attempt = &attempts[0];
    assert_ne!(observation["id"], gateway_cost["id"], "{diagnostics}");
    assert_eq!(observation["cost_status"], "rated", "{diagnostics}");
    assert_eq!(observation["settlement_currency"], "USD", "{diagnostics}");
    assert!(observation["billing_session_id"].is_null(), "{diagnostics}");
    assert_eq!(observation["usage_ledger_id"], usage["id"], "{diagnostics}");
    for row in [observation, usage, attempt] {
        assert_eq!(
            row["provider_instance_id"],
            json!(provider_instance_id),
            "{diagnostics}"
        );
    }
    assert_eq!(usage["model_id"], "fixture_chat", "{diagnostics}");
    assert_eq!(usage["usage_status"], "recorded", "{diagnostics}");
    assert_eq!(usage["input_tokens"], json!(5), "{diagnostics}");
    assert_eq!(usage["output_tokens"], json!(7), "{diagnostics}");
    assert_eq!(usage["total_tokens"], json!(12), "{diagnostics}");
    assert_eq!(attempt["status"], "succeeded", "{diagnostics}");
    assert_eq!(attempt["usage_ledger_id"], usage["id"], "{diagnostics}");
    assert_eq!(
        attempt["cost_ledger_id"], observation["id"],
        "{diagnostics}"
    );
    assert_eq!(
        attempt["node_run_id"], usage["node_run_id"],
        "{diagnostics}"
    );
    let llm_nodes = evidence["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["node_id"] == "node-llm")
        .collect::<Vec<_>>();
    assert_eq!(llm_nodes.len(), 1, "{diagnostics}");
    assert_eq!(usage["node_run_id"], llm_nodes[0]["id"], "{diagnostics}");
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

    let _terminal_detail = wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    let snapshot_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"
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
    let detail = &snapshot_payload["data"];
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

    let terminal_detail = wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    assert_eq!(terminal_detail["flow_run"]["status"], json!("succeeded"));
    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot_body = to_bytes(snapshot.into_body(), usize::MAX).await.unwrap();
    let snapshot_payload: Value = serde_json::from_slice(&snapshot_body).unwrap();
    let detail = &snapshot_payload["data"];
    assert_eq!(
        detail["flow_run"]["input_payload"]["__runtime_debug_artifact"],
        true
    );
    assert_eq!(
        detail["flow_run"]["output_payload"]["answer"]["__runtime_debug_artifact"],
        true
    );
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
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
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

    let terminal_detail = wait_for_run_detail(
        &app,
        &cookie,
        &application_id,
        run_id,
        &["succeeded", "failed", "cancelled"],
    )
    .await;
    assert_eq!(terminal_detail["flow_run"]["status"], json!("succeeded"));
    let snapshot = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/runs/{run_id}/debug-snapshot"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot_body = to_bytes(snapshot.into_body(), usize::MAX).await.unwrap();
    let snapshot_payload: Value = serde_json::from_slice(&snapshot_body).unwrap();
    let detail = &snapshot_payload["data"];
    assert_eq!(
        detail["flow_run"]["output_payload"]["answer"]["__runtime_debug_artifact"],
        true
    );
    rebuild_trace_projection_for_test(
        &state,
        Uuid::parse_str(&application_id).unwrap(),
        Uuid::parse_str(run_id).unwrap(),
    )
    .await;
    let trace_detail =
        wait_for_run_detail(&app, &cookie, &application_id, run_id, &["succeeded"]).await;
    assert!(trace_detail["nodes"]
        .as_array()
        .and_then(|nodes| nodes
            .iter()
            .find(|node| node["node_id"] == json!("node-answer")))
        .is_some());

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

    let answer_trace_node_id = trace_detail["nodes"]
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
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
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

    rebuild_trace_projection_for_test(
        &state,
        Uuid::parse_str(&application_id).unwrap(),
        flow_run_id,
    )
    .await;
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
