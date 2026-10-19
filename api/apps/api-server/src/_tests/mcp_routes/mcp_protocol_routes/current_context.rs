use super::*;
use crate::routes::mcp_protocol::client_context::{with_client_context, McpClientContext};
use control_plane::ports::{
    ApplicationPublicationRepository, CreateApplicationInput, SetApplicationApiEnabledInput,
};
use uuid::Uuid;

async fn ingest(
    store: &storage_durable_postgres::MainDurableStore,
    application_id: Uuid,
    scope: Uuid,
    source: &str,
    thread: &str,
    turn: &str,
) -> Uuid {
    control_plane::agent_logs::AgentLogsService::new(store.clone()).ingest(
        application_id, scope, Uuid::now_v7(), control_plane::ports::AgentLogsBatch {
            schema_version: control_plane::ports::AGENT_LOGS_SCHEMA_VERSION.into(),
            source_id: source.into(), source_client: "codex".into(),
            events: vec![serde_json::from_value(json!({
                "event_id":format!("{thread}-{turn}"),"source_session_id":thread,"source_task_id":turn,
                "sequence":1,"occurred_at":"2026-10-11T00:00:00Z","kind":"assistant",
                "phase":"final_answer","content":"current context fixture","raw":{}
            })).unwrap()],
        },
    ).await.unwrap().record_ids[0]
}

async fn application(
    store: &storage_durable_postgres::MainDurableStore,
    owner: Uuid,
    scope: Uuid,
    name: &str,
) -> Uuid {
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: owner,
            workspace_id: scope,
            application_type: domain::ApplicationType::AgentLogs,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: name.into(),
            description: "fixture".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    store
        .set_application_api_enabled(&SetApplicationApiEnabledInput {
            actor_user_id: owner,
            application_id: application.id,
            api_enabled: true,
        })
        .await
        .unwrap();
    application.id
}

fn meta(thread: &str, turn: &str) -> Value {
    json!({"threadId":thread,"sessionId":"process-session", "x-codex-turn-metadata":{
        "thread_id":thread,"session_id":"process-session","turn_id":turn
    }})
}

async fn current(app: &axum::Router, token: &str, meta: Value) -> Value {
    let payload = call_mcp(app, token, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
        "name":"mcp_call","_meta":meta,"arguments":{"tool_id":"current_run_context","arguments":{}}
    }})).await;
    assert_eq!(payload["result"]["isError"], false, "{payload}");
    payload["result"]["structuredContent"].clone()
}

#[tokio::test]
async fn mcp_current_context_resolves_exact_turn_list_get_call_and_failure_boundaries() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, cookie.parse().unwrap());
    let actor = require_session(&state, &headers).await.unwrap().actor;
    let id = application(
        &state.store,
        actor.user_id,
        actor.current_workspace_id,
        "context logs",
    )
    .await;
    let first = ingest(
        &state.store,
        id,
        actor.current_workspace_id,
        "source",
        "thread-a",
        "turn-a",
    )
    .await;
    let second = ingest(
        &state.store,
        id,
        actor.current_workspace_id,
        "source",
        "thread-a",
        "turn-b",
    )
    .await;
    let parallel = ingest(
        &state.store,
        id,
        actor.current_workspace_id,
        "source",
        "thread-b",
        "turn-a",
    )
    .await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    create_interface_tool_and_binding(
        &app,
        &cookie,
        &csrf,
        "current_run_context",
        "get_current_run_context",
        json!({"mappings":[]}),
    )
    .await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    let list = call_mcp(
        &app,
        &token,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
            "name":"mcp_list","arguments":{"path":"/runtime","depth":1}
        }}),
    )
    .await;
    assert!(
        list["result"]["structuredContent"]["result"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "current_run_context"),
        "{list}"
    );
    let get = call_mcp(
        &app,
        &token,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"mcp_get","arguments":{"tool_id":"current_run_context"}
        }}),
    )
    .await;
    assert_eq!(get["result"]["isError"], false, "{get}");
    for (thread, turn, expected) in [
        ("thread-a", "turn-a", first),
        ("thread-a", "turn-a", first),
        ("thread-a", "turn-b", second),
        ("thread-b", "turn-a", parallel),
    ] {
        let result = current(&app, &token, meta(thread, turn)).await;
        assert_eq!(result["status"], "resolved", "{result}");
        assert_eq!(result["record_id"], expected.to_string());
        assert_eq!(result["application_id"], id.to_string());
        assert_eq!(result["thread_id"], thread);
        assert_eq!(result["turn_id"], turn);
        assert_eq!(result["session_id"], "process-session");
    }
    // Seed only persisted native task metadata in this isolated schema. The
    // lookup does not execute a model or inspect native transcript bodies.
    let native_application = state
        .store
        .create_application(&CreateApplicationInput {
            actor_user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "native context fixture".into(),
            description: "metadata fixture".into(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    let editor = control_plane::ports::FlowRepository::get_or_create_editor_state(
        &state.store,
        actor.current_workspace_id,
        native_application.id,
        actor.user_id,
    )
    .await
    .unwrap();
    let compiled = control_plane::ports::OrchestrationRuntimeRepository::upsert_compiled_plan(
        &state.store,
        &control_plane::ports::UpsertCompiledPlanInput {
            actor_user_id: actor.user_id,
            flow_id: editor.flow.id,
            flow_draft_id: editor.draft.id,
            schema_version: editor.draft.schema_version.clone(),
            document_hash: "native-context-fixture".into(),
            document_updated_at: editor.draft.updated_at,
            plan: json!({"topological_order": ["node-start"]}),
        },
    )
    .await
    .unwrap();
    let native_key_id = Uuid::now_v7();
    control_plane::ports::ApiKeyRepository::create_api_key(
        &state.store,
        &control_plane::ports::CreateApiKeyInput {
            id: native_key_id,
            name: "native context fixture".into(),
            token_hash: format!("fixture-{}", native_key_id),
            token_prefix: "fixture".into(),
            key_kind: domain::ApiKeyKind::ApplicationApiKey,
            application_id: Some(native_application.id),
            role_code: None,
            creator_user_id: actor.user_id,
            tenant_id: actor.tenant_id,
            scope_kind: domain::DataModelScopeKind::Workspace,
            scope_id: actor.current_workspace_id,
            enabled: true,
            expires_at: None,
        },
    )
    .await
    .unwrap();
    let native_run =
        control_plane_contracts::application_public_runtime::ApplicationPublishedFlowRunRepository::create_published_flow_run(
            &state.store,
            &control_plane::ports::CreateFlowRunInput {
                application_run_log_context: Some(control_plane_contracts::ports::ApplicationRunLogContext {
                    identity_status: "identified".into(),
                    protocol: Some("openai_responses".into()),
                    thread_id: Some("native-thread".into()),
                    turn_id: Some("native-turn".into()),
                    call_kind: Some("generate".into()),
                    ..Default::default()
                }),
                actor_user_id: actor.user_id,
                application_id: native_application.id,
                flow_id: editor.flow.id,
                flow_draft_id: editor.draft.id,
                compiled_plan_id: compiled.id,
                debug_session_id: String::new(),
                flow_schema_version: compiled.schema_version.clone(),
                document_hash: compiled.document_hash.clone(),
                run_mode: domain::FlowRunMode::PublishedApiRun,
                target_node_id: None,
                title: "native identity fixture".into(),
                status: domain::FlowRunStatus::Running,
                input_payload: json!({"node-start":{"query":"native identity fixture"}}),
                started_at: time::OffsetDateTime::now_utc(),
                api_key_id: Some(native_key_id),
                publication_version_id: None,
                assistant_conversation_id: None,
                external_user: None,
                external_conversation_id: None,
                external_trace_id: None,
                compatibility_mode: None,
                idempotency_key: None,
            },
        )
        .await
        .unwrap();
    let native_id = native_run.flow_run.id;
    let native = current(&app, &token, meta("native-thread", "native-turn")).await;
    assert_eq!(native["status"], "resolved", "{native}");
    assert_eq!(native["record_id"], native_id.to_string());
    assert_eq!(native["native_run_id"], native_id.to_string());
    assert_eq!(native["application_id"], native_application.id.to_string());

    let overview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{id}/logs/records/{first}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(overview.status(), StatusCode::OK);
    assert_eq!(
        response_json(overview).await["data"]["record_id"],
        first.to_string()
    );
    for (metadata, status) in [
        (json!({}), "missing_identity"),
        (json!({"sessionId":"thread-a"}), "missing_identity"),
        (meta("thread-a", "absent"), "not_found"),
        (
            json!({"threadId":"thread-a","x-codex-turn-metadata":{"thread_id":"other","turn_id":"turn-a"}}),
            "conflicting_identity",
        ),
        (json!({"threadId":42,"turnId":"turn-a"}), "invalid_identity"),
    ] {
        let result = current(&app, &token, metadata).await;
        assert_eq!(result["status"], status, "{result}");
        assert!(result["record_id"].is_null());
    }
    let other_id = application(
        &state.store,
        actor.user_id,
        actor.current_workspace_id,
        "duplicate identity",
    )
    .await;
    ingest(
        &state.store,
        other_id,
        actor.current_workspace_id,
        "duplicate-source",
        "thread-a",
        "turn-a",
    )
    .await;
    let result = current(&app, &token, meta("thread-a", "turn-a")).await;
    assert_eq!(result["status"], "ambiguous");
    assert!(result["record_id"].is_null() && result["application_id"].is_null());
    let uri = "/api/console/applications/logs/current-context";
    let unauthenticated = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    let outside = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("cookie", &cookie)
                .header(
                    "x-codex-turn-metadata",
                    meta("thread-a", "turn-a").to_string(),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(outside.status(), StatusCode::OK);
    assert_eq!(
        response_json(outside).await["data"]["status"],
        "missing_identity",
        "HTTP headers cannot forge an MCP invocation scope"
    );
}

#[tokio::test]
async fn mcp_current_context_keeps_own_all_and_workspace_acl() {
    use crate::_tests::support::{create_role, replace_member_roles, seed_workspace};
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "context-viewer",
        "temp-pass",
    )
    .await;
    let member_id = Uuid::parse_str(&member).unwrap();
    create_role(&app, &root_cookie, &root_csrf, "context_viewer").await;
    async fn policy(app: &axum::Router, cookie: &str, csrf: &str, scope: &str) {
        let response = app.clone().oneshot(Request::builder().method("PUT").uri("/api/console/settings/roles/context_viewer/console-policy")
            .header("cookie",cookie).header("x-csrf-token",csrf).header("content-type","application/json")
            .body(Body::from(json!({"groups":[{"kind":"settings_feature","group_id":"system.applications","enabled":true,"strategy":"custom",
                "operations":[{"kind":"row","operation_id":"get_current_run_context","scope":scope}]}]}).to_string())).unwrap()).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert_eq!(
            status,
            StatusCode::NO_CONTENT,
            "policy {scope}: {}",
            String::from_utf8_lossy(&body)
        );
    }
    policy(&app, &root_cookie, &root_csrf, "own").await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member, &["context_viewer"]).await;
    let (cookie, _) = login_and_capture_cookie(&app, "context-viewer", "temp-pass").await;
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, root_cookie.parse().unwrap());
    let actor = require_session(&state, &headers).await.unwrap().actor;
    let foreign_scope = seed_workspace(&database_url, "context foreign scope").await;
    let owned = application(&state.store, member_id, actor.current_workspace_id, "owned").await;
    let hidden = application(
        &state.store,
        actor.user_id,
        actor.current_workspace_id,
        "hidden",
    )
    .await;
    let foreign = application(&state.store, member_id, foreign_scope, "foreign").await;
    let record = ingest(
        &state.store,
        owned,
        actor.current_workspace_id,
        "own",
        "thread-acl",
        "turn-acl",
    )
    .await;
    ingest(
        &state.store,
        hidden,
        actor.current_workspace_id,
        "hidden",
        "thread-acl",
        "turn-acl",
    )
    .await;
    ingest(
        &state.store,
        foreign,
        foreign_scope,
        "foreign",
        "thread-acl",
        "turn-acl",
    )
    .await;
    async fn read(app: &axum::Router, cookie: &str) -> Value {
        let response = with_client_context(
            McpClientContext {
                thread_id: Some("thread-acl".into()),
                turn_id: Some("turn-acl".into()),
                ..Default::default()
            },
            app.clone().oneshot(
                Request::builder()
                    .uri("/api/console/applications/logs/current-context")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response_json(response).await["data"].clone()
    }
    let result = read(&app, &cookie).await;
    assert_eq!(result["status"], "resolved", "{result}");
    assert_eq!(result["record_id"], record.to_string());
    policy(&app, &root_cookie, &root_csrf, "scope_all").await;
    assert_eq!(read(&app, &cookie).await["status"], "ambiguous");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/context_viewer/console-policy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"groups":[]}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = with_client_context(
        McpClientContext {
            thread_id: Some("thread-acl".into()),
            turn_id: Some("turn-acl".into()),
            ..Default::default()
        },
        app.clone().oneshot(
            Request::builder()
                .uri("/api/console/applications/logs/current-context")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
