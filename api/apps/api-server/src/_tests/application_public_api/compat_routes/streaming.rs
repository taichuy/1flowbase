use super::*;
use control_plane::ports::RuntimeEventCloseReason;

async fn wait_for_active_streaming_run(state: &ApiState) -> uuid::Uuid {
    timeout(Duration::from_secs(3), async {
        loop {
            let run_id = sqlx::query_scalar::<_, uuid::Uuid>(
                "select id from flow_runs where status = 'running' order by created_at desc, id desc limit 1",
            )
            .fetch_optional(state.store.pool())
            .await
            .unwrap();
            if let Some(run_id) = run_id {
                return run_id;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("delayed provider should leave the public streaming run active")
}

async fn wait_for_provider_partial_delta(
    state: &ApiState,
    run_id: uuid::Uuid,
    gate: &ProviderInvocationGate,
) {
    timeout(Duration::from_secs(3), async {
        loop {
            if gate.is_ready() {
                let replay = state
                    .runtime_event_stream
                    .replay(run_id, None, 64)
                    .await
                    .expect("gated stream should replay its provider delta");
                if replay.iter().any(|event| {
                    event.event_type == "text_delta" && event.text.as_deref() == Some("partial")
                }) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Provider must enter invoke and publish a partial delta before cancellation");
}

async fn cancel_active_streaming_run_and_collect_sse(
    app: &Router,
    state: &ApiState,
    token: &str,
    gate: &ProviderInvocationGate,
    response: axum::response::Response,
) -> (uuid::Uuid, String) {
    let run_id = wait_for_active_streaming_run(state).await;
    wait_for_provider_partial_delta(state, run_id, gate).await;
    let cancel = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/agent/v1/runs/{run_id}/cancel"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    gate.release();
    assert_eq!(cancel.status(), StatusCode::OK);
    let cancel = response_json(cancel).await;
    assert_eq!(cancel["data"]["status"], json!("cancelled"));
    assert!(cancel["data"]["answer"].is_null());
    assert_eq!(cancel["data"]["error"]["code"], json!("cancelled"));

    let latest = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/agent/v1/runs/{run_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(latest.status(), StatusCode::OK);
    let latest = response_json(latest).await;
    assert_eq!(latest["data"]["status"], json!("cancelled"));
    assert!(latest["data"]["answer"].is_null());
    assert_eq!(latest["data"]["error"]["code"], json!("cancelled"));

    let subscription = state
        .runtime_event_stream
        .subscribe(run_id, None)
        .await
        .expect("cancelled stream remains inspectable");
    assert_eq!(
        subscription
            .closure
            .borrow()
            .as_ref()
            .map(|closure| closure.reason),
        Some(RuntimeEventCloseReason::Cancelled)
    );
    assert_eq!(
        subscription
            .replay
            .iter()
            .filter(|event| event.event_type == "flow_cancelled")
            .count(),
        1,
        "live runtime stream must have one cancellation terminal"
    );

    let body = timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("cancelled SSE should close after its terminal event")
    .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    // The delayed Provider may return after cancellation. Its result must not overwrite the
    // canonical cancellation winner or add a second durable terminal.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let status = sqlx::query_scalar::<_, String>("select status from flow_runs where id = $1")
        .bind(run_id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let output =
        sqlx::query_scalar::<_, Value>("select output_payload from flow_runs where id = $1")
            .bind(run_id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    let error =
        sqlx::query_scalar::<_, Option<Value>>("select error_payload from flow_runs where id = $1")
            .bind(run_id)
            .fetch_one(state.store.pool())
            .await
            .unwrap()
            .expect("cancelled winner should retain a safe durable error");
    let durable_terminals = sqlx::query_scalar::<_, i64>(
        "select count(*) from flow_run_events where flow_run_id = $1 and event_type = 'flow_cancelled'",
    )
    .bind(run_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(status, "cancelled");
    assert_eq!(output, json!({}));
    assert_eq!(error["code"], json!("cancelled"));
    assert_eq!(durable_terminals, 1);

    (run_id, body)
}

#[tokio::test]
async fn d2_ac_004_native_streaming_cancel_projects_one_safe_terminal_and_closes() {
    let (app, state) = test_app_with_state().await;
    let (token, gate) =
        setup_published_app_with_provider_gate(&app, "Native Actual Cancel SSE App").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/v1/runs")
                .header("authorization", format!("Bearer {token}"))
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "query": "cancel this delayed native stream",
                        "response_mode": "streaming",
                        "stream_options": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let (_, body) =
        cancel_active_streaming_run_and_collect_sse(&app, state.as_ref(), &token, &gate, response)
            .await;

    assert!(body.contains("partial"), "{body}");
    assert!(body.contains("event: run.cancelled"), "{body}");
    assert!(body.contains("\"code\":\"run_cancelled\""), "{body}");
    assert!(!body.contains("event: run.completed"), "{body}");
    assert!(!body.contains("event: run.incomplete"), "{body}");
}

#[tokio::test]
async fn d2_ac_004_openai_chat_streaming_cancel_projects_error_without_done() {
    let (app, state) = test_app_with_state().await;
    let (token, gate) =
        setup_published_app_with_provider_gate(&app, "OpenAI Chat Actual Cancel SSE App").await;

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(true),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let (_, body) =
        cancel_active_streaming_run_and_collect_sse(&app, state.as_ref(), &token, &gate, response)
            .await;

    assert!(body.contains("partial"), "{body}");
    assert!(body.contains("\"code\":\"run_cancelled\""), "{body}");
    assert!(!body.contains("\"finish_reason\":\"stop\""), "{body}");
    assert!(!body.contains("[DONE]"), "{body}");
}

#[tokio::test]
async fn d2_ac_004_openai_responses_streaming_cancel_projects_failed_without_completed() {
    let (app, state) = test_app_with_state().await;
    let (token, gate) =
        setup_published_app_with_provider_gate(&app, "OpenAI Responses Actual Cancel SSE App")
            .await;

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(true),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let (_, body) =
        cancel_active_streaming_run_and_collect_sse(&app, state.as_ref(), &token, &gate, response)
            .await;

    assert!(body.contains("partial"), "{body}");
    assert!(body.contains("event: response.failed"), "{body}");
    assert!(body.contains("\"code\":\"run_cancelled\""), "{body}");
    assert!(!body.contains("event: response.completed"), "{body}");
}

#[tokio::test]
async fn d2_ac_004_anthropic_streaming_cancel_projects_error_without_message_stop() {
    let (app, state) = test_app_with_state().await;
    let (token, gate) =
        setup_published_app_with_provider_gate(&app, "Anthropic Actual Cancel SSE App").await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token.clone()),
        anthropic_body(true),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let (_, body) =
        cancel_active_streaming_run_and_collect_sse(&app, state.as_ref(), &token, &gate, response)
            .await;

    assert!(body.contains("partial"), "{body}");
    assert!(body.contains("event: error"), "{body}");
    assert!(body.contains("published run cancelled"), "{body}");
    assert!(!body.contains("event: message_stop"), "{body}");
    assert!(!body.contains("\"stop_reason\":\"end_turn\""), "{body}");
    assert!(!body.contains("\"stop_reason\":\"max_tokens\""), "{body}");
}

#[tokio::test]
async fn d2_ac_007_anthropic_blocking_preserves_marker_like_provider_output() {
    let app = test_app().await;
    let token = setup_published_app_with_marker_output_provider(
        &app,
        "Anthropic Blocking Marker Output App",
    )
    .await;

    let response = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        anthropic_body(false),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["type"], json!("message"));
    assert_eq!(
        payload["content"][0]["text"],
        json!(PROVIDER_MARKER_LIKE_OUTPUT),
        "blocking projection must not reinterpret Provider output as a protocol control marker"
    );
}

#[tokio::test]
async fn compatible_streaming_routes_return_protocol_sse() {
    let app = test_app().await;
    let token = setup_published_app(&app, "Compatible Streaming Route App").await;

    let openai = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(true),
    )
    .await;
    assert_eq!(openai.status(), StatusCode::OK);
    assert_eq!(
        openai.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let openai_body = timeout(
        Duration::from_secs(5),
        to_bytes(openai.into_body(), usize::MAX),
    )
    .await
    .expect("OpenAI compatible SSE should finish")
    .unwrap();
    let openai_body = String::from_utf8(openai_body.to_vec()).unwrap();
    assert!(openai_body.contains("[DONE]"), "{openai_body}");
    assert!(
        !openai_body.contains("event: workflow.event"),
        "{openai_body}"
    );

    let responses = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(true),
    )
    .await;
    assert_eq!(responses.status(), StatusCode::OK);
    assert_eq!(
        responses.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let responses_body = timeout(
        Duration::from_secs(5),
        to_bytes(responses.into_body(), usize::MAX),
    )
    .await
    .expect("OpenAI Responses SSE should finish")
    .unwrap();
    let responses_body = String::from_utf8(responses_body.to_vec()).unwrap();
    assert!(
        responses_body.contains("event: response.created"),
        "{responses_body}"
    );
    assert!(
        responses_body.contains("event: response.completed")
            || responses_body.contains("event: response.failed"),
        "{responses_body}"
    );
    assert!(
        !responses_body.contains("event: workflow.event"),
        "{responses_body}"
    );

    let anthropic = post_json(
        &app,
        "/v1/messages",
        ("x-api-key", token),
        anthropic_body(true),
    )
    .await;
    assert_eq!(anthropic.status(), StatusCode::OK);
    assert_eq!(
        anthropic.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let anthropic_body = timeout(
        Duration::from_secs(5),
        to_bytes(anthropic.into_body(), usize::MAX),
    )
    .await
    .expect("Anthropic compatible SSE should finish")
    .unwrap();
    let anthropic_body = String::from_utf8(anthropic_body.to_vec()).unwrap();
    assert_eq!(
        anthropic_body.matches("event: message_start").count(),
        1,
        "{anthropic_body}"
    );
    assert!(
        anthropic_body.contains("event: message_stop") || anthropic_body.contains("event: error"),
        "{anthropic_body}"
    );
    assert!(
        !anthropic_body.contains("event: workflow.event"),
        "{anthropic_body}"
    );
}

#[tokio::test]
async fn d2_ac_007_openai_chat_streaming_tool_continuation_is_rejected_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Streaming Tool Resume App").await;
    let before = flow_run_count(state.as_ref()).await;
    let tool_call_id = encode_openai_callback_tool_call_id(
        uuid::Uuid::from_u128(0x22222222222222222222222222222222),
        "call_inventory",
    );

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "stream": true,
            "messages": [
                {
                    "role": "user",
                    "content": "Find inventory"
                },
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": tool_call_id,
                        "type": "function",
                        "function": {
                            "name": "lookup_inventory",
                            "arguments": "{\"sku\":\"sku_123\"}"
                        }
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": "{\"stock\":7}"
                }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("unsupported_feature"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn d2_ac_007_openai_chat_nul_tool_continuation_is_rejected_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Streaming NUL Tool Resume App").await;
    let before = flow_run_count(state.as_ref()).await;
    let tool_call_id = encode_openai_callback_tool_call_id(
        uuid::Uuid::from_u128(0x11111111111111111111111111111111),
        "call_inventory",
    );

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "stream": true,
            "messages": [
                {
                    "role": "user",
                    "content": "Find inventory"
                },
                {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": tool_call_id,
                        "type": "function",
                        "function": {
                            "name": "lookup_inventory",
                            "arguments": "{\"sku\":\"sku_123\"}"
                        }
                    }]
                },
                {
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": "STDERR:\n\0after"
                }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("unsupported_feature"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn d2_ac_007_openai_responses_streaming_tool_continuation_is_rejected_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Streaming Tool Resume App").await;
    let before = flow_run_count(state.as_ref()).await;
    let previous_response_id = "resp_33333333-3333-3333-3333-333333333333";
    let call_id = encode_openai_callback_tool_call_id(
        uuid::Uuid::from_u128(0x44444444444444444444444444444444),
        "call_inventory",
    );

    let response = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "stream": true,
            "previous_response_id": previous_response_id,
            "input": [
                {
                    "type": "function_call_output",
                    "call_id": call_id,
                    "output": {"stock": 7}
                }
            ]
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["error"]["code"], json!("unsupported_feature"));
    assert_eq!(payload["error"]["param"], json!("previous_response_id"));
    assert_eq!(flow_run_count(state.as_ref()).await, before);
}

#[tokio::test]
async fn compatible_streaming_routes_emit_terminal_fallback_after_runtime_stream_closes() {
    let (app, _) =
        test_app_with_runtime_event_stream(Arc::new(DropTerminalRuntimeEventStream::new())).await;
    let token = setup_published_app(&app, "Compatible Terminal Fallback Route App").await;

    let openai = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(true),
    )
    .await;
    assert_eq!(openai.status(), StatusCode::OK);

    let openai_body = timeout(
        Duration::from_secs(5),
        to_bytes(openai.into_body(), usize::MAX),
    )
    .await
    .expect("OpenAI compatible SSE should finish from durable terminal fallback")
    .unwrap();
    let openai_body = String::from_utf8(openai_body.to_vec()).unwrap();

    assert!(openai_body.contains("[DONE]"), "{openai_body}");
}

#[tokio::test]
async fn compatible_streaming_routes_do_not_poll_durable_terminal_while_runtime_stream_stays_open()
{
    let (app, _) = test_app_with_runtime_event_stream(Arc::new(
        NeverCloseDropTerminalRuntimeEventStream::new(),
    ))
    .await;
    let token = setup_published_app(&app, "Compatible Stuck Runtime Stream Route App").await;

    let openai = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(true),
    )
    .await;
    assert_eq!(openai.status(), StatusCode::OK);

    let openai_body = timeout(
        Duration::from_millis(900),
        to_bytes(openai.into_body(), usize::MAX),
    )
    .await;

    assert!(
        openai_body.is_err(),
        "OpenAI compatible SSE should wait for an ephemeral close signal instead of polling durable state"
    );
}
