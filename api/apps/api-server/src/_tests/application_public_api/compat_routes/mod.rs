use crate::{
    _tests::{
        create_gated_provider_instance, create_marker_output_provider_instance,
        create_ready_provider_instance,
        support::{
            login_and_capture_cookie, test_api_state_with_database_url, test_app, test_config,
        },
        ProviderInvocationGate, PROVIDER_MARKER_LIKE_OUTPUT,
    },
    app_state::ApiState,
    host_infrastructure::LocalRuntimeEventStream,
    routes::application_public_api::tool_callback_ids::encode_openai_callback_tool_call_id,
};
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::ports::{
    AppendTerminalIfMissingAndCloseOutcome, RuntimeEventCloseReason, RuntimeEventEnvelope,
    RuntimeEventPayload, RuntimeEventStream, RuntimeEventStreamPolicy, RuntimeEventSubscription,
    RuntimeEventTrimPolicy,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use time::OffsetDateTime;
use tokio::time::{timeout, Duration};
use tower::ServiceExt;

const COMPAT_ROUTE_PROVIDER_MODEL: &str = "fixture_chat";

pub(super) struct DropTerminalRuntimeEventStream {
    inner: LocalRuntimeEventStream,
}

pub(super) struct NeverCloseDropTerminalRuntimeEventStream {
    inner: DropTerminalRuntimeEventStream,
}

pub(super) struct SubscribeBeforeAppendRuntimeEventStream {
    inner: LocalRuntimeEventStream,
    subscribed: AtomicBool,
    append_observed: AtomicBool,
    append_before_subscribe: AtomicBool,
}

impl SubscribeBeforeAppendRuntimeEventStream {
    pub(super) fn new() -> Self {
        Self {
            inner: LocalRuntimeEventStream::new(),
            subscribed: AtomicBool::new(false),
            append_observed: AtomicBool::new(false),
            append_before_subscribe: AtomicBool::new(false),
        }
    }

    pub(super) fn append_observed(&self) -> bool {
        self.append_observed.load(Ordering::SeqCst)
    }

    pub(super) fn append_before_subscribe(&self) -> bool {
        self.append_before_subscribe.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RuntimeEventStream for SubscribeBeforeAppendRuntimeEventStream {
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
        self.append_observed.store(true, Ordering::SeqCst);
        if !self.subscribed.load(Ordering::SeqCst) {
            self.append_before_subscribe.store(true, Ordering::SeqCst);
        }
        self.inner.append(run_id, event).await
    }

    async fn append_terminal_if_missing_and_close(
        &self,
        run_id: uuid::Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        self.append_observed.store(true, Ordering::SeqCst);
        if !self.subscribed.load(Ordering::SeqCst) {
            self.append_before_subscribe.store(true, Ordering::SeqCst);
        }
        self.inner
            .append_terminal_if_missing_and_close(run_id, event)
            .await
    }

    async fn subscribe(
        &self,
        run_id: uuid::Uuid,
        from_sequence: Option<i64>,
    ) -> anyhow::Result<RuntimeEventSubscription> {
        let subscription = self.inner.subscribe(run_id, from_sequence).await?;
        self.subscribed.store(true, Ordering::SeqCst);
        Ok(subscription)
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

impl DropTerminalRuntimeEventStream {
    pub(super) fn new() -> Self {
        Self {
            inner: LocalRuntimeEventStream::new(),
        }
    }
}

impl NeverCloseDropTerminalRuntimeEventStream {
    pub(super) fn new() -> Self {
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

    async fn append_terminal_if_missing_and_close(
        &self,
        run_id: uuid::Uuid,
        event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        // This fault-injection wrapper only drops legacy terminal `append` calls. Recovery uses
        // the stream's atomic primitive so the test double does not turn recovery into a lie.
        self.inner
            .append_terminal_if_missing_and_close(run_id, event)
            .await
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

    async fn append_terminal_if_missing_and_close(
        &self,
        _run_id: uuid::Uuid,
        _event: RuntimeEventPayload,
    ) -> anyhow::Result<AppendTerminalIfMissingAndCloseOutcome> {
        // Durable persistence has already chosen the terminal. This fault injection accepts that
        // projection but deliberately drops its ephemeral append and close, so timeout probes
        // observe an actually open producer instead of a durable-terminal fallback.
        Ok(AppendTerminalIfMissingAndCloseOutcome::Appended)
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
        "flow_finished"
            | "flow_incomplete"
            | "flow_failed"
            | "flow_cancelled"
            | "waiting_human"
            | "waiting_callback"
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
    let provider_instance_id = create_ready_provider_instance(app, cookie, csrf).await;
    publish_application_with_provider(app, cookie, csrf, application_id, &provider_instance_id)
        .await;
}

async fn publish_application_with_provider(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    provider_instance_id: &str,
) {
    publish_application_with_provider_and_mapping(
        app,
        cookie,
        csrf,
        application_id,
        provider_instance_id,
        json!([
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
        ]),
        None,
    )
    .await;
}

async fn publish_unbound_application_with_provider(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    provider_instance_id: &str,
) {
    publish_application_with_provider_and_mapping(
        app,
        cookie,
        csrf,
        application_id,
        provider_instance_id,
        json!([COMPAT_ROUTE_PROVIDER_MODEL]),
        Some(json!({
            "generate": null,
            "count_tokens": null,
            "compact": {
                "responses_compact": null,
                "responses_compaction_v2": null
            }
        })),
    )
    .await;
}

async fn publish_application_with_provider_and_mapping(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    provider_instance_id: &str,
    advertised_models: Value,
    operation_bindings: Option<Value>,
) {
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
    let nodes = document["graph"]["nodes"]
        .as_array_mut()
        .expect("nodes array");
    {
        let start_node = nodes
            .iter_mut()
            .find(|node| node["type"] == "start")
            .expect("default draft should include a start node");
        start_node["config"]["model_list"] = advertised_models;
    }
    let llm_node = nodes
        .iter_mut()
        .find(|node| node["type"] == "llm")
        .expect("default draft should include an LLM node");
    llm_node["config"]["model_provider"] = json!({
        "provider_code": "fixture_provider",
        "source_instance_id": provider_instance_id,
        "model_id": COMPAT_ROUTE_PROVIDER_MODEL
    });

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
                        "summary": "Configure compatible model list and provider route"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::OK);

    let mut mapping = json!({
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
    });
    if let Some(operation_bindings) = operation_bindings {
        mapping["operation_bindings"] = operation_bindings;
    }

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
                        "mapping": mapping,
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

async fn setup_published_app_with_provider_gate(
    app: &Router,
    name: &str,
) -> (String, ProviderInvocationGate) {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    let (provider_instance_id, gate) = create_gated_provider_instance(app, &cookie, &csrf).await;
    publish_application_with_provider(app, &cookie, &csrf, &application_id, &provider_instance_id)
        .await;
    (token, gate)
}

async fn setup_published_app_with_marker_output_provider(app: &Router, name: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    let provider_instance_id = create_marker_output_provider_instance(app, &cookie, &csrf).await;
    publish_application_with_provider(app, &cookie, &csrf, &application_id, &provider_instance_id)
        .await;
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

async fn setup_unbound_published_app_key(app: &Router, name: &str) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    let provider_instance_id = create_ready_provider_instance(app, &cookie, &csrf).await;
    publish_unbound_application_with_provider(
        app,
        &cookie,
        &csrf,
        &application_id,
        &provider_instance_id,
    )
    .await;
    token
}

async fn test_app_with_state() -> (Router, std::sync::Arc<crate::app_state::ApiState>) {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn flow_run_count(state: &ApiState) -> i64 {
    sqlx::query_scalar("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap()
}

async fn assert_published_compat_plan_has_provider_route(state: &ApiState) {
    let plan: Value = sqlx::query_scalar(
        "select plan from flow_compiled_plans order by created_at desc, id desc limit 1",
    )
    .fetch_one(state.store.pool())
    .await
    .unwrap();

    assert_eq!(plan["compile_issues"], json!([]), "{plan}");
    let runtime = &plan["nodes"]["node-llm"]["llm_runtime"];
    assert_eq!(
        runtime["provider_code"],
        json!("fixture_provider"),
        "{plan}"
    );
    assert_eq!(
        runtime["model"],
        json!(COMPAT_ROUTE_PROVIDER_MODEL),
        "{plan}"
    );
    assert!(
        runtime["provider_instance_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "{plan}"
    );
}

pub(super) async fn test_app_with_runtime_event_stream(
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
        "model": "qwen3.6-35b-a3b",
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
mod compact;
mod openai;
mod streaming;
