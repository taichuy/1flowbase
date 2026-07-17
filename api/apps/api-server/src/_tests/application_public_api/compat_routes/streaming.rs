use super::*;

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
async fn openai_chat_streaming_tool_resume_returns_done_on_current_connection() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Streaming Tool Resume App").await;

    let created = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created_payload = response_json(created).await;
    let run_id = created_payload["id"]
        .as_str()
        .and_then(|id| id.strip_prefix("chatcmpl-"))
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .expect("chat completion id should include run id");
    let callback_task = seed_llm_callback_for_response_run(state.as_ref(), run_id).await;
    let tool_call_id = encode_openai_callback_tool_call_id(callback_task.id, "call_inventory");
    state
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_started(run_id))
        .await
        .unwrap();
    state
        .runtime_event_stream
        .append(
            run_id,
            debug_stream_events::waiting_callback_with_task(
                run_id,
                callback_task.node_run_id,
                "node-llm",
                &callback_task,
            ),
        )
        .await
        .unwrap();

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "stream": true,
            "messages": [
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

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let body = timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("OpenAI streaming tool resume SSE should finish on current connection")
    .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(body.contains("[DONE]"), "{body}");
    assert!(
        !body.contains("\"finish_reason\":\"tool_calls\""),
        "resume stream replayed a stale waiting_callback instead of the resumed turn: {body}"
    );
    assert!(
        !body.contains("lookup_inventory"),
        "resume stream sent the stale tool call again: {body}"
    );
    assert!(
        body.contains("runtime_error") || body.contains("\"finish_reason\":\"stop\""),
        "{body}"
    );
}

#[tokio::test]
async fn openai_chat_streaming_tool_resume_escapes_nul_tool_output_before_persisting_callback() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Streaming NUL Tool Resume App").await;

    let created = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        openai_body(false),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created_payload = response_json(created).await;
    let run_id = created_payload["id"]
        .as_str()
        .and_then(|id| id.strip_prefix("chatcmpl-"))
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .expect("chat completion id should include run id");
    let callback_task = seed_llm_callback_for_response_run(state.as_ref(), run_id).await;
    seed_llm_callback_checkpoint_for_response_run(state.as_ref(), &callback_task).await;
    let tool_call_id = encode_openai_callback_tool_call_id(callback_task.id, "call_inventory");
    state
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_started(run_id))
        .await
        .unwrap();
    state
        .runtime_event_stream
        .append(
            run_id,
            debug_stream_events::waiting_callback_with_task(
                run_id,
                callback_task.node_run_id,
                "node-llm",
                &callback_task,
            ),
        )
        .await
        .unwrap();

    let response = post_json(
        &app,
        "/v1/chat/completions",
        ("authorization", format!("Bearer {token}")),
        json!({
            "model": "provider/custom-model:latest",
            "stream": true,
            "messages": [
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

    assert_eq!(response.status(), StatusCode::OK);
    let body = timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("OpenAI streaming tool resume with NUL output should finish")
    .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("[DONE]"), "{body}");

    let stored_task = state
        .store
        .get_callback_task(callback_task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_task.status, domain::CallbackTaskStatus::Completed);
    assert_eq!(
        stored_task.response_payload.unwrap()["tool_results"][0]["content"],
        json!("STDERR:\n\\u0000after")
    );
}

#[tokio::test]
async fn openai_responses_streaming_tool_resume_returns_current_turn_terminal_event() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_app(&app, "OpenAI Responses Streaming Tool Resume App").await;

    let first = post_json(
        &app,
        "/v1/responses",
        ("authorization", format!("Bearer {token}")),
        responses_body(false),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_payload = response_json(first).await;
    let previous_response_id = first_payload["id"].as_str().unwrap().to_string();
    let run_id = run_id_from_response_id(&previous_response_id).unwrap();
    let callback_task = seed_llm_callback_for_response_run(state.as_ref(), run_id).await;
    let call_id = encode_openai_callback_tool_call_id(callback_task.id, "call_inventory");
    state
        .runtime_event_stream
        .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
        .await
        .unwrap();
    state
        .runtime_event_stream
        .append(run_id, debug_stream_events::flow_started(run_id))
        .await
        .unwrap();
    state
        .runtime_event_stream
        .append(
            run_id,
            debug_stream_events::waiting_callback_with_task(
                run_id,
                callback_task.node_run_id,
                "node-llm",
                &callback_task,
            ),
        )
        .await
        .unwrap();

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

    assert_eq!(response.status(), StatusCode::OK);
    let body = timeout(
        Duration::from_secs(5),
        to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("OpenAI Responses streaming tool resume SSE should finish on current connection")
    .unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        !body.contains("lookup_inventory"),
        "resume stream sent the stale function call again: {body}"
    );
    assert!(
        body.contains("response.completed") || body.contains("response.failed"),
        "{body}"
    );
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
