use crate::{
    _tests::support::{
        create_member, create_role, login_and_capture_cookie, replace_member_roles,
        test_api_state_with_database_url,
    },
    middleware::require_session::{
        require_session, with_server_delegated_request_context, RequestContext,
    },
    routes::{
        mcp_management::{
            debug_execute::{McpDebugExecuteBody, McpDebugExecuteError, McpServerBoundInputs},
            mcp_interface_catalog_entries,
        },
        mcp_protocol::virtual_ui::{
            ConsoleRouterMcpInterfaceDispatchPort, McpInterfaceDispatchPort,
        },
    },
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("command-delegation-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cookie_headers(cookie: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    headers
}

async fn json_response(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn local_command_mcp_delegation_preserves_authorization_and_output() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let root = require_session(&state, &cookie_headers(&cookie))
        .await
        .unwrap();
    let interface = mcp_interface_catalog_entries(&state, &root.actor)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.interface_id == "local_commands.execute")
        .unwrap();
    let member = create_member(&app, &cookie, &csrf, "delegated-command", "temp-pass").await;
    create_role(&app, &cookie, &csrf, "delegated_command_role").await;
    replace_member_roles(&app, &cookie, &csrf, &member, &["delegated_command_role"]).await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "delegated-command", "temp-pass").await;
    let member_context = require_session(&state, &cookie_headers(&member_cookie))
        .await
        .unwrap();
    let delegated = RequestContext::server_delegation(member_context.user, member_context.actor);
    let dispatcher = ConsoleRouterMcpInterfaceDispatchPort::new(app.clone());
    let temp = Temp::new();
    let marker = temp.0.join("executed");
    let command = format!(
        "printf done > '{}'; printf delegated-output; printf delegated-error >&2; exit 9",
        marker.display()
    );

    // Reuse the same delegated identity across grant/revoke: current policy must be reloaded.
    for allowed in [false, true, false] {
        let operations = if allowed {
            json!([{"kind":"simple","operation_id":"local_commands.execute","enabled":true}])
        } else {
            json!([])
        };
        let update = app.clone().oneshot(Request::builder()
            .method("PUT").uri("/api/console/settings/roles/delegated_command_role/console-policy")
            .header("cookie", &cookie).header("x-csrf-token", &csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({"groups":[{"kind":"other","group_id":"other.local-commands","enabled":true,"strategy":"custom","operations":operations}]}).to_string())).unwrap())
            .await.unwrap();
        assert_eq!(update.status(), StatusCode::NO_CONTENT);

        // This is the MCP dispatch adapter with no cookie, API key or CSRF header on the
        // replayed HTTP request; its only identity is trusted in-process delegation.
        let result = with_server_delegated_request_context(
            delegated.clone(),
            dispatcher.execute(
                HeaderMap::new(),
                interface.clone(),
                serde_json::from_value::<McpDebugExecuteBody>(json!({
                    "interface_id":"local_commands.execute",
                    "debug_response_mode":"debug_details",
                    "mcp_arguments":{"command":command},
                    "input_mapping":{"mappings":[{"interface_param":"command","source":{"kind":"mcp_argument","path":"command"},"required":true}]},
                    "output_mapping":{}
                })).unwrap(),
                McpServerBoundInputs { workspace_id: delegated.actor.current_workspace_id },
            ),
        ).await;
        match result {
            Ok(value) => {
                assert!(allowed, "ungranted delegation executed: {value}");
                let output = &value["interface_response"]["data"];
                assert_eq!(output["stdout"], "delegated-output");
                assert_eq!(output["stderr"], "delegated-error");
                assert_eq!(output["exit_code"], 9);
                assert_eq!(output["cleanup_complete"], true);
                assert_eq!(output["output_complete"], true);
                assert_eq!(std::fs::read(&marker).unwrap(), b"done");
                std::fs::remove_file(&marker).unwrap();
            }
            Err(McpDebugExecuteError::TargetResponse(response)) => {
                let status = response.status();
                let value = json_response(response).await;
                assert!(!allowed, "delegated execution failed: {status} {value}");
                assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
                assert!(!marker.exists(), "denied command had a side effect");
            }
            Err(McpDebugExecuteError::Api(error)) => panic!("MCP dispatch failed: {error}"),
        }
    }
}

#[tokio::test]
async fn local_command_http_disconnect_keeps_completion_audit_owner() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let temp = Temp::new();
    let started = temp.0.join("started");
    let release = temp.0.join("release");
    let finished = temp.0.join("finished");
    // A bounded file barrier lets the test abort HTTP while execution is definitely active.
    // The timeout also prevents a failed assertion from leaving a shell running indefinitely.
    let command = format!(
        "printf started > '{}'; i=0; while [ ! -e '{}' ] && [ $i -lt 250 ]; do sleep 0.02; i=$((i+1)); done; printf finished > '{}'",
        started.display(), release.display(), finished.display()
    );
    let request = Request::builder()
        .method("POST")
        .uri("/api/console/local-commands/execute")
        .header("cookie", cookie)
        .header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(Body::from(json!({"command":command}).to_string()))
        .unwrap();
    let response_task = tokio::spawn(app.oneshot(request));
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !started.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("command must start before disconnect");
    assert!(!finished.exists());
    response_task.abort();
    assert!(response_task.await.unwrap_err().is_cancelled());
    std::fs::write(&release, b"release").unwrap();

    let store = storage_durable_postgres::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    let audits: Vec<(uuid::Uuid, String, Value)> =
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let rows = sqlx::query_as::<_, (uuid::Uuid, String, Value)>(
                    "SELECT target_id, event_code, payload FROM audit_logs WHERE target_type = 'local_command'",
                )
                .fetch_all(store.pool())
                .await
                .unwrap();
                if rows.len() == 2 {
                    break rows;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("detached Kernel owner must finish the audit");
    assert_eq!(std::fs::read(&finished).unwrap(), b"finished");
    assert_eq!(audits[0].0, audits[1].0);
    assert_eq!(
        audits
            .iter()
            .filter(|(_, event, p)| event == "local_command.started"
                && p.get("command_bytes").is_some())
            .count(),
        1
    );
    assert_eq!(
        audits
            .iter()
            .filter(
                |(_, event, p)| event == "local_command.finished" && p["cleanup_complete"] == true
            )
            .count(),
        1
    );
    assert!(!serde_json::to_string(&audits).unwrap().contains(&command));
}
