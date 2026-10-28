//! Assembled host boundary: receive -> published Agent-flow -> pause/resume -> progressive MCP -> send.
use super::*;
use crate::_tests::{application, support};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::ports::{OrchestrationRuntimeRepository, RoleRepository};
use tower::ServiceExt;

pub(super) async fn api(
    app: &Router,
    cookie: &str,
    csrf: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie)
        .header("content-type", "application/json");
    if !csrf.is_empty() {
        request = request.header("x-csrf-token", csrf);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"body":String::from_utf8_lossy(&bytes)})),
    )
}
async fn operator_run_access(f: &Fixture, role_code: &str, allowed_route: Option<&str>) {
    let operations = allowed_route
        .map(|path| {
            let access = f
                .state
                .console_operation_registry
                .access_for_console_route("POST", path)
                .unwrap();
            domain::ConsoleOperationPolicy::simple(
                domain::ConsoleOperationId::try_from(access.operation_id).unwrap(),
                true,
            )
        })
        .into_iter()
        .collect();
    f.state
        .store
        .replace_role_console_policy(&control_plane::ports::ReplaceRoleConsolePolicyInput {
            actor_user_id: f.actor.user_id,
            workspace_id: f.actor.current_workspace_id,
            role_code: role_code.into(),
            groups: vec![domain::RoleConsoleGroupPolicy::custom(
                domain::ConsolePolicyGroup::settings_feature("system.applications").unwrap(),
                operations,
            )],
        })
        .await
        .unwrap();
}

pub(super) async fn execution_diagnostics(
    f: &Fixture,
    application_id: &str,
    run_id: Uuid,
) -> Value {
    let detail = f
        .state
        .store
        .get_application_run_detail(Uuid::parse_str(application_id).unwrap(), run_id)
        .await
        .unwrap()
        .unwrap();
    json!({
        "run_status": detail.flow_run.status,
        "run_error": detail.flow_run.error_payload,
        "nodes": detail.node_runs.iter().map(|node| json!({
            "node_id":node.node_id,"status":node.status,"error":node.error_payload
        })).collect::<Vec<_>>()
    })
}

async fn pipeline(callback: bool) {
    let fixture = Fixture::new().await;
    fixture.receive().await;
    let foreign_conversation: Uuid = sqlx::query_scalar("SELECT id FROM communication_conversations WHERE instance_id=$1 AND external_identity='bob'")
        .bind(fixture.channel).fetch_one(fixture.state.store.pool()).await.unwrap();
    let app = fixture.app.clone();
    let (cookie, csrf) = support::login_and_capture_cookie(&app, "root", "change-me").await;
    let (status,grant) = api(&app,&cookie,&csrf,"POST",&format!("/api/console/settings/billing/credits/{}/grant",fixture.actor.user_id),json!({"amount":"100","reason":"communication pipeline fixture","source_type":"test","source_id":"communication-pipeline","idempotency_key":"communication-pipeline"})).await;
    assert_eq!(status, StatusCode::OK, "{grant}");
    let provider = application::create_communication_provider_instance(
        &app,
        &cookie,
        &csrf,
        callback,
        foreign_conversation,
    )
    .await;
    let application_id = if callback {
        application::seed_agent_flow_application(&app, &cookie, &csrf, &provider).await
    } else {
        application::seed_human_input_application(&app, &cookie, &csrf, &provider).await
    };
    if !callback {
        let path = format!("/api/console/applications/{application_id}/orchestration");
        let (status, orchestration) = api(&app, &cookie, "", "GET", &path, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{orchestration}");
        let mut document = orchestration["data"]["draft"]["document"].clone();
        for (edge, (source, target)) in document["graph"]["edges"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip([
                ("node-start", "node-human"),
                ("node-human", "node-llm"),
                ("node-llm", "node-answer"),
            ])
        {
            edge["source"] = json!(source);
            edge["target"] = json!(target);
        }
        for node in document["graph"]["nodes"].as_array_mut().unwrap() {
            if node["id"] == "node-human" {
                node["bindings"]["prompt"] =
                    json!({"kind":"templated_text","value":"Approve {{node-start.query}}"});
            }
            if node["id"] == "node-answer" {
                node["bindings"]["answer_template"] =
                    json!({"kind":"selector","value":["node-llm","text"]});
            }
        }
        let (status,saved) = api(&app,&cookie,&csrf,"PUT",&format!("{path}/draft"),json!({"document":document,"change_kind":"logical","summary":"Pause before scoped tools"})).await;
        assert_eq!(status, StatusCode::OK, "{saved}");
    }
    let (status,published) = api(&app,&cookie,&csrf,"POST",&format!("/api/console/applications/{application_id}/api-publications"),json!({
        "mapping":control_plane::application_public_api::mapping::ApplicationApiMappingConfig::default_native(),"api_enabled":true
    })).await;
    assert_eq!(status, StatusCode::CREATED, "{published}");
    let (status,binding) = api(&app,&cookie,&csrf,"PUT",&format!("/api/console/communications/instances/{}/binding",fixture.channel),json!({
        "application_id":application_id,"mcp_instance_ids":[fixture.mcp],"system_prompt":"Read only this sender's channel history using conversation_id from the session.",
        "model":null,"reasoning_effort":null,"file_table_id":null
    })).await;
    assert_eq!(status, StatusCode::OK, "{binding}");
    let operator_id =
        support::create_member(&app, &cookie, &csrf, "communication-operator", "temp-pass").await;
    let role: String = sqlx::query_scalar(
        "SELECT code FROM roles WHERE workspace_id=$1 AND is_default_member_role=true",
    )
    .bind(fixture.actor.current_workspace_id)
    .fetch_one(fixture.state.store.pool())
    .await
    .unwrap();
    operator_run_access(&fixture, &role, None).await;
    let (operator_cookie, operator_csrf) =
        support::login_and_capture_cookie(&app, "communication-operator", "temp-pass").await;
    assert_ne!(operator_id, fixture.actor.user_id.to_string());
    let (stop, signal) = watch::channel(false);
    let admission = tokio::spawn(super::super::commands::run(
        fixture.state.clone(),
        (fixture.actor.current_workspace_id, fixture.channel),
        signal.clone(),
    ));
    let execution = tokio::spawn(super::super::turns::run(
        fixture.state.clone(),
        (fixture.actor.current_workspace_id, fixture.channel),
        signal,
    ));
    let outcome = tokio::time::timeout(Duration::from_secs(60), async {
        let run_id = loop {
            let linked: Option<Uuid> = sqlx::query_scalar("SELECT run_id FROM communication_inputs WHERE instance_id=$1 AND external_message_key='alice-input'")
                .bind(fixture.channel).fetch_one(fixture.state.store.pool()).await.unwrap();
            if let Some(run_id)=linked {
                let status = fixture.state.store.get_flow_run_metadata(Uuid::parse_str(&application_id).unwrap(),run_id).await.unwrap().unwrap().status;
                if status == if callback {domain::FlowRunStatus::WaitingCallback} else {domain::FlowRunStatus::WaitingHuman} {break run_id;}
                if matches!(status,domain::FlowRunStatus::Failed|domain::FlowRunStatus::Cancelled) {
                    panic!("published communication failed before pause: {}", execution_diagnostics(&fixture,&application_id,run_id).await);
                }
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        };
        let (path,body) = if callback {
            let task: Uuid = sqlx::query_scalar("SELECT id FROM flow_run_callback_tasks WHERE flow_run_id=$1 AND status='pending' ORDER BY created_at DESC LIMIT 1")
                .bind(run_id).fetch_one(fixture.state.store.pool()).await.unwrap();
            (format!("/api/console/applications/{application_id}/orchestration/callback-tasks/{task}/complete"),json!({
                "response_payload":{"tool_results":[{"tool_call_id":"communication_approval","content":"approved"}]},
                "mcp_instance_ids":["attacker-selected-channel"]
            }))
        } else {
            let checkpoint: Uuid = sqlx::query_scalar("SELECT id FROM flow_run_checkpoints WHERE flow_run_id=$1 ORDER BY created_at DESC LIMIT 1")
                .bind(run_id).fetch_one(fixture.state.store.pool()).await.unwrap();
            (format!("/api/console/applications/{application_id}/orchestration/runs/{run_id}/resume"),json!({
                "checkpoint_id":checkpoint,"input_payload":{"node-human":{"input":"approved"}},
                "mcp_instance_ids":["attacker-selected-channel"]
            }))
        };
        let (status,denied) = api(&app,&operator_cookie,&operator_csrf,"POST",&path,body.clone()).await;
        assert_eq!(status,StatusCode::FORBIDDEN,"operator without application Run cannot resume: {denied}");
        operator_run_access(&fixture,&role,Some(&path)).await;
        sqlx::query("UPDATE users SET status='disabled' WHERE id=$1").bind(fixture.actor.user_id).execute(fixture.state.store.pool()).await.unwrap();
        let (status,denied) = api(&app,&operator_cookie,&operator_csrf,"POST",&path,body.clone()).await;
        sqlx::query("UPDATE users SET status='active' WHERE id=$1").bind(fixture.actor.user_id).execute(fixture.state.store.pool()).await.unwrap();
        assert_eq!(status,StatusCode::FORBIDDEN,"operator must not substitute for a disabled configuring actor: {denied}");
        assert_eq!(denied["code"], "communication_execution_denied", "the configuring-actor guard must be reached: {denied}");
        let (status,resumed) = api(&app,&operator_cookie,&operator_csrf,"POST",&path,body).await;
        assert_eq!(status,StatusCode::OK,"distinct operator may resume using live configuring authority: {resumed}");
        // Use the production projection owner, as the in-process fixture has no background host workers.
        loop {
            crate::workers::trace_projection::rebuild_trace_projection(&fixture.state.store,Uuid::parse_str(&application_id).unwrap(),run_id).await.unwrap();
            let (processing,send,reply): (String,String,Option<String>) = sqlx::query_as("SELECT m.processing_status,m.send_status,(SELECT content FROM communication_messages t WHERE t.input_id=m.id AND t.direction='outbound' AND t.type='text') FROM communication_inputs m WHERE m.instance_id=$1 AND m.external_message_key='alice-input'")
                .bind(fixture.channel).fetch_one(fixture.state.store.pool()).await.unwrap();
            if processing=="failed" || matches!(send.as_str(),"failed"|"unknown") {
                panic!("receive/final send failed: processing={processing}, send={send}, run={}",execution_diagnostics(&fixture,&application_id,run_id).await);
            }
            if send=="succeeded" { assert_eq!(processing,"succeeded");assert_eq!(reply.as_deref(),Some("Final for alice"));break; }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        let run = fixture.state.store.get_flow_run_metadata(Uuid::parse_str(&application_id).unwrap(),run_id).await.unwrap().unwrap();
        assert_eq!(run.created_by,fixture.actor.user_id,"callback operator cannot take ownership of execution");
        let logs = fixture.state.notification_service.logs(&fixture.actor,Default::default()).await.unwrap();
        let success = logs.items.iter().filter(|log|log.status=="success").collect::<Vec<_>>();
        assert_eq!(success.len(),1,"one final send for the resumed inbound input");
        assert_eq!(success[0].actor_user_id,fixture.actor.user_id,"the actual managed send retains configuring authority");
        assert!(success[0].content.is_none());
        // The real SDK worker rejects any target other than alice/external-room/alice-input,
        // and any text other than Final for alice. A success log therefore also proves that
        // provisional model tool text and the operator identity never became the outbound reply.

    }).await;
    if outcome.is_err() {
        let run_id: Option<Uuid> = sqlx::query_scalar("SELECT run_id FROM communication_inputs WHERE instance_id=$1 AND external_message_key='alice-input'")
            .bind(fixture.channel).fetch_one(fixture.state.store.pool()).await.unwrap();
        if let Some(run_id) = run_id {
            eprintln!(
                "communication timeout: {}",
                execution_diagnostics(&fixture, &application_id, run_id).await
            );
        }
    }
    stop.send(true).unwrap();
    execution.await.unwrap();
    admission.await.unwrap();
    fixture.runtime.host.stop().await.unwrap();
    outcome.expect("communication pipeline must complete within its bounded fixture budget");
}

#[tokio::test]
async fn communication_human_resume_preserves_configuring_actor_and_sends_final_scoped_reply() {
    pipeline(false).await;
}
#[tokio::test]
async fn communication_callback_preserves_configuring_actor_and_sends_final_scoped_reply() {
    pipeline(true).await;
}
