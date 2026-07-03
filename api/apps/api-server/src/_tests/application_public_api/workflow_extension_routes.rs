use std::sync::Arc;

use crate::{
    _tests::support::{
        login_and_capture_cookie, test_api_state_with_database_url, test_app, test_config,
    },
    app_state::ApiState,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use tower::ServiceExt;
use uuid::Uuid;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn test_app_with_state() -> (Router, Arc<ApiState>) {
    let (state, _) = test_api_state_with_database_url().await;
    let config = test_config();
    let app = crate::app_with_state_and_config(state.clone(), &config);
    (app, state)
}

async fn wait_for_flow_run_status(
    state: &ApiState,
    flow_run_id: Uuid,
    expected_status: &str,
) -> Value {
    let mut last_status = String::new();
    let mut last_output = json!({});
    for _ in 0..40 {
        let Some((status, output_payload)) = sqlx::query_as::<_, (String, Value)>(
            "select status::text, output_payload from flow_runs where id = $1",
        )
        .bind(flow_run_id)
        .fetch_optional(state.store.pool())
        .await
        .unwrap() else {
            sleep(Duration::from_millis(25)).await;
            continue;
        };
        last_status = status;
        last_output = output_payload;
        if last_status == expected_status {
            return last_output;
        }
        sleep(Duration::from_millis(25)).await;
    }

    panic!("expected flow run {flow_run_id} status {expected_status}, last status {last_status}");
}

async fn create_workflow_application(app: &Router, cookie: &str, csrf: &str, name: &str) -> String {
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
                        "application_type": "workflow",
                        "name": name,
                        "description": "workflow extension route test",
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

async fn save_workflow_document(app: &Router, cookie: &str, csrf: &str, application_id: &str) {
    save_workflow_document_with_builder(app, cookie, csrf, application_id, workflow_document).await;
}

async fn save_workflow_document_with_builder(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    build_document: fn(&str) -> Value,
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
    let flow_id = response_json(state).await["data"]["draft"]["document"]["meta"]["flowId"]
        .as_str()
        .unwrap()
        .to_string();

    let response = app
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
                        "document": build_document(&flow_id),
                        "change_kind": "logical",
                        "summary": "Configure workflow extension test"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
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
                        "name": "Workflow extension route key",
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

async fn publish_workflow_extension_with_enabled(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    options: WorkflowExtensionPublishOptions,
) -> Value {
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
                                "history_target": null,
                                "attachments_target": null
                            },
                            "output": {
                                "answer_selector": null,
                                "usage_selector": null,
                                "files_selector": null,
                                "error_selector": null
                            },
                            "extension": {
                                "slug": options.slug,
                                "method": "POST",
                                "response_mode": options.response_mode,
                                "parameters": options.parameters
                            }
                        },
                        "api_enabled": options.api_enabled
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await
}

struct WorkflowExtensionPublishOptions {
    slug: String,
    response_mode: String,
    parameters: Value,
    api_enabled: bool,
}

impl WorkflowExtensionPublishOptions {
    fn new(slug: &str, response_mode: &str, parameters: Value, api_enabled: bool) -> Self {
        Self {
            slug: slug.to_string(),
            response_mode: response_mode.to_string(),
            parameters,
            api_enabled,
        }
    }
}

async fn publish_workflow_extension(
    app: &Router,
    cookie: &str,
    csrf: &str,
    application_id: &str,
    slug: &str,
    response_mode: &str,
    parameters: Value,
) -> Value {
    publish_workflow_extension_with_enabled(
        app,
        cookie,
        csrf,
        application_id,
        WorkflowExtensionPublishOptions::new(slug, response_mode, parameters, true),
    )
    .await
}

fn workflow_document(flow_id: &str) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id, "name": "Ticket Workflow", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "alias": "Workflow Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "input_fields": [
                            { "key": "customer_id", "label": "Customer ID", "inputType": "text", "valueType": "string", "required": true },
                            { "key": "priority", "label": "Priority", "inputType": "text", "valueType": "string", "required": false },
                            { "key": "ticket_kind", "label": "Ticket Kind", "inputType": "text", "valueType": "string", "required": false }
                        ],
                        "sync_timeout_ms": 30000
                    },
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-transform",
                    "type": "template_transform",
                    "alias": "Template Transform",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "template": {
                            "kind": "templated_text",
                            "value": "ticket-{{ node-workflow-start.customer_id }}"
                        }
                    },
                    "outputs": [{ "key": "ticket_id", "title": "Ticket ID", "valueType": "string" }]
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "alias": "Workflow End",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "ticket_id": { "kind": "selector", "value": ["node-transform", "ticket_id"] }
                    },
                    "outputs": [{ "key": "ticket_id", "title": "Ticket ID", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-transform", "source": "node-workflow-start", "target": "node-transform", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-transform-end", "source": "node-transform", "target": "node-workflow-end", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

fn workflow_waiting_document(flow_id: &str) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id, "name": "Waiting Ticket Workflow", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-workflow-start",
                    "type": "workflow_start",
                    "alias": "Workflow Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {
                        "input_fields": [
                            { "key": "customer_id", "label": "Customer ID", "inputType": "text", "valueType": "string", "required": true }
                        ],
                        "sync_timeout_ms": 30000
                    },
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-human",
                    "type": "human_input",
                    "alias": "Human Input",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "prompt": { "kind": "templated_text", "value": "Review {{ node-workflow-start.customer_id }}" }
                    },
                    "outputs": [{ "key": "input", "title": "Human Input", "valueType": "string" }]
                },
                {
                    "id": "node-workflow-end",
                    "type": "workflow_end",
                    "alias": "Workflow End",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 480, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {
                        "ticket_id": { "kind": "selector", "value": ["node-human", "input"] }
                    },
                    "outputs": [{ "key": "ticket_id", "title": "Ticket ID", "valueType": "string" }]
                }
            ],
            "edges": [
                { "id": "edge-start-human", "source": "node-workflow-start", "target": "node-human", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] },
                { "id": "edge-human-end", "source": "node-human", "target": "node-workflow-end", "sourceHandle": null, "targetHandle": null, "containerId": null, "points": [] }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

async fn setup_workflow_extension_app(
    app: &Router,
    slug: &str,
    response_mode: &str,
    parameters: Value,
) -> (String, Value) {
    setup_workflow_extension_app_with_enabled(app, slug, response_mode, parameters, true).await
}

async fn setup_workflow_extension_app_with_enabled(
    app: &Router,
    slug: &str,
    response_mode: &str,
    parameters: Value,
    api_enabled: bool,
) -> (String, Value) {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_workflow_application(app, &cookie, &csrf, slug).await;
    save_workflow_document(app, &cookie, &csrf, &application_id).await;
    let token = create_application_key(app, &cookie, &csrf, &application_id).await;
    let publication = publish_workflow_extension_with_enabled(
        app,
        &cookie,
        &csrf,
        &application_id,
        WorkflowExtensionPublishOptions::new(slug, response_mode, parameters, api_enabled),
    )
    .await;
    (token, publication)
}

#[tokio::test]
async fn workflow_extension_sync_route_returns_workflow_end_object_without_wrapper() {
    let app = test_app().await;
    let (token, publication) = setup_workflow_extension_app(
        &app,
        "open-ticket-sync",
        "sync",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
    )
    .await;

    assert_eq!(
        publication["data"]["public_url"],
        json!("/api/ex/open-ticket-sync")
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/open-ticket-sync?customer_id=C-42")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload, json!({ "ticket_id": "ticket-C-42" }));
}

#[tokio::test]
async fn workflow_extension_sync_route_returns_accepted_when_run_waits_for_human_input() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_workflow_application(&app, &cookie, &csrf, "open-ticket-waiting").await;
    save_workflow_document_with_builder(
        &app,
        &cookie,
        &csrf,
        &application_id,
        workflow_waiting_document,
    )
    .await;
    let token = create_application_key(&app, &cookie, &csrf, &application_id).await;
    publish_workflow_extension(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "open-ticket-waiting",
        "sync",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/open-ticket-waiting?customer_id=C-42")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = response_json(response).await;
    assert!(payload["run_id"]
        .as_str()
        .is_some_and(|run_id| uuid::Uuid::parse_str(run_id).is_ok()));
    assert_eq!(payload["status"], json!("waiting_human"));
}

#[tokio::test]
async fn workflow_extension_async_route_returns_accepted_run_status() {
    let (app, state) = test_app_with_state().await;
    let (token, _) = setup_workflow_extension_app(
        &app,
        "open-ticket-async",
        "async",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/open-ticket-async?customer_id=C-42")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let payload = response_json(response).await;
    assert!(payload["run_id"]
        .as_str()
        .is_some_and(|run_id| uuid::Uuid::parse_str(run_id).is_ok()));
    assert_eq!(payload["status"], json!("queued"));
    let run_id = Uuid::parse_str(payload["run_id"].as_str().unwrap()).unwrap();
    let output_payload = wait_for_flow_run_status(state.as_ref(), run_id, "succeeded").await;
    assert_eq!(output_payload, json!({ "ticket_id": "ticket-C-42" }));
}

#[tokio::test]
async fn workflow_extension_route_returns_stable_errors_for_missing_slug_and_method_mismatch() {
    let app = test_app().await;
    let (token, _) = setup_workflow_extension_app(
        &app,
        "open-ticket-errors",
        "sync",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
    )
    .await;

    let missing_slug = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/missing-ticket?customer_id=C-42")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_slug.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response_json(missing_slug).await["code"],
        json!("workflow_extension_not_found")
    );

    let method_mismatch = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ex/open-ticket-errors?customer_id=C-42")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(method_mismatch.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        response_json(method_mismatch).await["code"],
        json!("method_not_allowed")
    );
}

#[tokio::test]
async fn workflow_extension_route_rejects_cross_application_api_key() {
    let app = test_app().await;
    let (first_token, _) = setup_workflow_extension_app(
        &app,
        "open-ticket-owner-a",
        "async",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
    )
    .await;
    setup_workflow_extension_app(
        &app,
        "open-ticket-owner-b",
        "async",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/open-ticket-owner-b?customer_id=C-42")
                .header("authorization", format!("Bearer {first_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["code"],
        json!("workflow_extension_forbidden")
    );
}

#[tokio::test]
async fn workflow_extension_route_rejects_disabled_publication() {
    let app = test_app().await;
    let (token, _) = setup_workflow_extension_app_with_enabled(
        &app,
        "open-ticket-disabled",
        "async",
        json!([
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            }
        ]),
        false,
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/open-ticket-disabled?customer_id=C-42")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["code"],
        json!("application_not_published")
    );
}

#[tokio::test]
async fn workflow_extension_openapi_registers_concrete_slug_operation() {
    let app = test_app().await;
    setup_workflow_extension_app(
        &app,
        "open-ticket-docs",
        "sync",
        json!([
            {
                "name": "slug",
                "source": "path",
                "target": "node-workflow-start.slug"
            },
            {
                "name": "customer_id",
                "source": "query",
                "target": "node-workflow-start.customer_id"
            },
            {
                "name": "priority",
                "source": "form",
                "target": "node-workflow-start.priority"
            },
            {
                "name": "ticket_kind",
                "source": "body",
                "target": "node-workflow-start.ticket_kind"
            }
        ]),
    )
    .await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    let operation = &payload["paths"]["/api/ex/open-ticket-docs"]["post"];
    assert_eq!(operation["parameters"][0]["in"], json!("path"));
    assert_eq!(operation["parameters"][1]["in"], json!("query"));
    assert_eq!(
        operation["requestBody"]["content"]["application/x-www-form-urlencoded"]["schema"]
            ["properties"]["priority"]["type"],
        json!("string")
    );
    assert_eq!(
        operation["requestBody"]["content"]["application/json"]["schema"]["properties"]
            ["ticket_kind"]["type"],
        json!("string")
    );
    assert_eq!(
        operation["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
            ["ticket_id"]["type"],
        json!("string")
    );
}
