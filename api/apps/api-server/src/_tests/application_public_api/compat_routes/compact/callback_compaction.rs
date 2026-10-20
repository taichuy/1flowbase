use super::*;

#[test]
fn root_1998_f9_invalid_key_precedes_unsupported_compaction_classification() {
    run_compat_route_test(|| async {
        let app = test_app().await;
        let response = post_openai_responses(
            &app,
            "/v1/responses",
            "not-an-application-api-key",
            v2_compaction_body(true),
            Some(codex_turn_metadata("unsupported_fixture_profile")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let payload = response_json(response).await;
        assert_eq!(payload["error"]["code"], "not_authenticated");
        assert!(!payload
            .to_string()
            .contains("unsupported_compaction_profile"));
    });
}

#[test]
fn root_1998_f9_stale_callback_with_compact_trigger_falls_through_to_unary_compact() {
    run_compat_route_test(|| async {
        let (app, state) = test_app_with_state().await;
        let token = setup_compact_published_app(
            &app,
            "Stale Callback Compact App",
            CompactFixtureMode::Success,
        )
        .await;
        let before = flow_run_count(state.as_ref()).await;
        let call_id = encode_openai_callback_tool_call_id(Uuid::now_v7(), "call_inventory");
        let response = post_openai_responses(
            &app,
            "/v1/responses",
            &token,
            mixed_callback_compact_body(&call_id),
            None,
        )
        .await;
        let status = response.status();
        if status != StatusCode::OK {
            let payload = response_json(response).await;
            panic!("unexpected compact response status {status}: {payload}");
        }
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        let completed = sse_json_event(&body, "response.completed");
        assert_eq!(completed["response"]["id"], "resp-v2-canary");
        assert_eq!(completed["response"]["output"].as_array().unwrap().len(), 1);
        assert_eq!(body.matches(V2_OPAQUE_CANARY).count(), 1, "{body}");
        assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
    });
}

#[test]
fn root_1998_f9_live_callback_with_compact_trigger_preserves_stream_resume_priority() {
    run_compat_route_test(|| async {
        let (app, state) = test_app_with_state().await;
        let token = setup_compact_published_app(
            &app,
            "Live Callback Compact Priority App",
            CompactFixtureMode::LiveCallback,
        )
        .await;
        let mut initial = responses_body(false);
        initial["tools"] = json!([{
            "type": "function", "name": "lookup_inventory", "strict": true,
            "parameters": {
                "type": "object",
                "properties": {"sku": {"type": "string"}},
                "required": ["sku"],
                "additionalProperties": false
            }
        }]);
        let response =
            post_openai_responses(&app, "/v1/responses", &token, initial.clone(), None).await;
        let status = response.status();
        let created = response_json(response).await;
        assert_eq!(status, StatusCode::OK, "{created}");
        let call = created["output"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "function_call")
            .expect("provider must create a live callback");
        let call_id = call["call_id"].as_str().unwrap();
        assert_eq!(call_id, "call_inventory");
        let actor = control_plane::application_public_api::api_keys::ApplicationApiKeyService::new(
            state.store.clone(),
        )
        .authenticate_bearer_token(&token)
        .await
        .expect("published API key should resolve its actor");
        let callbacks = state
            .store
            .find_native_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                created["id"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            callbacks.len(),
            1,
            "response should identify one live callback"
        );
        let callback_task_id = callbacks[0].id;
        assert_eq!(callbacks[0].status, domain::CallbackTaskStatus::Pending);
        let before = flow_run_count(state.as_ref()).await;
        let response = post_openai_responses(
            &app,
            "/v1/responses",
            &token,
            live_callback_compact_resume_body(initial, call_id, created["id"].as_str().unwrap()),
            None,
        )
        .await;
        let status = response.status();
        if status != StatusCode::OK {
            let payload = response_json(response).await;
            panic!("unexpected live callback response status {status}: {payload}");
        }
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        let completed = sse_json_event(&body, "response.completed");
        assert_ne!(
            completed["response"]["id"], created["id"],
            "resume must create a new response round"
        );
        assert_eq!(
            completed["response"]["previous_response_id"], created["id"],
            "resume response must link to the pending callback round"
        );
        assert!(body.contains("callback resume completed"), "{body}");
        assert!(
            !body.contains(V2_OPAQUE_CANARY),
            "live callback must take priority over Compact"
        );
        assert_eq!(flow_run_count(state.as_ref()).await, before);
        assert_eq!(
            state
                .store
                .get_published_callback_task(callback_task_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            domain::CallbackTaskStatus::Completed
        );
    });
}

#[test]
fn local_summary_with_pending_native_callback_starts_new_run_and_supersedes_predecessor() {
    run_compat_route_test(|| async {
        let (app, state) = test_app_with_state().await;
        let token = setup_compact_published_app(
            &app,
            "Local Summary Pending Callback App",
            CompactFixtureMode::LiveCallback,
        )
        .await;
        let thread_id = format!("thread-{}", Uuid::now_v7());
        let turn_id = format!("turn-{}", Uuid::now_v7());
        let normal_metadata = json!({
            "thread_id": thread_id, "turn_id": turn_id, "request_kind": "turn"
        });
        let mut initial = responses_body(false);
        initial["tools"] = json!([{
            "type": "function", "name": "lookup_inventory", "strict": true,
            "parameters": {
                "type": "object", "properties": {"sku": {"type": "string"}},
                "required": ["sku"], "additionalProperties": false
            }
        }]);
        initial["client_metadata"] = json!({
            "thread_id": thread_id,
            "turn_id": turn_id,
            "x-codex-turn-metadata": normal_metadata.to_string()
        });
        let initial_response = post_openai_responses(
            &app,
            "/v1/responses",
            &token,
            initial.clone(),
            Some(normal_metadata),
        )
        .await;
        let initial_status = initial_response.status();
        let created = response_json(initial_response).await;
        assert_eq!(initial_status, StatusCode::OK, "{created}");
        let call = created["output"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["type"] == "function_call")
            .expect("first response should wait for a tool callback");
        let call_id = call["call_id"].as_str().unwrap();
        let actor = control_plane::application_public_api::api_keys::ApplicationApiKeyService::new(
            state.store.clone(),
        )
        .authenticate_bearer_token(&token)
        .await
        .unwrap();
        let callbacks = state
            .store
            .find_native_responses_callbacks_by_response_id(
                actor.workspace_id,
                actor.application_id,
                actor.api_key_id,
                actor.creator_user_id,
                created["id"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(callbacks.len(), 1);
        let predecessor = &callbacks[0];
        assert_eq!(predecessor.status, domain::CallbackTaskStatus::Pending);
        let before = flow_run_count(state.as_ref()).await;

        let summary_metadata = json!({
            "thread_id": thread_id, "turn_id": turn_id, "request_kind": "compaction",
            "compaction": {"implementation": "responses"}
        });
        let mut summary = initial;
        summary["stream"] = json!(true);
        summary["previous_response_id"] = created["id"].clone();
        summary["input"] = json!([
            {"type": "message", "role": "user", "content": LOCAL_SUMMARY_PREFIX},
            {"type": "function_call", "call_id": call_id, "name": "lookup_inventory", "arguments": "{\"sku\":\"sku_123\"}"},
            {"type": "function_call_output", "call_id": call_id, "output": {"stock": 7}}
        ]);
        summary["client_metadata"] = json!({
            "thread_id": thread_id,
            "turn_id": turn_id,
            "x-codex-turn-metadata": summary_metadata.to_string()
        });
        let response = post_openai_responses(
            &app,
            "/v1/responses",
            &token,
            summary,
            Some(summary_metadata),
        )
        .await;
        let status = response.status();
        if status != StatusCode::OK {
            panic!(
                "local summary rejected with {status}: {}",
                response_json(response).await
            );
        }
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains("event: response.completed"), "{body}");
        assert!(body.contains("local summary raw input preserved"), "{body}");
        assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
        let callback = state
            .store
            .get_published_callback_task(predecessor.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(callback.status, domain::CallbackTaskStatus::Cancelled);
        let (status, reason): (String, Option<String>) =
            sqlx::query_as("select status,error_payload->>'reason' from flow_runs where id=$1")
                .bind(predecessor.flow_run_id)
                .fetch_one(state.store.pool())
                .await
                .unwrap();
        assert_eq!(status, "cancelled");
        assert_eq!(reason.as_deref(), Some("local_summary_superseded"));
        let resume_attempts: i64 = sqlx::query_scalar(
            "select count(*) from flow_run_callback_resume_attempts where callback_task_id=$1",
        )
        .bind(predecessor.id)
        .fetch_one(state.store.pool())
        .await
        .unwrap();
        assert_eq!(resume_attempts, 0);
        let request_kind: Option<String> = sqlx::query_scalar(
            "select log_context->>'request_kind' from flow_runs where id<>$1 order by created_at desc,id desc limit 1",
        ).bind(predecessor.flow_run_id).fetch_one(state.store.pool()).await.unwrap();
        assert_eq!(request_kind.as_deref(), Some("compaction"));
    });
}

#[test]
fn k3_local_summary_keeps_generate_sse_and_does_not_persist_codex_turn_header() {
    run_compat_route_test(|| async {
        let (app, state) = test_app_with_state().await;
        let token = setup_published_app(&app, "OpenAI Local Summary Generate App").await;
        let before = flow_run_count(state.as_ref()).await;
        let metadata = json!({
            "request_kind": "compaction",
            "compaction": {"implementation": "responses"},
            "private_turn_marker": PRIVATE_TURN_MARKER
        });

        let response = post_openai_responses(
            &app,
            "/v1/responses",
            &token,
            responses_body(true),
            Some(metadata),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("local summary SSE body should be readable")
                .to_vec(),
        )
        .expect("local summary SSE body should be UTF-8");
        assert!(body.contains("event: response.completed"), "{body}");
        assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
        let input_payload: Value = sqlx::query_scalar(
            "select input_payload from flow_runs order by created_at desc, id desc limit 1",
        )
        .fetch_one(state.store.pool())
        .await
        .expect("local summary flow run should retain its normal input payload");
        assert!(
            !input_payload.to_string().contains(PRIVATE_TURN_MARKER),
            "captured Codex turn metadata must not enter the persisted Generate input"
        );
    });
}

#[test]
fn k3_compact_provider_failure_is_an_error_without_completed_projection() {
    run_compat_route_test(|| async {
        let (app, state) = test_app_with_state().await;
        let token = setup_compact_published_app(
            &app,
            "OpenAI Compact Provider Failure App",
            CompactFixtureMode::ProviderFailure,
        )
        .await;
        let before = flow_run_count(state.as_ref()).await;

        let response = post_openai_responses(
            &app,
            "/v1/responses",
            &token,
            v2_compaction_body(true),
            Some(codex_turn_metadata("responses_compaction_v2")),
        )
        .await;

        let status = response.status();
        let payload = response_json(response).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY, "{payload}");
        assert_eq!(payload["error"]["code"], json!("provider_upstream_error"));
        assert!(payload.get("response").is_none());
        assert!(!payload.to_string().contains("response.completed"));
        assert_eq!(flow_run_count(state.as_ref()).await, before + 1);
    });
}
