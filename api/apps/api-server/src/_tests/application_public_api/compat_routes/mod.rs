use crate::{
    _tests::support::{
        login_and_capture_cookie, test_api_state_with_database_url, test_app, test_config,
    },
    app_state::ApiState,
    host_infrastructure::LocalRuntimeEventStream,
    routes::application_public_api::tool_callback_ids::encode_openai_callback_tool_call_id,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use control_plane::{
    application_public_api::compat::openai::run_id_from_response_id,
    orchestration_runtime::debug_stream_events,
    ports::{
        CreateCallbackTaskInput, CreateCheckpointInput, CreateNodeRunInput,
        OrchestrationRuntimeRepository, RuntimeEventCloseReason, RuntimeEventEnvelope,
        RuntimeEventPayload, RuntimeEventStream, RuntimeEventStreamPolicy,
        RuntimeEventSubscription, RuntimeEventTrimPolicy, UpdateFlowRunInput,
    },
};
use serde_json::{Value, json};
use std::sync::Arc;
use time::OffsetDateTime;
use tokio::time::{Duration, timeout};
use tower::ServiceExt;

struct DropTerminalRuntimeEventStream {
    inner: LocalRuntimeEventStream,
}

struct NeverCloseDropTerminalRuntimeEventStream {
    inner: DropTerminalRuntimeEventStream,
}

impl DropTerminalRuntimeEventStream {
    fn new() -> Self {
        Self {
            inner: LocalRuntimeEventStream::new(),
        }
    }
}

impl NeverCloseDropTerminalRuntimeEventStream {
    fn new() -> Self {
        Self {
            inner: DropTerminalRuntimeEventStream::new(),
        }
    }
}

#[async_trait]
impl RuntimeEventStream for DropTerminalRuntimeEventStream {
    async fn open_run(
        &self,
        run_id: uuid::Uuid,
        policy: RuntimeEventStreamPolicy,
    ) -> anyhow::Result<()> {
        self.inner.open_run(run_id, policy).await
    }

    async fn append(
        &self,
        run_id: uuid::Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope> {
        if is_terminal_runtime_event(&event.event_type) {
            return Ok(RuntimeEventEnvelope::new(run_id, 0, event));
        }
        self.inner.append(run_id, event).await
    }

    async fn subscribe(
        &self,
        run_id: uuid::Uuid,
        from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        self.inner.subscribe(run_id, from_sequence).await
    }

    async fn replay(
        &self,
        run_id: uuid::Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>> {
        self.inner.replay(run_id, from_sequence, limit).await
    }

    async fn close_run(
        &self,
        run_id: uuid::Uuid,
        reason: RuntimeEventCloseReason,
    ) -> anyhow::Result<()> {
        self.inner.close_run(run_id, reason).await
    }

    async fn trim(&self, run_id: uuid::Uuid, policy: RuntimeEventTrimPolicy) -> anyhow::Result<()> {
        self.inner.trim(run_id, policy).await
    }
}

#[async_trait]
impl RuntimeEventStream for NeverCloseDropTerminalRuntimeEventStream {
    async fn open_run(
        &self,
        run_id: uuid::Uuid,
        policy: RuntimeEventStreamPolicy,
    ) -> anyhow::Result<()> {
        self.inner.open_run(run_id, policy).await
    }

    async fn append(
        &self,
        run_id: uuid::Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<RuntimeEventEnvelope> {
        self.inner.append(run_id, event).await
    }

    async fn subscribe(
        &self,
        run_id: uuid::Uuid,
        from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        self.inner.subscribe(run_id, from_sequence).await
    }

    async fn replay(
        &self,
        run_id: uuid::Uuid,
        from_sequence: Option<i64>,
        limit: usize,
    ) -> anyhow::Result<Vec<RuntimeEventEnvelope>> {
        self.inner.replay(run_id, from_sequence, limit).await
    }

    async fn close_run(
        &self,
        _run_id: uuid::Uuid,
        _reason: RuntimeEventCloseReason,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn trim(&self, run_id: uuid::Uuid, policy: RuntimeEventTrimPolicy) -> anyhow::Result<()> {
        self.inner.trim(run_id, policy).await
    }
}

fn is_terminal_runtime_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "flow_finished" | "flow_failed" | "flow_cancelled" | "waiting_human" | "waiting_callback"
    )
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_application(app: &Router, cookie: &str, csrf: &str, name: &str) -> String {
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
                        "name": name,
                        "description": "compatible public route test",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn create_application_key(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
) -> String {
    create_application_key_with_id(app, cookie, csrf, application_id)
        .await
        .0
}

async fn create_application_key_with_id(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
) -> (String, uuid::Uuid) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-keys"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Compatible route key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = response_json(response).await;
    let token = payload["data"]["token"].as_str().unwrap().to_string();
    let api_key_id = uuid::Uuid::parse_str(payload["data"]["id"].as_str().unwrap()).unwrap();
    (token, api_key_id)
}

async fn publish_application(app: &Router, cookie: &str, csrf: &str, application_id: &str) {
    let state = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
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
    let mut document = response_json(state).await["data"]["draft"]["document"].clone();
    let start_node = document["graph"]["nodes"]
        .as_array_mut()
        .expect("nodes array")
        .iter_mut()
        .find(|node| node["type"] == "start")
        .expect("default draft should include a start node");
    start_node["config"]["model_list"] = json!([
        {
            "id": "qwen3.6-35b-a3b",
            "name": "Qwen 3.6 35B",
            "context_window": 128000,
            "max_output_tokens": 32000,
            "auto_compact_token_limit": 110000,
            "capabilities": {
                "reasoning": true,
                "tool_call": true,
                "multimodal": false,
                "structured_output": true
            },
            "reasoning": {
                "default_effort": "medium",
                "supported_efforts": ["low", "medium", "high"]
            }
        },
        "deepseek-v4-flash"
    ]);

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
                        "document": document,
                        "change_kind": "logical",
                        "summary": "Configure compatible model list"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": {
                            "input": {
                                "query_target": "node-start.query",
                                "model_target": null,
                                "inputs_target": null,
                                "history_target": "node-start.history",
                                "attachments_target": null
                            },
                            "output": {
                                "answer_selector": "answer",
                                "usage_selector": null,
                                "files_selector": null,
                                "error_selector": null
                            }
                        },
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn setup_published_app(app: &Router, name: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    publish_application(app, &cookie, &csrf, &application_id).await;
    token
}

async fn setup_published_app_with_key_id(app: &Router, name: &str) -> (String, uuid::Uuid) {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let (token, api_key_id) =
        create_application_key_with_id(app, &cookie, &csrf, &application_id).await;
    publish_application(app, &cookie, &csrf, &application_id).await;
    (token, api_key_id)
}

async fn setup_unpublished_app_key(app: &Router, name: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    create_application_key(app, &cookie, &csrf, &application_id).await
}

async fn test_app_with_state() -> (Router, std::sync::Arc<crate::app_state::ApiState>) {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn test_app_with_runtime_event_stream(
    runtime_event_stream: Arc<dyn RuntimeEventStream>,
) -> (Router, Arc<ApiState>) {
    let (base_state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let state = Arc::new(ApiState {
        store: base_state.store.clone(),
        settings_feature_registry: base_state.settings_feature_registry.clone(),
        console_operation_registry: base_state.console_operation_registry.clone(),
        infrastructure: base_state.infrastructure.clone(),
        console_surface_registry: base_state.console_surface_registry.clone(),
        file_storage_registry: base_state.file_storage_registry.clone(),
        runtime_engine: base_state.runtime_engine.clone(),
        provider_runtime: base_state.provider_runtime.clone(),
        process_started_at: base_state.process_started_at,
        runtime_activity: base_state.runtime_activity.clone(),
        api_runtime_profile: base_state.api_runtime_profile.clone(),
        plugin_runner_system: base_state.plugin_runner_system.clone(),
        official_plugin_source: base_state.official_plugin_source.clone(),
        official_agent_flow_template_source: base_state.official_agent_flow_template_source.clone(),
        official_mcp_bundle_source: base_state.official_mcp_bundle_source.clone(),
        api_node_id: base_state.api_node_id.clone(),
        provider_install_root: base_state.provider_install_root.clone(),
        provider_secret_master_key: base_state.provider_secret_master_key.clone(),
        host_extension_dropin_root: base_state.host_extension_dropin_root.clone(),
        allow_unverified_filesystem_dropins: base_state.allow_unverified_filesystem_dropins,
        allow_uploaded_host_extensions: base_state.allow_uploaded_host_extensions,
        session_store: base_state.session_store.clone(),
        runtime_event_stream,
        api_docs: base_state.api_docs.clone(),
        cookie_name: base_state.cookie_name.clone(),
        cookie_secure: base_state.cookie_secure,
        session_ttl_days: base_state.session_ttl_days,
        bootstrap_workspace_name: base_state.bootstrap_workspace_name.clone(),
    });
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn seed_llm_callback_for_response_run(
    state: &crate::app_state::ApiState,
    flow_run_id: uuid::Uuid,
) -> domain::CallbackTaskRecord {
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id,
            status: domain::FlowRunStatus::WaitingCallback,
            output_payload: json!({
                "tool_calls": [
                    {
                        "id": "call_inventory",
                        "name": "lookup_inventory",
                        "arguments": { "sku": "sku_123" }
                    }
                ]
            }),
            error_payload: None,
            finished_at: None,
        })
        .await
        .unwrap();
    let node_run = state
        .store
        .create_node_run(&CreateNodeRunInput {
            flow_run_id,
            node_id: "node-llm".to_string(),
            node_type: "llm".to_string(),
            node_alias: "LLM".to_string(),
            status: domain::NodeRunStatus::WaitingCallback,
            input_payload: json!({}),
            debug_payload: json!({ "llm_rounds": [] }),
            started_at: OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();

    state
        .store
        .create_callback_task(&CreateCallbackTaskInput {
            flow_run_id,
            node_run_id: node_run.id,
            callback_kind: "llm_tool_calls".to_string(),
            request_payload: json!({
                "tool_calls": [
                    {
                        "id": "call_inventory",
                        "name": "lookup_inventory",
                        "arguments": { "sku": "sku_123" }
                    }
                ],
                "finish_reason": "tool_call"
            }),
            external_ref_payload: None,
        })
        .await
        .unwrap()
}

async fn seed_llm_callback_checkpoint_for_response_run(
    state: &crate::app_state::ApiState,
    callback_task: &domain::CallbackTaskRecord,
) {
    state
        .store
        .create_checkpoint(&CreateCheckpointInput {
            flow_run_id: callback_task.flow_run_id,
            node_run_id: Some(callback_task.node_run_id),
            status: "waiting_callback".to_string(),
            reason: "等待 callback 回填".to_string(),
            locator_payload: json!({
                "node_id": "node-llm",
                "next_node_index": 1
            }),
            variable_snapshot: json!({
                "node-start": {
                    "query": "Final question"
                },
                "node-llm": {
                    "__llm_tool_callback": {
                        "callback_kind": "llm_tool_calls",
                        "pending_tool_calls": callback_task.request_payload["tool_calls"],
                        "history": [
                            {
                                "role": "user",
                                "content": "Final question"
                            },
                            {
                                "role": "assistant",
                                "content": "",
                                "tool_calls": callback_task.request_payload["tool_calls"]
                            }
                        ]
                    }
                }
            }),
            external_ref_payload: Some(callback_task.request_payload.clone()),
        })
        .await
        .unwrap();
}

async fn application_api_key_last_used_at(
    state: &ApiState,
    api_key_id: uuid::Uuid,
) -> Option<OffsetDateTime> {
    sqlx::query_scalar::<_, Option<OffsetDateTime>>(
        "select last_used_at from api_keys where id = $1",
    )
    .bind(api_key_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap()
}

async fn post_json(
    app: &Router,
    uri: &str,
    token_header: (&str, String),
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(token_header.0, token_header.1)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_models(app: &Router, uri: &str, token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

fn openai_body(stream: bool) -> Value {
    json!({
        "model": "provider/custom-model:latest",
        "stream": stream,
        "messages": [
            {"role": "system", "content": "Use the support playbook."},
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": "Final question"}
        ],
        "metadata": {
            "trace_id": "trace-openai"
        }
    })
}

fn responses_body(stream: bool) -> Value {
    json!({
        "model": "provider/custom-model:latest",
        "stream": stream,
        "input": "Final question",
        "user": "external-user-123",
        "metadata": {
            "trace_id": "trace-responses"
        }
    })
}

fn anthropic_body(stream: bool) -> Value {
    json!({
        "model": "anthropic/custom-model:latest",
        "max_tokens": 512,
        "stream": stream,
        "system": "Use the support playbook.",
        "messages": [
            {"role": "user", "content": "Earlier question"},
            {"role": "assistant", "content": "Earlier answer"},
            {"role": "user", "content": [{"type": "text", "text": "Final question"}]}
        ],
        "metadata": {
            "expand_id": "external-user-123"
        }
    })
}

mod auth;
mod openai;
mod streaming;
