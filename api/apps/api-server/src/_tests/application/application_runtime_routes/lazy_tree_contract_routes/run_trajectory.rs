use super::*;

pub(super) async fn get(
    app: &axum::Router,
    cookie: Option<&str>,
    path: &str,
) -> (StatusCode, Value) {
    let mut request = Request::builder().uri(path);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)})),
    )
}

#[tokio::test]
async fn application_runtime_run_trajectory_and_payload_sections_are_scoped_lazy_and_authorized() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let provider = create_ready_provider_instance(&app, &cookie, &csrf).await;
    let application = seed_agent_flow_application(&app, &cookie, &csrf, &provider).await;
    let preview = start_llm_preview(&app, &cookie, &csrf, &application, "section fixture").await;
    let run = preview["data"]["flow_run"]["id"].as_str().unwrap();
    let run_id = Uuid::parse_str(run).unwrap();
    let node = Uuid::parse_str(preview["data"]["node_run"]["id"].as_str().unwrap()).unwrap();
    let base = format!("/api/console/applications/{application}/logs/runs/{run}");
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let original = json!({"text":"actual NUL \0 and literal \\u0000"});
    sqlx::query("update flow_runs set input_payload='{}',raw_json_payloads=jsonb_build_object('input_payload',$2::text,'output_payload','invalid JSON') where id=$1")
        .bind(run_id).bind(original.to_string()).execute(&pool).await.unwrap();
    sqlx::query("update node_runs set raw_json_payloads=jsonb_build_object('input_payload','invalid JSON') where flow_run_id=$1")
        .bind(run_id).execute(&pool).await.unwrap();
    let (status, input) = get(
        &app,
        Some(&cookie),
        &format!("{base}/payloads/input_payload"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{input}");
    assert_eq!(input["data"], original);
    assert_eq!(
        get(
            &app,
            Some(&cookie),
            &format!("{base}/payloads/error_payload")
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // Seed two summaries independently of provider capture configuration.
    seed_flow_run_history_events(&database_url, &(0..2).map(|index| AppendRuntimeEventInput {
        flow_run_id: run_id, node_run_id: Some(node), span_id: None, parent_span_id: None,
        event_type: "provider_semantic_step".into(), layer: domain::RuntimeEventLayer::RuntimeItem,
        source: domain::RuntimeEventSource::Host, trust_level: domain::RuntimeTrustLevel::HostFact,
        item_id: None, ledger_ref: None,
        payload: json!({"source":"ai_native","invocation_id":"section-fixture","provider_attempt_index":0,"step_key":format!("step-{index}"),"kind":"model_reply","status":"recorded","preview":"Hello","body":"{\"text\":\"Hello\"}"}),
        visibility: domain::RuntimeEventVisibility::Internal, durability: domain::RuntimeEventDurability::Durable,
    }).collect::<Vec<_>>()).await.unwrap();
    let (status, page) = get(&app, Some(&cookie), &format!("{base}/trajectory?limit=1")).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["data"]["items"].as_array().unwrap().len(), 1);
    let cursor = page["data"]["next_cursor"].as_i64().unwrap();
    let (status, next) = get(
        &app,
        Some(&cookie),
        &format!("{base}/trajectory?limit=1&cursor={cursor}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{next}");
    assert!(next["data"]["items"][0]["event_sequence"].as_i64().unwrap() > cursor);
    let item = &page["data"]["items"][0];
    assert_eq!(item["metadata"]["flow_run_id"], run);
    assert!(item["metadata"].get("body").is_none());
    let event = item["event_id"].as_str().unwrap();
    let node_id = item["metadata"]["node_run_id"].as_str().unwrap();
    assert_eq!(
        get(
            &app,
            Some(&cookie),
            &format!("{base}/nodes/{node_id}/trajectory/{event}")
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        get(
            &app,
            Some(&cookie),
            &format!("{base}/nodes/{}/trajectory/{event}", Uuid::now_v7())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    for suffix in ["trajectory", "payloads/input_payload"] {
        assert_eq!(
            get(&app, None, &format!("{base}/{suffix}")).await.0,
            StatusCode::UNAUTHORIZED
        );
        let missing = format!(
            "/api/console/applications/{application}/logs/runs/{}/{suffix}",
            Uuid::now_v7()
        );
        assert_eq!(
            get(&app, Some(&cookie), &missing).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let member = format!("trajectory-denied-{}", Uuid::now_v7().simple());
    let member_id =
        crate::_tests::support::create_member(&app, &cookie, &csrf, &member, "temp-pass").await;
    crate::_tests::support::replace_member_roles(&app, &cookie, &csrf, &member_id, &[]).await;
    let (member_cookie, _) = login_and_capture_cookie(&app, &member, "temp-pass").await;
    for suffix in ["trajectory", "payloads/input_payload"] {
        assert_eq!(
            get(&app, Some(&member_cookie), &format!("{base}/{suffix}"))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }

    let output = json!({"text":"large output \0 ".repeat(1000)});
    sqlx::query("update flow_runs set raw_json_payloads=jsonb_build_object('output_payload',$2::text,'input_payload','invalid JSON') where id=$1")
        .bind(run_id).bind(output.to_string()).execute(&pool).await.unwrap();
    let before: i64 =
        sqlx::query_scalar("select count(*) from runtime_debug_artifacts where flow_run_id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let (status, selected) = get(
        &app,
        Some(&cookie),
        &format!("{base}/payloads/output_payload"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{selected}");
    assert_eq!(selected["data"]["__runtime_debug_artifact"], true);
    assert!(selected["data"]["preview"].as_str().unwrap().len() <= 2048);
    let artifact = selected["data"]["artifact_ref"].as_str().unwrap();
    let (status, resolved) = get(
        &app,
        Some(&cookie),
        &format!(
            "/api/console/applications/{application}/orchestration/debug-artifacts/{artifact}"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved, output);
    let after: i64 =
        sqlx::query_scalar("select count(*) from runtime_debug_artifacts where flow_run_id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, before + 1);
    let retained: Value = sqlx::query_scalar("select raw_json_payloads from flow_runs where id=$1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(retained["input_payload"], "invalid JSON");
    assert_eq!(retained["output_payload"], output.to_string());

    // Existing artifact references pass through without generating another artifact.
    sqlx::query("update flow_runs set output_payload=$2,raw_json_payloads='{}' where id=$1")
        .bind(run_id)
        .bind(&selected["data"])
        .execute(&pool)
        .await
        .unwrap();
    let (status, existing) = get(
        &app,
        Some(&cookie),
        &format!("{base}/payloads/output_payload"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(existing["data"], selected["data"]);
    let final_count: i64 =
        sqlx::query_scalar("select count(*) from runtime_debug_artifacts where flow_run_id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(final_count, after);
}
