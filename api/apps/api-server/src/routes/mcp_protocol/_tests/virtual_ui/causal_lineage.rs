use std::{collections::BTreeMap, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    http::{header::COOKIE, HeaderMap, HeaderValue, Request, StatusCode},
};
use orchestration_runtime::{
    compiled_plan::CompiledNode, execution_engine::RuntimeInternalToolInvoker,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::{
    _tests::support::{login_and_capture_cookie, test_api_state_with_database_url},
    middleware::require_session::require_session,
    routes::mcp_protocol::virtual_ui::ApiMcpRuntimeToolInvoker,
};

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_mcp_instance(app: &axum::Router, cookie: &str, csrf: &str) {
    create_named_mcp_instance(app, cookie, csrf, "taichuy").await;
}

async fn create_named_mcp_instance(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    instance_id: &str,
) {
    let response = app.clone().oneshot(Request::builder()
        .method("POST").uri("/api/console/mcp/instances")
        .header("cookie", cookie).header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(Body::from(json!({"instance_id":instance_id,"name":instance_id,"description_short":null,"status":"enabled","default_entry_path":"/"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "{}",
        response_json(response).await
    );
}

async fn create_interface_tool_and_binding(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    tool_id: &str,
    interface_id: &str,
    input_mapping: Value,
) {
    let create_tool_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/tools")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "tool_id": tool_id,
                        "des_id": format!("{tool_id}_description"),
                        "name": tool_id,
                        "short_description": format!("Execute {tool_id}."),
                        "full_description": "",
                        "execution_target": {
                            "kind": "interface_wrapper",
                            "interface_id": interface_id
                        },
                        "parameter_schema": {},
                        "result_schema": {},
                        "input_mapping": input_mapping,
                        "output_mapping": {},
                        "permission_code": null,
                        "risk_level": "low",
                        "status": "enabled"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = create_tool_response.status();
    let payload = response_json(create_tool_response).await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");

    let create_binding_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/taichuy/tool-bindings")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "group_path": "/runtime",
                        "tool_id": tool_id,
                        "display_alias": null,
                        "visible": true,
                        "sort_order": 0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_binding_response.status(), StatusCode::CREATED);
}

fn runtime_mcp_test_node() -> CompiledNode {
    CompiledNode {
        node_id: "node-llm".to_string(),
        node_type: "llm".to_string(),
        alias: "LLM".to_string(),
        container_id: None,
        dependency_node_ids: Vec::new(),
        downstream_node_ids: Vec::new(),
        bindings: BTreeMap::new(),
        outputs: Vec::new(),
        config: json!({}),
        plugin_runtime: None,
        llm_runtime: None,
        code_runtime: None,
    }
}

async fn runtime_mcp_invoker(
    state: std::sync::Arc<crate::app_state::ApiState>,
    cookie: &str,
) -> ApiMcpRuntimeToolInvoker {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, HeaderValue::from_str(cookie).unwrap());
    let context = require_session(&state, &headers).await.unwrap();
    ApiMcpRuntimeToolInvoker::new(
        crate::runtime_internal_tool_invoker_factory(&state, &context.actor)
            .await
            .unwrap(),
        headers,
        context.actor,
        vec!["taichuy".to_string()],
    )
    .await
    .unwrap()
}

async fn invoke_runtime_create_model(
    invoker: &ApiMcpRuntimeToolInvoker,
    model_code: &str,
) -> orchestration_runtime::execution_engine::RuntimeInternalToolOutput {
    let node = runtime_mcp_test_node();
    let registration = invoker
        .registrations_for_node(&node)
        .into_iter()
        .find(|registration| registration.provider_name == "taichuy_mcp_call")
        .expect("runtime registration should expose the selected MCP call tool");

    invoker
        .invoke_runtime_internal_tool(
            &node,
            &registration,
            json!({
                "tool_id": "create_model_probe",
                "max_inline_chars": 12000,
                "arguments": {
                    "body": {
                        "code": model_code,
                        "title": "Delegated model",
                        "scope_kind": "workspace",
                        "template_provider": "core",
                        "template_code": "general",
                        "template_version": "v1"
                    }
                }
            }),
        )
        .await
        .unwrap()
}

async fn create_model_probe_tool(app: &axum::Router, cookie: &str, csrf: &str) {
    create_interface_tool_and_binding(
        app,
        cookie,
        csrf,
        "create_model_probe",
        "create_model",
        json!({
            "mappings": [
                {
                    "interface_param": "code",
                    "mcp_param": "body.code",
                    "required": true
                },
                {
                    "interface_param": "title",
                    "mcp_param": "body.title",
                    "required": true
                },
                {
                    "interface_param": "scope_kind",
                    "mcp_param": "body.scope_kind",
                    "required": true
                },
                {
                    "interface_param": "template_provider",
                    "mcp_param": "body.template_provider",
                    "required": true
                },
                {
                    "interface_param": "template_code",
                    "mcp_param": "body.template_code",
                    "required": true
                },
                {
                    "interface_param": "template_version",
                    "mcp_param": "body.template_version",
                    "required": true
                }
            ]
        }),
    )
    .await;
}

#[tokio::test]
async fn system_event_center_mcp_spawn_preserves_causal_lineage() {
    use control_plane_contracts::ports::{
        LifecycleFactObserver, LifecycleObservationContext, LifecycleOutboxRecord,
        RecordLifecycleFactInput,
    };
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;
    #[derive(Default)]
    struct CommittedModelFacts(Mutex<Vec<LifecycleObservationContext>>);
    impl LifecycleFactObserver for CommittedModelFacts {
        fn published(&self, input: &RecordLifecycleFactInput) {
            if let Some(context) = &input.observation_context {
                if context.producer_id == "model_definition.create" {
                    self.0.lock().unwrap().push(context.clone());
                }
            }
        }
        fn consumed(&self, _: &LifecycleOutboxRecord, _: &str) {}
    }
    let (base, _) = test_api_state_with_database_url().await;
    let facts = Arc::new(CommittedModelFacts::default());
    // The compiled Console port intentionally owns the boot store without an ancestry field.
    // Only the later runtime invoker store owns the trusted workflow context.
    let state = Arc::new(crate::app_state::ApiState {
        store: base.store.clone().with_lifecycle_observer(facts.clone()),
        ..(*base).clone()
    });
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    create_model_probe_tool(&app, &cookie, &csrf).await;
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, HeaderValue::from_str(&cookie).unwrap());
    let workspace = require_session(&state, &headers)
        .await
        .unwrap()
        .actor
        .current_workspace_id;
    let expected = vec![Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    let causal = Arc::new(crate::app_state::ApiState {
        store: state
            .store
            .clone()
            .with_workflow_ancestor_ids(expected.clone()),
        ..(*state).clone()
    });
    let invoker = runtime_mcp_invoker(causal, &cookie).await;
    let output = control_plane_contracts::ports::with_lifecycle_workflow_ancestors(
        vec![Uuid::now_v7()],
        async move {
            tokio::spawn(async move {
                assert!(
                    control_plane_contracts::ports::lifecycle_workflow_ancestor_ids().is_empty(),
                    "spawn does not inherit tasklocal context"
                );
                invoke_runtime_create_model(&invoker, "system_event_mcp_causal_native").await
            })
            .await
            .unwrap()
        },
    )
    .await;
    assert!(!output.is_error, "{}", output.content);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM model_definitions WHERE scope_id=$1 AND code=$2")
            .bind(workspace)
            .bind("system_event_mcp_causal_native")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(count, 1, "the actual Console model transaction must commit");
    let recorded = facts.0.lock().unwrap();
    assert!(
        recorded.iter().any(|context| context.scope
            == domain::event_center::EventSourceScope::Workspace {
                workspace_id: workspace
            }
            && context.workflow_ancestor_ids == expected),
        "actual native postcommit metadata lost the explicit workflow chain: {recorded:?}"
    );
    drop(recorded);

    // A later invocation from the boot store must not retain the prior invocation's chain.
    // It still uses the same real Console dispatch and spawned transaction owner.
    facts.0.lock().unwrap().clear();
    assert!(control_plane_contracts::ports::lifecycle_workflow_ancestor_ids().is_empty());
    let independent_invoker = runtime_mcp_invoker(state.clone(), &cookie).await;
    let output =
        invoke_runtime_create_model(&independent_invoker, "system_event_mcp_without_ancestors")
            .await;
    assert!(!output.is_error, "{}", output.content);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM model_definitions WHERE scope_id=$1 AND code=$2")
            .bind(workspace)
            .bind("system_event_mcp_without_ancestors")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(
        count, 1,
        "the independent Console model transaction must commit"
    );
    let recorded = facts.0.lock().unwrap();
    assert_eq!(
        recorded.len(),
        1,
        "one independent model-create fact must commit"
    );
    assert_eq!(
        recorded[0].scope,
        domain::event_center::EventSourceScope::Workspace {
            workspace_id: workspace
        }
    );
    assert!(
        recorded[0].workflow_ancestor_ids.is_empty(),
        "independent invocation inherited another workflow's chain: {recorded:?}"
    );
    assert!(control_plane_contracts::ports::lifecycle_workflow_ancestor_ids().is_empty());
}
