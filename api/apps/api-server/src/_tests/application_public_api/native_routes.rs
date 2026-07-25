use crate::_tests::{
    create_ready_provider_instance,
    support::{login_and_capture_cookie, test_api_state_with_database_url, test_app, test_config},
};
use crate::routes::application_public_api::native::{
    parse_native_run_request, service_error, to_native_run_response,
};
use axum::{
    body::{to_bytes, Body, Bytes},
    http::{Request, StatusCode},
    Router,
};
use control_plane::application_public_api::native::{NativeRunResult, NativeRunStatus};
use control_plane::application_public_api::protocol_translation::{
    TranslationDecisionKind, TranslationProtocol, TranslationSafeRepresentation,
};
use control_plane::ports::{
    ApplicationCompiledPlanRepository, ApplicationPublicationRepository, CreateCallbackTaskInput,
    CreateNodeRunInput, OrchestrationRuntimeRepository, UpdateFlowRunInput,
};
use orchestration_runtime::execution_state::{CountTokensReceipt, NativeOperationTerminal};
use plugin_framework::provider_contract::{
    ProviderCountTokensResult, ProviderInvocationCapability, ProviderWireOperation,
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use tower::ServiceExt;
use uuid::Uuid;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn native_blocking_response_exposes_typed_operation_terminal() {
    let terminal = NativeOperationTerminal::CountTokens(
        CountTokensReceipt::new(ProviderCountTokensResult {
            operation: ProviderWireOperation::CountTokens,
            input_tokens: 29,
        })
        .expect("fixture CountTokens receipt"),
    );
    let response = to_native_run_response(NativeRunResult {
        id: Uuid::nil(),
        application_id: Uuid::nil(),
        api_key_id: Uuid::nil(),
        publication_version_id: Uuid::nil(),
        status: NativeRunStatus::Succeeded,
        node_input_payload: json!({}),
        metadata: json!({}),
        answer: None,
        answer_segments: None,
        required_action: None,
        tool_calls: None,
        usage: None,
        error: None,
        operation_terminal: Some(terminal),
        created_at: OffsetDateTime::UNIX_EPOCH,
    });
    assert_eq!(
        response.operation_terminal,
        Some(json!({
            "semantic_terminal": "count_tokens",
            "result": { "operation": "count_tokens", "input_tokens": 29 }
        }))
    );
}

#[test]
fn ac_007_callback_payload_conflict_maps_to_http_409() {
    let error = service_error(
        control_plane::errors::ControlPlaneError::Conflict("callback_resume_payload_conflict")
            .into(),
    );

    assert_eq!(error.status, StatusCode::CONFLICT);
    assert_eq!(error.code, "callback_resume_payload_conflict");
}

#[test]
fn d2_ac_001_native_malformed_json_has_one_safe_adapter_receipt() {
    let sentinel = "D2-NATIVE-MALFORMED-JSON-MUST-NOT-REACH-RECEIPT";
    let error = parse_native_run_request(Bytes::from(format!("{{\"raw\":\"{sentinel}\"")))
        .expect_err("malformed Native JSON must be rejected by the adapter boundary");

    assert_eq!(error.report.protocol, TranslationProtocol::Native);
    assert_eq!(error.report.decisions.len(), 1);
    let decision = &error.report.decisions[0];
    assert_eq!(decision.source_path, "$.body");
    assert_eq!(decision.kind, TranslationDecisionKind::Rejected);
    assert_eq!(
        decision.effective_value,
        TranslationSafeRepresentation::Present
    );
    assert!(
        !serde_json::to_string(&error.report)
            .expect("receipt should serialize")
            .contains(sentinel),
        "malformed JSON must not be retained in the receipt"
    );
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
                        "description": "native public route test",
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
                        "name": "Native route key",
                        "expires_at": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["token"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn publish_native_application(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    mapping: Value,
) {
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
    let payload = response_json(response).await;
    assert!(payload["data"].get("operation_bindings").is_none());
}

fn mapping_with_runnable_generate_target() -> Value {
    json!({
        "input": {
            "query_target": "node-start.query",
            "model_target": null,
            "inputs_target": "node-start",
            "history_target": "node-start.history",
            "attachments_target": "node-start.files"
        },
        "output": {
            "answer_selector": null,
            "usage_selector": null,
            "files_selector": null,
            "error_selector": null
        }
    })
}

fn native_run_body(model: Value) -> Value {
    json!({
        "query": "Summarize the incident",
        "model": model,
        "inputs": {
            "priority": "high"
        },
        "history": [
            {
                "role": "user",
                "content": "The customer cannot log in."
            }
        ],
        "attachments": [
            {
                "source": "upload_file_id",
                "value": "file-1",
                "name": "screenshot.png"
            }
        ],
        "conversation": {
            "id": "conversation-1"
        },
        "response_mode": "blocking",
        "stream_options": {
            "include_usage": true
        },
        "execution": {
            "timeout_seconds": 30
        },
        "metadata": {
            "trace_id": "trace-native-route-1"
        }
    })
}

async fn post_native_run(app: &Router, token: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/v1/runs")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

pub(super) async fn configure_runnable_native_generate_target(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
) {
    let provider_instance_id = create_ready_provider_instance(app, cookie, csrf).await;
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
    let mut document = response_json(state).await["data"]["draft"]["document"].clone();
    let llm_node = document["graph"]["nodes"]
        .as_array_mut()
        .expect("default draft should include graph nodes")
        .iter_mut()
        .find(|node| node["type"] == "llm")
        .expect("default draft should include an LLM node");
    assert_eq!(llm_node["id"], json!("node-llm"));
    llm_node["config"]["model_provider"] = json!({
        "provider_code": "fixture_provider",
        "source_instance_id": provider_instance_id,
        "model_id": "fixture_chat"
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
                        "summary": "configure native Generate provider route"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::OK);
}

/// Root #1453: publication freezes the workflow; it does not create a second Provider route.
pub(super) async fn assert_published_native_generate_route(
    state: &crate::app_state::ApiState,
    application_id: &str,
) {
    let application_id = Uuid::parse_str(application_id).expect("application id should be a UUID");
    let publication = state
        .store
        .load_active_application_publication(application_id)
        .await
        .unwrap()
        .expect("fixture should publish an active application version");
    let compiled_plan = state
        .store
        .get_application_compiled_plan(publication.compiled_plan_id)
        .await
        .unwrap()
        .expect("fixture publication should freeze a compiled plan");
    let plan: orchestration_runtime::compiled_plan::CompiledPlan =
        serde_json::from_value(compiled_plan.plan).expect("compiled plan should be valid");
    let runtime = plan.nodes["node-llm"]
        .llm_runtime
        .as_ref()
        .expect("workflow LLM node should retain its runtime");
    assert_eq!(runtime.provider_code, "fixture_provider");
    assert_eq!(runtime.model, "fixture_chat");
}

async fn setup_published_native_app(
    app: &Router,
    state: &crate::app_state::ApiState,
    name: &str,
) -> String {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, name).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    configure_runnable_native_generate_target(app, &cookie, &csrf, &application_id).await;
    publish_native_application(
        app,
        &cookie,
        &csrf,
        &application_id,
        mapping_with_runnable_generate_target(),
    )
    .await;
    assert_published_native_generate_route(state, &application_id).await;
    token
}

#[tokio::test]
async fn native_legacy_v1_agent_route_is_not_mounted() {
    let app = test_app().await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agent/runs")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

async fn test_app_with_state() -> (Router, std::sync::Arc<crate::app_state::ApiState>) {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn seed_pending_llm_callback(
    state: &crate::app_state::ApiState,
    flow_run_id: Uuid,
) -> domain::CallbackTaskRecord {
    state
        .store
        .update_flow_run(&UpdateFlowRunInput {
            flow_run_id,
            status: domain::FlowRunStatus::WaitingCallback,
            output_payload: json!({
                "tool_calls": [
                    {
                        "id": "call_weather",
                        "name": "lookup_weather",
                        "arguments": { "city": "Shanghai" }
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
                        "id": "call_weather",
                        "name": "lookup_weather",
                        "arguments": { "city": "Shanghai" }
                    }
                ],
                "finish_reason": "tool_call"
            }),
            external_ref_payload: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn native_run_route_accepts_any_string_model_and_preserves_metadata_without_node_input_model()
{
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(&app, state.as_ref(), "Native Route Model App").await;

    let response = post_native_run(
        &app,
        &token,
        native_run_body(json!("provider/model:any-public-string")),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = response_json(response).await;
    assert_eq!(
        payload["data"]["metadata"]["model"],
        json!("provider/model:any-public-string")
    );
    assert_eq!(
        payload["data"]["node_input_payload"]["node-start"]["query"],
        json!("Summarize the incident")
    );
    assert_eq!(
        payload["data"]["node_input_payload"]["node-start"]["priority"],
        json!("high")
    );
    assert!(payload["data"]["node_input_payload"]["node-start"]
        .get("model")
        .is_none());
}

#[tokio::test]
async fn native_get_run_exposes_pending_llm_required_action() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Required Action Route App").await;
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["response_mode"] = json!("manual");

    let created = post_native_run(&app, &token, body).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_payload = response_json(created).await;
    let run_id = Uuid::parse_str(created_payload["data"]["id"].as_str().unwrap()).unwrap();
    let callback_task = seed_pending_llm_callback(state.as_ref(), run_id).await;

    let response = app
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

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    assert_eq!(payload["data"]["status"], json!("waiting"));
    assert_eq!(
        payload["data"]["required_action"]["action_type"],
        json!("submit_tool_outputs")
    );
    assert_eq!(
        payload["data"]["required_action"]["payload"]["callback_task_id"],
        json!(callback_task.id.to_string())
    );
    assert_eq!(
        payload["data"]["required_action"]["payload"]["callback_kind"],
        json!("llm_tool_calls")
    );
    assert_eq!(
        payload["data"]["tool_calls"][0]["id"],
        json!("call_weather")
    );
}

#[tokio::test]
async fn native_resume_rejects_missing_llm_tool_result_without_consuming_task() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(
        &app,
        state.as_ref(),
        "Native Resume Missing Tool Result App",
    )
    .await;
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["response_mode"] = json!("manual");

    let created = post_native_run(&app, &token, body).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_payload = response_json(created).await;
    let run_id = Uuid::parse_str(created_payload["data"]["id"].as_str().unwrap()).unwrap();
    let callback_task = seed_pending_llm_callback(state.as_ref(), run_id).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/agent/v1/runs/{run_id}/resume"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "callback_task_id": callback_task.id,
                        "response_payload": {
                            "tool_results": []
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("tool_results"));
    let stored_task = state
        .store
        .get_callback_task(callback_task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_task.status, domain::CallbackTaskStatus::Pending);
}

#[tokio::test]
async fn native_tool_resume_consumes_in_request_and_records_failure_timeline() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Streaming Tool Resume App").await;
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["response_mode"] = json!("manual");

    let created = post_native_run(&app, &token, body).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_payload = response_json(created).await;
    let application_id =
        Uuid::parse_str(created_payload["data"]["application_id"].as_str().unwrap()).unwrap();
    let run_id = Uuid::parse_str(created_payload["data"]["id"].as_str().unwrap()).unwrap();
    let callback_task = seed_pending_llm_callback(state.as_ref(), run_id).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/agent/v1/runs/{run_id}/resume"))
                .header("authorization", format!("Bearer {token}"))
                .header("accept", "text/event-stream")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "callback_task_id": callback_task.id,
                        "response_mode": "streaming",
                        "response_payload": {
                            "tool_results": [
                                {
                                    "tool_call_id": "call_weather",
                                    "content": "{\"temperature\":21}"
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("internal_error"));

    let stored_task = state
        .store
        .get_callback_task(callback_task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_task.status, domain::CallbackTaskStatus::Pending);

    let flow_run = state
        .store
        .get_flow_run(application_id, run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(flow_run.status, domain::FlowRunStatus::Failed);

    let detail = state
        .store
        .get_application_run_detail(application_id, run_id)
        .await
        .unwrap()
        .unwrap();
    let event_types = detail
        .events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(
        event_types.contains(&"public_run_resume_requested")
            && event_types.contains(&"public_run_resume_failed"),
        "resume timeline should expose request and failure: {event_types:?}"
    );
    assert!(!event_types.contains(&"public_run_resume_claimed"));
}

#[tokio::test]
async fn native_run_route_accepts_expand_id_and_returns_default_title_metadata() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Route Expand Id App").await;
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["expand_id"] = json!("external-user-123");

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = response_json(response).await;
    assert_eq!(
        payload["data"]["metadata"]["expand_id"],
        json!("external-user-123")
    );
    assert!(payload["data"]["metadata"].get("user_id").is_none());
    assert_eq!(
        payload["data"]["metadata"]["external_user"],
        json!("external-user-123")
    );
    assert_eq!(
        payload["data"]["metadata"]["title"],
        json!("Summarize the incident")
    );
}

#[tokio::test]
async fn native_run_route_rejects_legacy_user_id_field() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Route Legacy User Id App").await;
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["user_id"] = json!("external-user-123");

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("user_id"));
}

#[tokio::test]
async fn d2_ac_007_native_run_route_rejects_compatibility_mode_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(
        &app,
        state.as_ref(),
        "Native Route Compatibility Mode Rejection App",
    )
    .await;
    let before = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["compatibility_mode"] = json!("native-v1");

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("compatibility_mode"));
    let after = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn d2_f1_native_run_route_rejects_execution_compatibility_mode_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(
        &app,
        state.as_ref(),
        "Native Route Execution Compatibility Mode Rejection App",
    )
    .await;
    let before = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["execution"]["compatibility_mode"] = json!("native-v1");

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("compatibility_mode"));
    let after = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn d2_f1_native_run_route_rejects_unknown_metadata_before_fingerprint_or_response_echo() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(
        &app,
        state.as_ref(),
        "Native Route Typed Metadata Rejection App",
    )
    .await;
    let before = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let sentinel = "D2-F1-NATIVE-ROUTE-METADATA-SECRET";
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["metadata"] = json!({
        "trace_id": "trace-native-route-1",
        sentinel: "must-not-reach-fingerprint-or-response"
    });

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("metadata"));
    assert!(
        !serde_json::to_string(&payload)
            .expect("error response serializes")
            .contains(sentinel),
        "unknown metadata must not echo in the initial response"
    );
    let after = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn d2_ac_007_native_run_persists_no_compatibility_mode() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Route Canonical Contract App")
            .await;
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["response_mode"] = json!("manual");

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let payload = response_json(response).await;
    let run_id = Uuid::parse_str(payload["data"]["id"].as_str().unwrap()).unwrap();
    let mode = sqlx::query_scalar::<_, Option<String>>(
        "select compatibility_mode from flow_runs where id = $1",
    )
    .bind(run_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(mode, None);
}

#[tokio::test]
async fn d1_ac_009_native_run_route_rejects_unknown_fields_before_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Route Unknown Field App").await;
    let before = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["unrecognized_native_option"] = json!(true);

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("body"));
    assert!(payload["message"]
        .as_str()
        .is_some_and(|message| message.contains("unknown Native request field")));
    let after = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn d2_ac_001_native_model_parameter_unknown_rejects_before_conversation_or_run_creation() {
    let (app, state) = test_app_with_state().await;
    let token = setup_published_native_app(
        &app,
        state.as_ref(),
        "Native Route Model Parameter Rejection App",
    )
    .await;
    let application_conversations_before =
        sqlx::query_scalar::<_, i64>("select count(*) from application_conversations")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    let public_conversations_before =
        sqlx::query_scalar::<_, i64>("select count(*) from application_public_conversations")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    let flow_runs_before = sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let node_runs_before = sqlx::query_scalar::<_, i64>("select count(*) from node_runs")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let mut body = native_run_body(json!("provider/model:any-public-string"));
    body["conversation"]["user"] = json!("model-parameter-user");
    body["execution"]["model_parameters"] = json!({"context_window": 128000});

    let response = post_native_run(&app, &token, body).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("invalid_model_parameters"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from application_conversations")
            .fetch_one(state.store.pool())
            .await
            .unwrap(),
        application_conversations_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from application_public_conversations")
            .fetch_one(state.store.pool())
            .await
            .unwrap(),
        public_conversations_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from flow_runs")
            .fetch_one(state.store.pool())
            .await
            .unwrap(),
        flow_runs_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from node_runs")
            .fetch_one(state.store.pool())
            .await
            .unwrap(),
        node_runs_before
    );
}

#[tokio::test]
async fn native_run_route_rejects_non_string_model_json_values() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Route Invalid Model App").await;

    for invalid_model in [
        json!(null),
        json!(42),
        json!(true),
        json!({ "name": "gpt" }),
        json!(["gpt"]),
    ] {
        let response = post_native_run(&app, &token, native_run_body(invalid_model)).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], json!("model"));
    }
}

#[tokio::test]
async fn native_run_route_validates_public_native_request_fields() {
    let (app, state) = test_app_with_state().await;
    let token =
        setup_published_native_app(&app, state.as_ref(), "Native Route Validation App").await;

    for (field, invalid_value) in [
        ("query", json!(false)),
        ("inputs", json!("not-object")),
        ("history", json!({ "role": "user" })),
        ("attachments", json!({ "id": "file-1" })),
        ("conversation", json!("not-object")),
        ("expand_id", json!({ "id": "external-user-123" })),
        ("response_mode", json!(["blocking"])),
        ("stream_options", json!("not-object")),
        ("execution", json!("not-object")),
        ("metadata", json!("not-object")),
        ("title", json!(["Quarterly support escalation"])),
    ] {
        let mut body = native_run_body(json!("any-model"));
        body[field] = invalid_value;

        let response = post_native_run(&app, &token, body).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = response_json(response).await;
        assert_eq!(payload["code"], json!(field));
    }
}

#[tokio::test]
async fn native_run_route_returns_application_not_published_for_unpublished_key_application() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_application(&app, &cookie, &csrf, "Unpublished Native Route App").await;
    let token = create_application_key(&app, &cookie, &csrf, &application_id).await;

    let response = post_native_run(&app, &token, native_run_body(json!("any-model"))).await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("application_not_published"));
}

#[tokio::test]
async fn native_run_route_forbids_reading_run_created_by_another_application_api_key() {
    let (app, state) = test_app_with_state().await;
    let first_token =
        setup_published_native_app(&app, state.as_ref(), "First Native Route App").await;
    let second_token =
        setup_published_native_app(&app, state.as_ref(), "Second Native Route App").await;
    let created = post_native_run(&app, &first_token, native_run_body(json!("any-model"))).await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_payload = response_json(created).await;
    let run_id = created_payload["data"]["id"].as_str().unwrap();

    let forbidden = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/agent/v1/runs/{run_id}"))
                .header("authorization", format!("Bearer {second_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let payload = response_json(forbidden).await;
    assert_eq!(payload["code"], json!("application_run_forbidden"));
}
